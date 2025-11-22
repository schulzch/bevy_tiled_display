use super::*;
use bevy::prelude::*;
use std::env;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::time::{Duration, Instant};

/// A UDP-based synchronization backend.
///
// This follows a best-effort approach by sending a small message to the multicast group
// and waiting a short while for any incoming packets.
pub struct UdpSync {
    socket: UdpSocket,
    multicast: SocketAddr,
    buf_size: usize,
}

impl UdpSync {
    fn join_multicast(socket: &UdpSocket, multi: IpAddr) -> io::Result<()> {
        match multi {
            IpAddr::V4(v4) => socket.join_multicast_v4(&v4, &Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(v6) => socket.join_multicast_v6(&v6, 0),
        }
    }
}

impl SyncBackend for UdpSync {
    fn new() -> Self {
        // Allow overriding using env vars: DEFAULT_MULTICAST_IP, MULTICAST_IP, DEFAULT_MULTICAST_PORT, and MULTICAST_PORT.
        let multicast_ip: IpAddr = env::var("MULTICAST_IP")
            .ok()
            .or_else(|| env::var("DEFAULT_MULTICAST_IP").ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| "239.255.0.1".parse().unwrap());
        let mutli_port = env::var("MULTICAST_PORT")
            .ok()
            .or_else(|| env::var("DEFAULT_MULTICAST_PORT").ok())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(4000);

        // Create and bind socket according to IP version
        let (socket, multicast_addr) = {
            let (bind_addr, multicast_addr) = match multicast_ip {
                IpAddr::V4(v4) => (
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, mutli_port)),
                    SocketAddr::V4(SocketAddrV4::new(v4, mutli_port)),
                ),
                IpAddr::V6(v6) => (
                    SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, mutli_port, 0, 0)),
                    SocketAddr::V6(SocketAddrV6::new(v6, mutli_port, 0, 0)),
                ),
            };

            let sock = UdpSocket::bind(bind_addr).expect("UdpSync: bind failed");
            sock.set_read_timeout(Some(TIMEOUT)).ok();
            let _ = Self::join_multicast(&sock, multicast_ip);
            (sock, multicast_addr)
        };

        info!(
            "UdpSync listening on {} (multicast {})",
            socket.local_addr().unwrap_or_else(|_| multicast_addr),
            multicast_addr
        );

        UdpSync {
            socket,
            multicast: multicast_addr,
            buf_size: 65536,
        }
    }

    fn barrier(&self) {
        let _ = self.socket.send_to(b"BARRIER", self.multicast);

        let timeout = Duration::from_millis(200);
        let deadline = Instant::now() + timeout;
        let mut buf = vec![0u8; 1500];
        while Instant::now() < deadline {
            match self.socket.recv_from(&mut buf) {
                Ok((_n, _addr)) => continue,
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => break,
                    _ => break,
                },
            }
        }
    }

    fn broadcast(&self, bytes: &[u8]) -> Vec<u8> {
        if !bytes.is_empty() {
            let _ = self.socket.send_to(bytes, self.multicast);
        }

        let mut buf = vec![0u8; self.buf_size];
        match self.socket.recv_from(&mut buf) {
            Ok((n, _addr)) => {
                buf.truncate(n);
                buf
            }
            Err(_) => {
                if !bytes.is_empty() {
                    bytes.to_vec()
                } else {
                    vec![]
                }
            }
        }
    }
}

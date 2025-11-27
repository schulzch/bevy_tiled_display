use super::*;
use std::env;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::time::{Duration, Instant};

/// A UDP-based synchronization backend.
///
// This follows a best-effort approach by sending a small message to the multicast group
// and waiting a short while for any incoming packets.
pub struct UdpSync {
    rank: u32,
    socket: UdpSocket,
    multicast: SocketAddr,
    buf_size: usize,
}

impl UdpSync {
    pub fn new() -> Self {
        // Allow overriding using env vars: RANK, DEFAULT_MULTICAST_IP, MULTICAST_IP, DEFAULT_MULTICAST_PORT, and MULTICAST_PORT.
        let rank = env::var("RANK")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or_else(|| std::process::id() + 1);
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

            let sock = UdpSocket::bind(bind_addr).expect("UDP socket bind failed");
            sock.set_read_timeout(Some(TIMEOUT)).ok();
            Self::join_multicast(&sock, multicast_ip).expect("UDP join multicast failed");
            (sock, multicast_addr)
        };

        UdpSync {
            rank,
            socket,
            multicast: multicast_addr,
            buf_size: 65536,
        }
    }

    fn join_multicast(socket: &UdpSocket, multi: IpAddr) -> io::Result<()> {
        match multi {
            IpAddr::V4(v4) => socket.join_multicast_v4(&v4, &Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(v6) => socket.join_multicast_v6(&v6, 0),
        }
    }
}

impl SyncBackend for UdpSync {
    fn rank(&self) -> u32 {
        self.rank
    }

    fn barrier(&self) -> Result<(), SyncError> {
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
        Ok(())
    }

    fn broadcast(&self, data: &[u8]) -> Result<Vec<u8>, SyncError> {
                //TODO: use rank
        if !data.is_empty() {
            let _ = self.socket.send_to(data, self.multicast);
        }

        let mut buf = vec![0u8; self.buf_size];
        match self.socket.recv_from(&mut buf) {
            Ok((n, _addr)) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(_) => {
                if !data.is_empty() {
                    Ok(data.to_vec())
                } else {
                    Ok(vec![])
                }
            }
        }
    }
}

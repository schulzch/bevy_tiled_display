use super::*;
use socket2::{Domain, Protocol, Socket, Type};
use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

/// A UDP-based synchronization backend.
///
// This follows a best-effort approach by sending a small message to the multicast group
// and waiting a short while for any incoming packets.
pub struct UdpSync {
    rank: u8,
    world_size: u8,
    socket: UdpSocket,
    multicast_addr: SocketAddr,
    generation: Arc<AtomicU64>,
}

impl UdpSync {
    pub fn new() -> Self {
        // Allow overriding using env vars: RANK, DEFAULT_MULTICAST_IP, MULTICAST_IP, DEFAULT_MULTICAST_PORT, and MULTICAST_PORT.
        let rank = env::var("RANK")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .expect("RANK environment variable must be set");
        let world_size = env::var("WORLD_SIZE")
            .ok()
            .and_then(|s| s.parse::<u8>().ok())
            .expect("WORLD_SIZE environment variable must be set");
        let multicast_ip: IpAddr = env::var("MULTICAST_IP")
            .ok()
            .or_else(|| env::var("DEFAULT_MULTICAST_IP").ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| "239.255.0.1".parse().unwrap());
        let multicast_port = env::var("MULTICAST_PORT")
            .ok()
            .or_else(|| env::var("DEFAULT_MULTICAST_PORT").ok())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(4000);

        // Create and bind socket according to IP version
        let (socket, multicast_addr) = {
            let (socket, bind_addr, multicast_addr) = match multicast_ip {
                IpAddr::V4(v4) => (
                    Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
                        .expect("UDP socket creation failed"),
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, multicast_port)),
                    SocketAddr::V4(SocketAddrV4::new(v4, multicast_port)),
                ),
                IpAddr::V6(v6) => (
                    Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))
                        .expect("UDP socket creation failed"),
                    SocketAddr::V6(SocketAddrV6::new(
                        Ipv6Addr::UNSPECIFIED,
                        multicast_port,
                        0,
                        0,
                    )),
                    SocketAddr::V6(SocketAddrV6::new(v6, multicast_port, 0, 0)),
                ),
            };

            socket
                .set_reuse_address(true)
                .expect("Failed to set reuse address");
            socket
                .bind(&bind_addr.into())
                .expect("UDP socket bind failed");
            socket
                .set_read_timeout(Some(TIMEOUT))
                .expect("Failed to set read timeout");
            match multicast_ip {
                IpAddr::V4(v4) => socket.join_multicast_v4(&v4, &Ipv4Addr::UNSPECIFIED),
                IpAddr::V6(v6) => socket.join_multicast_v6(&v6, 0),
            }
            .expect("UDP join multicast failed");
            (socket.into(), multicast_addr)
        };

        UdpSync {
            rank,
            world_size,
            socket,
            multicast_addr,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl SyncBackend for UdpSync {
    fn rank(&self) -> u32 {
        self.rank as u32
    }

    fn barrier(&self) -> Result<(), SyncError> {
        const BARRIER_MESSAGE_SIZE: usize = 64;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst);

        // Phase 1: Everyone announces by sending their (generation, rank) tuple
        let message = bincode::serialize(&(generation, self.rank))
            .map_err(|e| SyncError::Error(e.to_string()))?;
        self.socket
            .send_to(&message, &self.multicast_addr)
            .map_err(|e| SyncError::Error(e.to_string()))?;

        // Phase 2: Everyone waits for all announcements of this generation
        let deadline = Instant::now() + TIMEOUT;
        let mut arrived = vec![false; self.world_size as usize];
        arrived[self.rank as usize] = true;
        while Instant::now() < deadline {
            let mut buf = [0u8; BARRIER_MESSAGE_SIZE];
            match self.socket.recv_from(&mut buf) {
                Ok((len, _)) => {
                    if let Ok((g, r)) = bincode::deserialize::<(u64, u8)>(&buf[..len]) {
                        if g == generation && r < self.world_size {
                            arrived[r as usize] = true;
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(SyncError::Error(e.to_string())),
            }

            // Check if all announcements have arrived
            if arrived.iter().all(|&x| x) {
                return Ok(());
            }
        }

        Err(SyncError::Timeout)
    }

    fn broadcast(&self, data: &mut Vec<u8>) -> Result<(), SyncError> {
        // The primary rank broadcasts.
        if self.rank == 0 {
            self.socket
                .send_to(data, &self.multicast_addr)
                .map_err(|e| SyncError::Error(e.to_string()))?;
            return Ok(());
        }

        // Non-primary ranks wait for data.
        const MAX_DATA_SIZE: usize = 65536;
        data.resize(MAX_DATA_SIZE, 0);
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            match self.socket.recv_from(data) {
                Ok((data_len, _)) => {
                    data.truncate(data_len);
                    return Ok(());
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => return Err(SyncError::Error(e.to_string())),
            }
        }

        Err(SyncError::Timeout)
    }
}

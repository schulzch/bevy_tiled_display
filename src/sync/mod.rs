#[cfg(feature = "mpi")]
pub mod mpi;
pub mod udp;

#[cfg(feature = "mpi")]
pub use mpi::*;
pub use udp::*;

use std::time::Duration;

/// Reasonable synchronization timeout.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(2);

/// Errors that can occur during synchronization operations.
#[derive(Debug, Clone)]
pub enum SyncError {
    /// Operation failed
    Failed(String),
    /// Operation timed out
    Timeout,
}

/// Non-send trait for process synchronization backends (must live on the main thread).
///
/// Synchronization backends coordinate state across multiple processes.
pub trait SyncBackend {
    /// Returns this process rank.
    ///
    /// Rank zero is the primary process and is responsible for initiating broadcasts and coordinating other collective operations.
    fn rank(&self) -> u32;

    /// Broadcasts data from the primary process to all others.
    ///
    /// This is a collective operation - all processes must call this method to proceed.
    /// The primary process sends its `data`, while all other processes ignore their
    /// `data` parameter and receive the broadcast value.
    fn broadcast(&self, data: &mut Vec<u8>) -> Result<(), SyncError>;

    /// Blocks until all participating processes reach this barrier point.
    ///
    /// This is a collective operation - all processes must call it for any to proceed.
    fn barrier(&self) -> Result<(), SyncError>;
}

/// Available sync backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackendType {
    /// UDP backend.
    Udp,
    /// MPI backend (if `mpi` feature is disabled, `Mpi` falls back to `Udp`).
    Mpi,
}

impl SyncBackendType {
    /// Build a concrete backend.
    pub fn build(self) -> Box<dyn SyncBackend> {
        match self {
            SyncBackendType::Udp => Box::new(UdpSync::new()),
            SyncBackendType::Mpi => {
                #[cfg(feature = "mpi")]
                {
                    Box::new(MpiSync::new())
                }
                #[cfg(not(feature = "mpi"))]
                {
                    Box::new(UdpSync::new())
                }
            }
        }
    }
}

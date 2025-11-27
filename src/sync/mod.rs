#[cfg(feature = "mpi")]
pub mod mpi;
pub mod udp;

#[cfg(feature = "mpi")]
pub use mpi::*;
pub use udp::*;

use std::{fmt, time::Duration};

/// Reasonable synchronization timeout.
const TIMEOUT: Duration = Duration::from_secs(2);

/// Errors that can occur during synchronization operations.
#[derive(Debug, Clone)]
pub enum SyncError {
    /// Operation failed
    Error(String),
    /// Operation timed out
    Timeout,
}

/// Non-send trait for process synchronization backends (must live on the main thread).
///
/// Synchronization backends coordinate state across multiple processes.
pub trait SyncBackend {
    /// Returns whether this process is the primary process.
    ///
    /// The primary process is responsible for initiating broadcasts and coordinating other collective operations.
    fn is_primary(&self) -> bool;

    /// Broadcasts data from the primary process to all others.
    ///
    /// This is a collective operation - all processes must call this method to proceed.
    /// The primary process sends its `data`, while all other processes ignore their
    /// `data` parameter and receive the broadcast value.
    fn broadcast(&self, data: &[u8]) -> Result<Vec<u8>, SyncError>;

    /// Blocks until all participating processes reach this barrier point.
    ///
    /// This is a collective operation - all processes must call it for any to proceed.
    fn barrier(&self) -> Result<(), SyncError>;
}

/// Selection enum for available synchronization backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackends {
    /// Pick a sensible backend at runtime.
    Auto,
    /// UDP backend.
    Udp,
    /// MPI backend (requires `mpi` feature).
    Mpi,
}

#[derive(Debug)]
pub enum SyncBackendError {
    FeatureNotEnabled(&'static str),
}

impl fmt::Display for SyncBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureNotEnabled(feature) => {
                write!(f, "Backend requires '{}' feature to be enabled", feature)
            }
        }
    }
}

impl std::error::Error for SyncBackendError {}

impl SyncBackends {
    /// Construct a backend instance.
    pub fn build(self) -> Result<Box<dyn SyncBackend>, SyncBackendError> {
        match self {
            SyncBackends::Udp => Ok(Box::new(UdpSync::new())),
            SyncBackends::Mpi => {
                #[cfg(feature = "mpi")]
                {
                    Ok(Box::new(MpiSync::new()))
                }
                #[cfg(not(feature = "mpi"))]
                {
                    Err(SyncBackendError::FeatureNotEnabled("mpi"))
                }
            }
            SyncBackends::Auto => {
                #[cfg(feature = "mpi")]
                {
                    Ok(Box::new(MpiSync::new()))
                }
                #[cfg(not(feature = "mpi"))]
                {
                    Ok(Box::new(UdpSync::new()))
                }
            }
        }
    }
}

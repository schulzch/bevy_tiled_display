#[cfg(feature = "mpi")]
pub mod mpi;
pub mod no;
pub mod udp;

#[cfg(feature = "mpi")]
pub use mpi::*;
pub use no::*;
pub use udp::*;

use std::time::Duration;

/// Reasonable synchronization timeout.
const TIMEOUT: Duration = Duration::from_secs(2);

/// Non-send trait for screen synchronization backends (must live on the main thread).
pub trait SyncBackend {
    /// Blocks until every participating process reaches this point.
    fn barrier(&self);

    /// Broadcasts the given bytes to all processes participating in synchronization.
    ///
    /// # Broadcast Pattern
    /// - The broadcast is initiated by the calling process (typically rank 0 or the main process).
    /// - All other processes receive the broadcasted data.
    /// - All processes must call this method collectively; the sender provides the data, receivers may receive it via backend-specific mechanisms.
    fn broadcast(&self, bytes: &[u8]) -> Vec<u8>;
}

/// Selection enum for available synchronization backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackends {
    /// Pick a sensible backend at runtime.
    Auto,
    /// No-op backend.
    No,
    /// UDP backend.
    Udp,
    /// MPI backend (requires `mpi` feature).
    Mpi,
}

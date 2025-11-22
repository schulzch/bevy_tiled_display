#[cfg(feature = "mpi")]
pub mod mpi;
pub mod no;

#[cfg(feature = "mpi")]
pub use mpi::*;
pub use no::*;

/// Non-send trait for screen synchronization backends (must live on the main thread).
#[allow(dead_code)]
pub trait SyncBackend {
    /// Constructs a new synchronization backend.
    fn new() -> Self
    where
        Self: Sized;

    /// Blocks until every participating process reaches this point.
    fn barrier(&self);

    /// Broadcasts the given bytes to all processes participating in synchronization.
    ///
    /// # Broadcast Pattern
    /// - The broadcast is initiated by the calling process (typically rank 0 or the main process).
    /// - All other processes receive the broadcasted data.
    /// - All processes must call this method collectively; the sender provides the data, receivers may receive it via backend-specific mechanisms.
    fn broadcast(&self, bytes: &[u8]);
}

/// Selection enum for available synchronization backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackends {
    /// Pick a sensible backend at runtime.
    Auto,
    /// No-op backend.
    No,
    /// MPI backend (requires `mpi` feature).
    Mpi,
}

#[cfg(feature = "mpi")]
pub mod mpi;
pub mod no;

#[cfg(feature = "mpi")]
pub use mpi::*;
pub use no::*;

/// Non-send trait for screen synchronization backends (must live on the main thread).
///
/// Implementations should register any resources and systems necessary to
/// coordinate frames across multiple processes.
#[allow(dead_code)]
pub trait SyncBackend {
    /// Called during app construction to register resources and systems.
    fn new() -> Self
    where
        Self: Sized;

    /// Called by frame synchronization at the end of a frame.
    fn barrier(&self);
}

/// Simple selection enum for available synchronization backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackends {
    /// Pick a sensible default at runtime or by feature flags.
    Auto,
    /// No-op backend.
    No,
    /// Use an MPI-backed barrier synchronization (requires `mpi` feature).
    Mpi,
}

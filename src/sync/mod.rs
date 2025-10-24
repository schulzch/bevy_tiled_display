use bevy::prelude::*;

#[cfg(feature = "mpi")]
pub mod mpi;

#[cfg(feature = "mpi")]
pub use mpi::*;

/// Trait for screen synchronization backends.
///
/// Implementations should register any resources and systems necessary to
/// coordinate frames across multiple processes.
#[allow(dead_code)]
pub trait SyncBackend {
    /// Called during app construction to register resources and systems.
    fn setup(&self, app: &mut App);
}

/// Simple selection enum for available synchronization backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncBackends {
    /// Pick a sensible default at runtime or by feature flags.
    Auto,
    /// Use an MPI-backed barrier synchronization (requires `mpi` feature).
    Mpi,
}

use super::SyncBackend;

/// A no-op synchronization backend.
#[derive(Clone)]
pub struct NoSync;

impl SyncBackend for NoSync {
    fn new() -> Self {
        NoSync
    }

    fn barrier(&self) {}
}

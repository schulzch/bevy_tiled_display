use super::SyncBackend;

/// No-op sync backend for single-threaded or testing purposes.
#[derive(Clone)]
pub struct NoSync;
impl SyncBackend for NoSync {
    fn new() -> Self {
        NoSync
    }

    fn barrier(&self) {}

    fn broadcast(&self, _bytes: &[u8]) {}
}

use super::SyncBackend;

/// No-op sync backend for single-threaded or testing purposes.
#[derive(Clone)]
pub struct NoSync;

impl NoSync {
    pub fn new() -> Self {
        NoSync
    }
}

impl SyncBackend for NoSync {
    fn barrier(&self) {}

    fn broadcast(&self, bytes: &[u8]) -> Vec<u8> {
        bytes.to_vec()
    }
}

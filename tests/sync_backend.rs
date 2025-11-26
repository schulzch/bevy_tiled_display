use bevy_tiled_display::*;
use mockall::mock;
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};

mock! {
    pub Backend {}

    impl SyncBackend for Backend {
        fn barrier(&self);
        fn broadcast(&self, bytes: &[u8]) -> Vec<u8>;
    }
}

#[test]
fn sync_barrier_broadcast() {
    // Test barrier: set an atomic flag when barrier() is called.
    let mut mock = MockBackend::new();
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = flag.clone();
    mock.expect_barrier().times(1).returning(move || {
        flag_clone.store(true, Ordering::SeqCst);
    });

    mock.barrier();
    assert!(flag.load(Ordering::SeqCst));

    // Broadcast without override echoes input.
    let mut mock_echo = MockBackend::new();
    mock_echo
        .expect_broadcast()
        .returning(|bytes: &[u8]| bytes.to_vec());
    let data = vec![1u8, 2, 3];
    let echoed_data = mock_echo.broadcast(&data);
    assert_eq!(echoed_data, data);

    // Broadcast with override returns the override bytes.
    let override_bytes = vec![9u8, 9, 9];
    let mut mock_override = MockBackend::new();
    mock_override
        .expect_broadcast()
        .returning(move |_bytes: &[u8]| override_bytes.clone());
    let overridden_result = mock_override.broadcast(&[0u8]);
    assert_eq!(overridden_result, vec![9u8, 9, 9]);
}

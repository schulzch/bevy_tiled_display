use bevy::prelude::*;
use bevy_tiled_display::*;
use mockall::mock;
use std::sync::{Arc, atomic::AtomicBool, atomic::Ordering};

#[derive(Resource, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
struct MockResource {
    pub val: i32,
}

mock! {
    pub Backend {}

    impl SyncBackend for Backend {
        fn is_primary(&self) -> bool;

        fn broadcast(&self, data: &[u8]) -> Result<Vec<u8>, SyncError>;

        fn barrier(&self) -> Result<(), SyncError>;
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
        Ok(())
    });

    mock.barrier().unwrap();
    assert!(flag.load(Ordering::SeqCst));

    // Broadcast without override echoes input.
    let mut mock_echo = MockBackend::new();
    mock_echo
        .expect_broadcast()
        .returning(|bytes: &[u8]| Ok(bytes.to_vec()));
    let data = vec![1u8, 2, 3];
    let echoed_data = mock_echo.broadcast(&data).unwrap();
    assert_eq!(echoed_data, data);

    // Broadcast with override returns the override bytes.
    let override_bytes = vec![9u8, 9, 9];
    let mut mock_override = MockBackend::new();
    mock_override
        .expect_broadcast()
        .returning(move |_bytes: &[u8]| Ok(override_bytes.clone()));
    let overridden_result = mock_override.broadcast(&[0u8]).unwrap();
    assert_eq!(overridden_result, vec![9u8, 9, 9]);
}

#[test]
fn sync_resource_register() {
    let mut app = App::new();

    // The plugin normally inserts TileSyncRegistry; mimic that here.
    app.insert_resource(TileSyncRegistry::new());

    app.insert_sync_resource(MockResource { val: 7 });

    let registry = app
        .world()
        .get_resource::<TileSyncRegistry>()
        .expect("TileSyncRegistry should be present");
    assert!(registry.contains::<MockResource>());

    let stored = app
        .world()
        .get_resource::<MockResource>()
        .expect("MockResource should be present after insert_sync_resource");
    assert_eq!(stored.val, 7);
}

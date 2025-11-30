use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_tiled_display::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

/// Test resource to verify sync resource registration and broadcasting.
#[derive(Resource, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
struct TestResource {
    pub val: i32,
}

/// Test backend that counts calls to `broadcast` and `barrier` to verify sync
/// registration and that broadcast/barrier are called once per frame.
struct TestBackend {
    rank: u32,
    pub barrier_calls: Arc<AtomicU32>,
    pub broadcast_calls: Arc<AtomicU32>,
    /// Optional payload written to by serializers.
    pub broadcast_input: Arc<Mutex<Option<Vec<u8>>>>,
    /// Optional payload for deserializers when `rank != 0`.
    pub broadcast_output: Option<Vec<u8>>,
}

impl TestBackend {
    fn new(
        rank: u32,
        barrier_calls: Arc<AtomicU32>,
        broadcast_calls: Arc<AtomicU32>,
        broadcast_input: Arc<Mutex<Option<Vec<u8>>>>,
        broadcast_output: Option<Vec<u8>>,
    ) -> Self {
        Self {
            rank,
            barrier_calls,
            broadcast_calls,
            broadcast_input,
            broadcast_output,
        }
    }
}

impl SyncBackend for TestBackend {
    fn rank(&self) -> u32 {
        self.rank
    }

    fn world_size(&self) -> u32 {
        todo!();
    }

    fn broadcast(&self, data: &mut Vec<u8>) -> Result<(), SyncError> {
        self.broadcast_calls.fetch_add(1, Ordering::SeqCst);

        {
            let mut guard = self.broadcast_input.lock().unwrap();
            *guard = Some(data.clone());
        }

        // For non-zero ranks, i.e., broadcast receivers, write payload.
        if self.rank != 0 {
            data.clear();
            data.extend_from_slice(self.broadcast_output.as_deref().unwrap_or(&[]));
        }

        Ok(())
    }

    fn barrier(&self) -> Result<(), SyncError> {
        self.barrier_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Helper to build a `TiledDisplay` containing a single machine/tile;
/// for plugin's startup and avoiding panics when running `app.update()`.
fn make_tiled_display(identity: &str) -> TiledDisplay {
    let tile = Tile {
        name: "test-tile".to_string(),
        stereo_channel: StereoChannel::Left,
        left_offset: 0,
        top_offset: 0,
        window_left: 0,
        window_top: 0,
        window_width: 100,
        window_height: 100,
    };

    let machine = Machine {
        identity: identity.to_string(),
        tiles: vec![tile],
    };

    TiledDisplay {
        machines: vec![machine],
        name: "test-display".to_string(),
        width: 100,
        height: 100,
    }
}

/// Helper to build a `TiledDisplay` test app with a custom backend rank and
/// optional broadcast payload. Returns the `App` and a `TestBackend` instance
/// that shares the same counters and payload used by the plugin.
fn make_test_app(
    identity: &str,
    rank: u32,
    broadcast_output: Option<Vec<u8>>,
) -> (App, TestBackend) {
    let mut app = App::new();

    // Add a minimal window entity so the plugin's startup system can run.
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    let barrier_calls = Arc::new(AtomicU32::new(0));
    let broadcast_calls = Arc::new(AtomicU32::new(0));
    let broadcast_input = Arc::new(Mutex::new(None));

    let returned_backend = TestBackend::new(
        rank,
        barrier_calls.clone(),
        broadcast_calls.clone(),
        broadcast_input.clone(),
        broadcast_output.clone(),
    );

    let tiled = make_tiled_display(identity);
    app.add_plugins(
        TiledDisplayPlugin::new()
            .with_identity(identity)
            .with_tiled_display(tiled)
            .with_sync(move || {
                Box::new(TestBackend::new(
                    rank,
                    barrier_calls.clone(),
                    broadcast_calls.clone(),
                    broadcast_input.clone(),
                    broadcast_output.clone(),
                ))
            }),
    );

    (app, returned_backend)
}

/// Verifies the sync backend's `barrier` is invoked once at the end of each frame.
#[test]
fn frame_barrier_called_once() {
    let (mut app, backend) = make_test_app("test-barrier", 0, None);

    let barrier_calls = backend.barrier_calls.clone();
    app.add_systems(PostUpdate, move || {
        // Ensure the barrier has not already run for this frame — broadcast
        // should happen before the frame barrier. If this triggers the test
        // will fail, indicating incorrect ordering in the plugin systems.
        let barrier_count = barrier_calls.load(Ordering::SeqCst);
        assert_eq!(barrier_count, 0, "barrier was called before end of frame");
    });

    // Run a single frame; PreUpdate systems perform broadcasts, Last performs barrier.
    app.update();

    assert_eq!(backend.barrier_calls.load(Ordering::SeqCst), 1);
}

/// Verifies that a resource can be registered for synchronization.
#[test]
fn sync_resource_registration() {
    let (mut app, _backend) = make_test_app("test-reg", 0, None);

    // Insert a sync resource; plugin should have created the TileSyncRegistry.
    app.insert_sync_resource(TestResource { val: 7 });

    // Run one frame to ensure startup systems have executed.
    app.update();

    let registry = app
        .world()
        .get_resource::<TileSyncRegistry>()
        .expect("TileSyncRegistry should be present");
    assert!(registry.contains::<TestResource>());

    let stored = app
        .world()
        .get_resource::<TestResource>()
        .expect("TestResource should be present after insert_sync_resource");
    assert_eq!(stored.val, 7);
}

/// Verifies the sync backend's `broadcast` is invoked once per frame
/// when a sync resource is registered and before the frame barrier.
#[test]
fn resource_broadcast_called_once() {
    let (mut app, backend) = make_test_app("test-broadcast", 0, None);

    // Register a sync resource so the broadcast system has something to serialize.
    app.insert_sync_resource(TestResource { val: 42 });

    let barrier_calls = backend.barrier_calls.clone();
    app.add_systems(PostUpdate, move || {
        // Ensure the barrier has not already run for this frame — broadcast
        // should happen before the frame barrier. If this triggers the test
        // will fail, indicating incorrect ordering in the plugin systems.
        let barrier_count = barrier_calls.load(Ordering::SeqCst);
        assert_eq!(
            barrier_count, 0,
            "barrier was called before broadcast; expected broadcast to run before barrier"
        );
    });

    // Run one frame.
    app.update();

    assert_eq!(backend.broadcast_calls.load(Ordering::SeqCst), 1);
}

/// Verifies that when the backend has `rank == 0` (root), resources are
/// serialized and the broadcast callback is invoked; the deserializer then
/// re-inserts the resource (round-trip) so the value remains the same.
#[test]
fn root_serializes_and_deserializes() {
    let (mut app, backend) = make_test_app("test-root-serialize", 0, None);

    // Register and insert a sync resource with a distinctive value.
    app.insert_sync_resource(TestResource { val: 77 });

    // Run one frame to trigger the sync system.
    app.update();

    // Ensure broadcast was called exactly once.
    assert_eq!(backend.broadcast_calls.load(Ordering::SeqCst), 1);

    // Check that backend captured the serialized resource payload.
    {
        let guard = backend.broadcast_input.lock().unwrap();
        let bytes = guard
            .as_ref()
            .expect("backend should have broadcast_input after broadcast");
        let deser: TestResource =
            bincode::deserialize(bytes).expect("failed to deserialize backend broadcast_input");
        assert_eq!(deser, TestResource { val: 77 });
    }

    // Resource should be present and retain the serialized value.
    let stored = app
        .world()
        .get_resource::<TestResource>()
        .expect("TestResource should be present after sync");
    assert_eq!(stored.val, 77);
}

/// Verifies that when the backend has `rank != 0` (non-root), the backend's
/// provided payload is deserialized into the app's world replacing the local
/// resource value.
#[test]
fn nonroot_deserializes_payload() {
    // Prepare payload representing TestResource { val: 55 }
    let payload = bincode::serialize(&TestResource { val: 55 }).unwrap();

    let (mut app, backend) = make_test_app("test-nonroot-deser", 1, Some(payload));

    // Insert a different value locally; the incoming broadcast payload should
    // overwrite it during deserialization.
    app.insert_sync_resource(TestResource { val: 0 });

    // Run one frame to trigger the sync system and apply the payload.
    app.update();

    // Ensure broadcast was called once on the backend.
    assert_eq!(backend.broadcast_calls.load(Ordering::SeqCst), 1);

    // The resource should now reflect the value from the payload.
    let stored = app
        .world()
        .get_resource::<TestResource>()
        .expect("TestResource should be present after sync");
    assert_eq!(stored.val, 55);
}

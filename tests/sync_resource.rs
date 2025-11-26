use bevy::prelude::*;
use bevy_tiled_display::*;

#[derive(Resource, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
struct MockResource {
    pub val: i32,
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

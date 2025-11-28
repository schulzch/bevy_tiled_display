use bevy::{
    input::InputSystem,
    prelude::*,
    render::camera::SubCameraView,
    window::{PrimaryWindow, WindowResolution},
};
use bincode;
use serde::Deserialize;
use std::any::TypeId;
use std::path::Path;
use std::sync::Arc;

use crate::sync::*;

pub struct TiledDisplayPlugin {
    /// Optional factory function that produces a TiledDisplay configuration.
    tiled_display_factory: Box<dyn Fn() -> TiledDisplay + Send + Sync + 'static>,
    /// Identity of this machine in the tiled display configuration.
    identity: String,
    /// Factory that will be invoked on the main thread during `Plugin::build`
    /// to construct a backend instance. The factory must be `Send + Sync`
    /// so the plugin itself remains thread-safe; the produced backend may be
    /// non-`Send`/`Sync` and will be inserted as a non-send resource.
    sync_factory: Box<dyn Fn() -> Box<dyn SyncBackend> + Send + Sync + 'static>,
}

#[derive(Resource, Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct TiledDisplay {
    #[serde(default, deserialize_with = "wrapped_vec")]
    pub machines: Vec<Machine>,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

impl TiledDisplay {
    pub fn size(&self) -> UVec2 {
        UVec2::new(self.width, self.height)
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename = "Machine", rename_all = "PascalCase")]
pub struct Machine {
    pub identity: String,
    #[serde(default, deserialize_with = "wrapped_vec")]
    pub tiles: Vec<Tile>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum StereoChannel {
    Left,
    Right,
}

#[derive(Resource, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct Tile {
    pub name: String,
    pub stereo_channel: StereoChannel,
    pub left_offset: i32,
    pub top_offset: i32,
    pub window_left: i32,
    pub window_top: i32,
    pub window_width: u32,
    pub window_height: u32,
}

impl Tile {
    pub fn offset(&self) -> Vec2 {
        Vec2::new(self.left_offset as f32, self.top_offset as f32)
    }
    pub fn size(&self) -> UVec2 {
        UVec2::new(self.window_width, self.window_height)
    }
}

/// Custom deserializer to convert a wrapped vector, e.g., the XML structure:
/// <Machines>
///   <Machine>...</Machine>
///   <Machine>...</Machine>
/// </Machines>
/// into a plain Vec<Machine>.
fn wrapped_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(bound = "T: serde::Deserialize<'de>")]
    struct Wrapper<T> {
        #[serde(rename = "$value", default)]
        items: Vec<T>,
    }

    let wrapper = Wrapper::<T>::deserialize(deserializer)?;
    Ok(wrapper.items)
}

impl Default for TiledDisplayPlugin {
    fn default() -> Self {
        Self {
            tiled_display_factory: Box::new(|| TiledDisplay::default()),
            identity: TiledDisplayPlugin::hostname(),
            sync_factory: Box::new(|| SyncBackends::Auto.build().unwrap()),
        }
    }
}

impl TiledDisplayPlugin {
    /// Find a machine with matching identity, and grab its first tile.
    fn select_tile(tiled_display: &TiledDisplay, identity: &str) -> Option<Tile> {
        let selected_machine = tiled_display
            .machines
            .iter()
            .find(|m| m.identity == *identity)
            .cloned();

        let selected_tile = selected_machine
            .as_ref()
            .and_then(|m| m.tiles.first().cloned());

        if let Some(machine) = &selected_machine {
            if let Some(tile) = selected_tile.as_ref() {
                info!(
                    identity = machine.identity,
                    tile = ?tile,
                    "Selected machine and tile"
                );
            } else {
                warn!(identity = machine.identity, "Missing tile for machine");
            }
        } else {
            warn!(
                identity = identity,
                "Missing machine for identity; skipping"
            );
        }
        selected_tile
    }

    /// Get hostname of the machine.
    fn hostname() -> String {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_default()
    }

    /// Create a new plugin with default settings.
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the tiled display XML config path.
    pub fn with_config<P: AsRef<Path>>(mut self, config: P) -> Self {
        let xml_data = match std::fs::read_to_string(&config) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    "Failed to read tiled display config from {}: {}",
                    config.as_ref().display(),
                    e
                );
                return self;
            }
        };

        let parsed = match quick_xml::de::from_str::<TiledDisplay>(&xml_data) {
            Ok(td) => td,
            Err(e) => {
                error!(
                    "Failed to parse tiled display config from {}: {}",
                    config.as_ref().display(),
                    e
                );
                return self;
            }
        };

        // Store a factory that will produce the parsed `TiledDisplay` on the
        // main thread during `Plugin::build`.
        self.tiled_display_factory = Box::new(move || parsed.clone());
        self
    }

    /// Set a factory that will be called on the main thread during `build`
    /// to produce the `TiledDisplay` configuration.
    pub fn with_tiled_display_factory<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> TiledDisplay + Send + Sync + 'static,
    {
        self.tiled_display_factory = Box::new(factory);
        self
    }

    /// Set the identity for this machine. An empty identity will use the hostname.
    pub fn with_identity<S: Into<String>>(mut self, identity: S) -> Self {
        let id = identity.into();
        if id.is_empty() {
            self.identity = TiledDisplayPlugin::hostname();
        } else {
            self.identity = id;
        }
        self
    }

    /// Set the synchronization backend to use.
    pub fn with_sync(mut self, sync: SyncBackends) -> Self {
        let backend_choice = sync;
        self.sync_factory = Box::new(move || match backend_choice.build() {
            Ok(b) => {
                info!("sync backend initialized (rank: {})", b.rank());
                b
            }
            Err(e) => {
                error!("Failed to initialize sync backend: {}", e);
                // Fall back to Auto backend if initialization failed.
                SyncBackends::Auto.build().unwrap()
            }
        });

        self
    }

    /// Set the synchronization backend via a factory. The factory will be
    /// called on the main thread during `Plugin::build` to construct the
    /// backend instance. The factory must be `Send + Sync` so the plugin
    /// remains thread-safe.
    pub fn with_sync_backend<F>(mut self, factory: F) -> Self
    where
        F: Fn() -> Box<dyn SyncBackend> + Send + Sync + 'static,
    {
        self.sync_factory = Box::new(factory);
        self
    }
}

impl Plugin for TiledDisplayPlugin {
    fn build(&self, app: &mut App) {
        // Produce the tiled display configuration using the configured
        // factory. The factory is invoked on the main thread during `build`.
        let tiled_display = (self.tiled_display_factory)();

        if let Some(tile) = TiledDisplayPlugin::select_tile(&tiled_display, &self.identity) {
            app.insert_resource(tile);
        };

        let sync_backend: Box<dyn SyncBackend> = (self.sync_factory)();
        info!("sync backend initialized (rank: {})", sync_backend.rank());

        // Insert tiled display resource and other resources.
        app.insert_resource(tiled_display.clone())
            .insert_resource(TileSyncRegistry::new())
            .insert_non_send_resource(sync_backend)
            .add_systems(Startup, tiled_window_start_system)
            .add_systems(
                PreUpdate,
                (
                    tiled_camera_hook_system,
                    tiled_ui_hook_system,
                    tiled_sync_resources_system.after(InputSystem),
                ),
            )
            .add_systems(Last, tiled_frame_barrier_system);
    }
}

/// Adjusts the window position and size.
fn tiled_window_start_system(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    tile: Res<Tile>,
) {
    let position = IVec2::new(tile.window_left, tile.window_top);
    window.position = WindowPosition::At(position);
    window.resolution = WindowResolution::new(tile.window_width as f32, tile.window_height as f32)
        .with_scale_factor_override(1.0);
    window.decorations = false;
}

/// Sets `SubCameraView` for all cameras.
fn tiled_camera_hook_system(
    mut cameras: Query<&mut Camera, Added<Camera>>,
    tiled_display: Res<TiledDisplay>,
    tile: Res<Tile>,
) {
    for mut camera in cameras.iter_mut() {
        camera.sub_camera_view = Some(SubCameraView {
            full_size: tiled_display.size(),
            offset: tile.offset(),
            size: tile.size(),
        });
    }
}

/// Shifts all UI root nodes.
fn tiled_ui_hook_system(
    mut root_nodes: Query<&mut Node, (Added<Node>, Without<ChildOf>)>,
    tile: Res<Tile>,
) {
    // TODO: This approach directly shifts all UI root nodes by the tile offset, which is hacky.
    let offset = tile.offset();
    for mut root_node in root_nodes.iter_mut() {
        if root_node.position_type == PositionType::Absolute {
            if let Val::Px(left) = root_node.left {
                root_node.left = Val::Px(left - offset.x);
            }
            if let Val::Px(top) = root_node.top {
                root_node.top = Val::Px(top - offset.y);
            }
        }
    }
}

/// Broadcasts all resources registered in `TileSyncRegistry`.
fn tiled_sync_resources_system(world: &mut World) {
    let Some(registry) = world.get_resource::<TileSyncRegistry>() else {
        return;
    };
    let Some(sync) = world.get_non_send_resource::<Box<dyn SyncBackend>>() else {
        return;
    };

    // Clone the serializer/deserializer closures out of the registry so we
    // don't hold an immutable borrow of the World while later mutably
    // borrowing it to insert deserialized resources.
    let local_entries: Vec<_> = registry
        .entries
        .iter()
        .map(|e| (e.type_id, e.serializer.clone(), e.deserializer.clone()))
        .collect();

    // Serialize all registered resources (using the cloned serializer Arcs).
    let mut buffer = Vec::<u8>::new();
    for (type_id, serializer, _) in local_entries.iter() {
        match serializer(world) {
            Some(bytes) => buffer.extend(bytes),
            None => warn!(type_id = ?type_id, "Sync resource not present in world"),
        }
    }

    if let Err(e) = sync.broadcast(&mut buffer) {
        error!(error = ?e, "Broadcast failed or timed out. Exiting.");
        std::process::exit(1);
    }

    // Deserialize received bytes back into resources. We create a Cursor so
    // each deserializer can read from the shared byte stream in registration
    // order.
    if !buffer.is_empty() {
        let mut cursor = std::io::Cursor::new(buffer.as_slice());
        for (type_id, _, deserializer) in local_entries.iter() {
            let ok = deserializer(world, &mut cursor);
            if !ok {
                warn!(type_id = ?type_id, "Failed to apply synced resource");
            }
        }
    }
}

/// Blocks at the end of a frame until all tiled displays reach this point.
fn tiled_frame_barrier_system(sync: NonSend<Box<dyn SyncBackend>>) {
    if let Err(e) = sync.barrier() {
        error!(error = ?e, "Barrier failed or timed out. Exiting.");
        std::process::exit(1);
    }
}

struct SyncEntry {
    type_id: TypeId,
    // A boxed serializer that receives a `&mut World` and returns
    // an owned `Vec<u8>` containing the bincode serialization of
    // the resource, or `None` if the resource is not present.
    serializer: Arc<dyn for<'w> Fn(&'w World) -> Option<Vec<u8>> + Send + Sync>,
    // A boxed deserializer that receives a `&mut World` and a cursor over
    // the incoming bytes. It should attempt to read its resource from the
    // cursor (advancing its position) and insert the resource into the
    // world. Returns `true` on success, `false` on failure.
    deserializer: Arc<dyn Fn(&mut World, &mut std::io::Cursor<&[u8]>) -> bool + Send + Sync>,
}

#[derive(Resource)]
pub struct TileSyncRegistry {
    entries: Vec<SyncEntry>,
}

impl TileSyncRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a resource type for synchronization. This stores only the
    /// `TypeId` (no resource value) so callers can both register and then
    /// insert the actual resource into the `App` without moving it here.
    /// Register a resource type for synchronization. The resource type
    /// must implement `serde::Serialize`. We store a serializer closure
    /// that will be invoked later by the sync system to fetch the
    /// resource from the `World` and produce bincode bytes.
    pub fn insert<R>(&mut self)
    where
        R: Resource + 'static + Send + Sync + serde::Serialize + serde::de::DeserializeOwned,
    {
        let id = TypeId::of::<R>();
        if self.entries.iter().any(|e| e.type_id == id) {
            return;
        }

        let serializer: Arc<dyn for<'w> Fn(&'w World) -> Option<Vec<u8>> + Send + Sync> =
            Arc::new(|world: &World| {
                world
                    .get_resource::<R>()
                    .and_then(|r| bincode::serialize(r).ok())
            });

        let deserializer: Arc<
            dyn Fn(&mut World, &mut std::io::Cursor<&[u8]>) -> bool + Send + Sync,
        > = Arc::new(|world: &mut World, cursor: &mut std::io::Cursor<&[u8]>| {
            match bincode::deserialize_from::<_, R>(cursor) {
                Ok(res) => {
                    world.insert_resource::<R>(res);
                    true
                }
                Err(e) => {
                    warn!(
                        "Failed to deserialize sync resource {}: {}",
                        std::any::type_name::<R>(),
                        e
                    );
                    false
                }
            }
        });

        self.entries.push(SyncEntry {
            type_id: id,
            serializer,
            deserializer,
        });
    }

    /// Check whether a resource type is registered for synchronization.
    pub fn contains<R: 'static>(&self) -> bool {
        let id = TypeId::of::<R>();
        self.entries.iter().any(|e| e.type_id == id)
    }

    /// Register a resource type in the `TileSyncRegistry` attached to `app`.
    pub fn register_resource_type_in_app<
        R: Resource + serde::Serialize + Send + Sync + serde::de::DeserializeOwned + 'static,
    >(
        app: &mut App,
    ) {
        if let Some(mut registry) = app.world_mut().get_resource_mut::<TileSyncRegistry>() {
            registry.insert::<R>();
        } else {
            warn!(
                "Resource cannot be registered for synchronization {} (plugin not added?)",
                std::any::type_name::<R>()
            );
        }
    }
}

pub trait SyncResourceAppExt {
    fn init_sync_resource<
        R: Resource + Default + serde::Serialize + Send + Sync + serde::de::DeserializeOwned + 'static,
    >(
        &mut self,
    ) -> &mut Self;

    fn insert_sync_resource<
        R: Resource + serde::Serialize + Send + Sync + serde::de::DeserializeOwned + 'static,
    >(
        &mut self,
        resource: R,
    ) -> &mut Self;
}

impl SyncResourceAppExt for App {
    /// Inserts the synchronized [`Resource`] into the app.
    ///
    /// See `bevy::app::App::insert_resource` for details.
    fn insert_sync_resource<
        R: Resource + serde::Serialize + Send + Sync + serde::de::DeserializeOwned + 'static,
    >(
        &mut self,
        resource: R,
    ) -> &mut Self {
        TileSyncRegistry::register_resource_type_in_app::<R>(self);
        self.insert_resource(resource)
    }

    /// Initializes the synchronized [`Resource`] into the app.
    ///
    /// See `bevy::app::App::init_resource` for details.
    fn init_sync_resource<
        R: Resource + Default + serde::Serialize + Send + Sync + serde::de::DeserializeOwned + 'static,
    >(
        &mut self,
    ) -> &mut Self {
        TileSyncRegistry::register_resource_type_in_app::<R>(self);
        self.init_resource::<R>()
    }
}

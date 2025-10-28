use std::path::{Path, PathBuf};

use bevy::{
    prelude::*,
    render::camera::SubCameraView,
    window::{PrimaryWindow, WindowResolution},
};
use serde::Deserialize;

use crate::sync::*;

#[derive(Clone)]
pub struct TiledDisplayPlugin {
    /// Path to the tiled display XML configuration file.
    pub config: PathBuf,
    /// Identity of this machine in the tiled display configuration.
    pub identity: String,
    /// Which synchronization backend to use for frame coordination.
    pub sync: SyncBackends,
}

#[derive(Resource, Deserialize, Debug, Clone)]
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
            config: PathBuf::new(),
            identity: TiledDisplayPlugin::hostname(),
            sync: SyncBackends::Auto,
        }
    }
}

impl TiledDisplayPlugin {
    fn select_sync(&self) -> Option<Box<dyn SyncBackend>> {
        match self.sync {
            SyncBackends::Auto => {
                #[cfg(feature = "mpi")]
                {
                    Some(Box::new(MpiSync))
                }
                #[cfg(not(feature = "mpi"))]
                {
                    None
                }
                // Auto falls back to no-op.
            }
            SyncBackends::Mpi => {
                #[cfg(feature = "mpi")]
                {
                    Some(Box::new(MpiSync))
                }
                #[cfg(not(feature = "mpi"))]
                {
                    error!("Requested MPI but crate built without 'mpi' feature");
                    None
                }
            }
        }
    }

    /// Find a machine with matching identiy, and grab its first tile.
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

    /// Parse the tiled display configuration from XML.
    fn load<P: AsRef<Path>>(config: P) -> Result<TiledDisplay, Box<dyn std::error::Error>> {
        let xml_data = std::fs::read_to_string(config)?;
        let tiled_display = quick_xml::de::from_str::<TiledDisplay>(&xml_data)?;
        Ok(tiled_display)
    }

    /// Get hostname of the machine.
    fn hostname() -> String {
        hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_default()
    }
}

impl Plugin for TiledDisplayPlugin {
    fn build(&self, app: &mut App) {
        let tiled_display = Self::load(&self.config).unwrap();
        if let Some(tile) = TiledDisplayPlugin::select_tile(&tiled_display, &self.identity) {
            app.insert_resource(tile);
        };
        // Load tiled display and hostname once, store as resource for easy access.
        app.insert_resource(tiled_display)
            .add_systems(Startup, tiled_window_start_system)
            .add_systems(PreUpdate, (tiled_camera_hook_system, tiled_ui_hook_system));

        // Wire synchronization backend.
        if let Some(sync) = self.select_sync() {
            sync.setup(app);
        }
    }
}

/// Adjusts the window position and size.
fn tiled_window_start_system(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    tile: Res<Tile>,
) {
    let position = IVec2::new(tile.window_left as i32, tile.window_top as i32);
    window.position = WindowPosition::At(position);
    window.resolution = WindowResolution::new(tile.window_width as f32, tile.window_height as f32)
        .with_scale_factor_override(1.0);
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
    //XXX: this approach is quite hacky but works for now.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_vvand20_xml() {
        let td = TiledDisplayPlugin::load("configs/vvand20.xml").expect("load xml");

        // Basic sanity checks from the provided file
        assert_eq!(td.name, "VVand");
        assert_eq!(td.width, 10800);
        assert_eq!(td.height, 4096);

        // Expect 20 machines (keshiki01..keshiki20)
        assert_eq!(td.machines.len(), 20);
        assert_eq!(td.machines.first().unwrap().identity, "keshiki01");
        assert_eq!(td.machines.last().unwrap().identity, "keshiki20");
    }
}

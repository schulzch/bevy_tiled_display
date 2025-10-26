use bevy::{
    prelude::*,
    render::camera::{CameraProjection, Viewport},
    window::{PrimaryWindow, WindowResolution},
};
use serde::Deserialize;

use crate::sync::*;

#[derive(Clone)]
pub struct TiledDisplayPlugin {
    /// Path to the tiled display XML configuration file.
    pub path: String,
    /// Identity of this machine in the tiled display configuration.
    pub identity: String,
    /// Which synchronization backend to use for frame coordination.
    pub sync: SyncBackends,
}

#[derive(Resource, Deserialize, Debug, Clone)]
pub struct TiledDisplay {
    #[serde(rename = "Machines")]
    pub machines: Machines,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Width")]
    pub width: u32,
    #[serde(rename = "Height")]
    pub height: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Machines {
    #[serde(rename = "Machine", default)]
    pub machine: Vec<Machine>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Machine {
    #[serde(rename = "Identity")]
    pub identity: String,
    #[serde(rename = "Tiles")]
    pub tiles: Option<Tiles>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Tiles {
    #[serde(rename = "Tile", default)]
    pub tile: Vec<Tile>,
}

#[derive(Resource, Deserialize, Debug, Clone)]
pub struct Tile {
    #[serde(rename = "LeftOffset")]
    pub left_offset: u32,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "StereoChannel")]
    pub stereo_channel: String,
    #[serde(rename = "TopOffset")]
    pub top_offset: u32,
    #[serde(rename = "WindowHeight")]
    pub window_height: u32,
    #[serde(rename = "WindowWidth")]
    pub window_width: u32,
    #[serde(rename = "WindowLeft")]
    pub window_left: u32,
    #[serde(rename = "WindowTop")]
    pub window_top: u32,
}

impl Default for TiledDisplayPlugin {
    fn default() -> Self {
        Self {
            path: String::new(),
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

    fn select_tile(tiled_display: &TiledDisplay, identity: &str) -> Option<Tile> {
        // Try to find a machine that matches our hostname, and grab its first tile.
        let selected_machine = tiled_display
            .machines
            .machine
            .iter()
            .find(|m| m.identity == *identity)
            .cloned();

        let selected_tile = selected_machine
            .as_ref()
            .and_then(|m| m.tiles.as_ref())
            .and_then(|t| t.tile.first().cloned());

        if let Some(machine) = &selected_machine {
            if let Some(tile) = selected_tile.as_ref() {
                info!(
                    "Selected machine '{}' and tile '{}'",
                    machine.identity, tile.name
                );
                info!("Tile size: {}x{}", tile.window_width, tile.window_height);
            } else {
                warn!("Selected machine '{}' but no tiles found", machine.identity);
            }
        } else {
            warn!(
                "No matching machine found for identity '{}'; skipping",
                identity
            );
        }
        selected_tile
    }

    /// Parse the tiled display configuration from XML.
    fn load(path: &str) -> Result<TiledDisplay, Box<dyn std::error::Error>> {
        let xml_data = std::fs::read_to_string(path)?;
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
        let tiled_display = Self::load(&self.path).unwrap();
        if let Some(tile) = TiledDisplayPlugin::select_tile(&tiled_display, &self.identity) {
            app.insert_resource(tile);
        };
        // Load tiled display and hostname once, store as resource for easy access.
        app.insert_resource(tiled_display)
            .add_systems(Startup, tiled_window_start_system)
            .add_systems(Update, tiled_viewport_hook_system);

        // Wire synchronization backend.
        if let Some(sync) = self.select_sync() {
            sync.setup(app);
        }
    }
}

fn tiled_window_start_system(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    tile: Res<Tile>,
) {
    let position = IVec2::new(tile.window_left as i32, tile.window_top as i32);
    window.position = WindowPosition::At(position);
    window.resolution = WindowResolution::new(tile.window_width as f32, tile.window_height as f32);
}

fn tiled_viewport_hook_system(
    mut cameras: Query<(&mut Camera, &mut Projection), Added<Camera>>,
    tile: Res<Tile>,
) {
    let physical_position = UVec2::new(tile.left_offset, tile.top_offset);
    let physical_size = UVec2::new(tile.window_width, tile.window_height);

    for (mut camera, mut projection) in cameras.iter_mut() {
        camera.viewport = Some(Viewport {
            physical_position,
            physical_size,
            ..default()
        });
        projection.update(physical_size.x as f32, physical_size.y as f32);
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
        assert_eq!(td.machines.machine.len(), 20);
        assert_eq!(td.machines.machine.first().unwrap().identity, "keshiki01");
        assert_eq!(td.machines.machine.last().unwrap().identity, "keshiki20");
    }
}

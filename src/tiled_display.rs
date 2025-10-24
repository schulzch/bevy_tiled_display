use bevy::prelude::*;
use serde::Deserialize;

use crate::sync::*;

#[derive(Clone)]
pub struct TiledDisplayPlugin {
    pub path: String,
    /// Which synchronization backend to use for frame coordination.
    pub sync: SyncBackends,
}

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct Machines {
    #[serde(rename = "Machine", default)]
    pub machine: Vec<Machine>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Machine {
    #[serde(rename = "Identity")]
    pub identity: String,
    #[serde(rename = "Tiles")]
    pub tiles: Option<Tiles>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Tiles {
    #[serde(rename = "Tile", default)]
    pub tile: Vec<Tile>,
}

#[derive(Debug, Deserialize, Clone)]
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

/// Handy, tiled display metadata you can use anywhere.
#[derive(Resource, Debug, Clone)]
pub struct TiledDisplayMeta {
    pub tiled_display: TiledDisplay,
    pub hostname: String,
}

impl TiledDisplayPlugin {
    /// Create a new plugin that uses the default sync selection (Auto).
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            sync: SyncBackends::Auto,
        }
    }

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

    /// Parse the tiled display configuration from XML.
    fn load(path: &str) -> Result<TiledDisplay, Box<dyn std::error::Error>> {
        let xml_data = std::fs::read_to_string(path)?;
        let tiled_display: TiledDisplay = quick_xml::de::from_str(&xml_data)?;
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
        // Store as resource for easy access.
        app.insert_resource(TiledDisplayMeta {
            hostname: Self::hostname(),
            tiled_display: Self::load(&self.path).unwrap(),
        });

        // Wire synchronization backend.
        if let Some(sync) = self.select_sync() {
            sync.setup(app);
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
        assert_eq!(td.machines.machine.len(), 20);
        assert_eq!(td.machines.machine.first().unwrap().identity, "keshiki01");
        assert_eq!(td.machines.machine.last().unwrap().identity, "keshiki20");
    }
}

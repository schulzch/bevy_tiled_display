use std::path::Path;

use bevy::prelude::*;
use serde::Deserialize;

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

impl TiledDisplay {
    /// Find a machine with matching identity, and grab its first tile.
    pub fn find_tile(&self, identity: &str) -> Option<Tile> {
        let selected_machine = self
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

    pub fn load<P: AsRef<Path>>(config: P) -> Result<TiledDisplay, Box<dyn std::error::Error>> {
        let xml_data = std::fs::read_to_string(config)?;
        let tiled_display = quick_xml::de::from_str::<TiledDisplay>(&xml_data)?;
        Ok(tiled_display)
    }
}

use bevy::prelude::*;
use bevy_tiled_display::*;
use clap::Parser;

const SHAPE_WIDTH: f32 = 75.0;
const SHAPE_HEIGHT: f32 = 100.0;
const SHAPE_SPACING_X: f32 = SHAPE_WIDTH * 2.0;
const SHAPE_SPACING_Y: f32 = SHAPE_HEIGHT / 2.0;
const SHAPE_FREQUENCY_MIN: f32 = 1.0 / 32.0;
const SHAPE_FREQUENCY_MAX: f32 = 1.0 / 4.0;

/// Horizontal speed in pixels per second.
#[derive(Component)]
struct SpeedX(f32);

#[derive(Parser)]
#[command(version)]
struct Args {
    #[arg(
        short,
        long,
        help = "XML configuration file",
        default_value_t = String::from("configs/vvand20.xml")
    )]
    path: String,
    #[arg(
        short,
        long,
        help = "Identity of this machine, empty defaults to hostname",
        default_value_t = String::new()
    )]
    identity: String,
}

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let Args { path, identity } = Args::parse();
    let mut tiled_display_plugin = TiledDisplayPlugin { path, ..default() };
    if !identity.is_empty() {
        tiled_display_plugin.identity = identity.clone();
    }

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Bevy Tiled Display v{} Demo on {}", version, identity),
                    ..default()
                }),
                ..default()
            }),
            tiled_display_plugin,
        ))
        .add_systems(Startup, setup_shapes)
        .add_systems(Update, move_shapes)
        .run();
}

fn setup_shapes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    tiled_display: Res<TiledDisplay>,
) {
    commands.spawn(Camera2d);

    let shape = meshes.add(Rhombus::new(SHAPE_WIDTH, SHAPE_HEIGHT));

    // Add moving shapes based on the display size with speed increasing from top to bottom.
    let width = tiled_display.width as f32;
    let height = tiled_display.height as f32;
    let rows =
        (((height + SHAPE_SPACING_Y) / (SHAPE_HEIGHT + SHAPE_SPACING_Y)).floor() as usize).max(2);
    let cols =
        (((width + SHAPE_SPACING_X) / (SHAPE_WIDTH + SHAPE_SPACING_X)).floor() as usize).max(2);
    assert!(rows > 1);
    assert!(cols > 1);
    for row in 0..rows {
        let t_row = row as f32 / (rows as f32 - 1.0);
        let frequency = SHAPE_FREQUENCY_MIN + t_row * (SHAPE_FREQUENCY_MAX - SHAPE_FREQUENCY_MIN);
        for col in 0..cols {
            let t_col = col as f32 / (cols as f32 - 1.0);
            let material = materials.add(Color::hsl(360.0 * t_col, 0.95, 0.7));
            commands.spawn((
                SpeedX(frequency * width),
                Transform::from_xyz((t_col - 0.5) * width, (0.5 - t_row) * height, 0.0),
                Mesh2d(shape.clone()),
                MeshMaterial2d(material),
            ));
        }
    }
}

fn move_shapes(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &SpeedX)>,
    tiled_display: Res<TiledDisplay>,
) {
    // Move shapes horizontally by their pixel speed. When a shape exits the right
    // edge, wrap it to the left edge so it continuously iterates left->right.
    let delta = time.delta().as_secs_f32();
    let width = tiled_display.width as f32;
    let half_width = width / 2.0;
    for (mut transform, speed) in query.iter_mut() {
        transform.translation.x =
            (transform.translation.x + speed.0 * delta + half_width).rem_euclid(width) - half_width;
    }
}

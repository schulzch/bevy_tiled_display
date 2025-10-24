# Bevy Tiled Display

**Highly experimental and work in progress. Use at your own risk!**

This repository contains a Bevy plugin for a large multi-machine tiled display support.

## Quick start

Build the project:

```sh
cargo build
```

Run tests:

```sh
cargo test
```

## Example

```rust
app.add_plugins((
    #[cfg(feature = "mpi")]
    MpiPlugin::default(),
    TiledDisplayPlugin {
        path: "configs/vvand20.xml".to_string(),
    },
));
```

The `TiledDisplayPlugin` exposes a `TiledDisplayMeta` resource.

## License

No license specified — contact the repository owner if you need permission to use the code.

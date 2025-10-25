# Bevy Tiled Display

A Bevy plugin for multi-machine tiled displays. Useful for driving synchronized windows across multiple machines or processes to form a single large display.

**This crate is highly experimental!**

## Quick start

Add the crate to your `Cargo.toml`.

```toml
[dependencies]
bevy_tiled_display = { version = "0.16.0", features = ["mpi"] }
```

- The `mpi` feature enables multi-node/multi-process support using your system MPI implementation (OpenMPI, MPICH, MS-MPI, etc.). It should work out of the box if you have both, the MPI SDK and Clang (required by `rust-bindgen`) installed. If not, see common issues below.

### Run the demo (single machine)

```sh
cargo run --example demo -- --identity "keshiki01"
```

### Run the demo with MPI support

```sh
cargo build --release --features mpi --example demo
# Start two processes on this machine; adapt to your cluster as needed.
mpiexec -n 1 ./target/release/examples/demo.exe --identity "keshiki01" : -n 1 ./target/release/examples/demo.exe --identity "keshiki02"
```

## Example

Here is a minimal example, showing how to register the plugin in your Bevy app:

```rust
use bevy_tiled_display::*;

fn main() {
    // ...your code here...
    app.add_plugins((TiledDisplayPlugin {
        path: "configs/vvand20.xml".into(),
        identity: "keshiki01".into(), // defaults to machine hostname
        ..default()
    },));
    // ...your code here...
}
```

See `examples/demo.rs` for a more complete example.

## Common issues

- When building with `mpi`, you might run into issues with `rust-bindgen` and `clang`. See [rust-bindgen requirements](https://rust-lang.github.io/rust-bindgen/requirements.html) for platform-specific instructions. The Clang version provided by the Visual Studio Installer is usually outdated (tldr: run `winget install LLVM.LLVM` and `setx LIBCLANG_PATH "C:\Program Files\LLVM\"`, and then, from a `Developer Command Prompt` invoke cargo).
- When building with `mpi`, you might run into linker errors: ensure your system MPI is installed and that environment variables are set correctly. See the [mpi crate](https://crates.io/crates/mpi) for platform-specific build and runtime notes.
- Early termination or missing display: verify your XML config (see `configs/`) and machine identities. The plugin logs which machine and tile is used.

## Tests

Run the test suite:

```sh
cargo test
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

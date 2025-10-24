# Bevy Tiled Display

**Highly experimental and work in progress. Use at your own risk!**

This repository contains a Bevy plugin for a large multi-machine tiled display support.

## Quick start

Build the project:

```sh
cargo build
```

Compile with MPI support in release mode:

```sh
cargo build --release --features mpi
```

Note: MPI support requires a system MPI implementation (for example OpenMPI or MPICH) to be installed on your machine. To run the built binary across multiple nodes or processes use `mpirun`/`mpiexec` (or your cluster's job launcher).

Run tests:

```sh
cargo test
```

## Example

```sh
cargo run --example demo
```

## License

No license specified — contact the repository owner if you need permission to use the code.

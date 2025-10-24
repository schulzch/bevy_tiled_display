use super::SyncBackend;
use bevy::prelude::*;
use mpi::traits::*;

#[derive(Clone)]
pub struct MpiSync;

/// Non-send MPI context (must live on the main thread).
/// Holding `Universe` ensures MPI is finalized on drop.
struct MpiContext {
    universe: mpi::environment::Universe,
}

impl SyncBackend for MpiSync {
    fn setup(&self, app: &mut App) {
        // Initialize MPI once during app construction so we can insert a NonSend resource.
        // If MPI is already initialized (e.g., launched under another runtime), we handle it gracefully.
        let universe = match mpi::initialize() {
            Ok(u) => u,
            Err(mpi::environment::AlreadyInitializedError) => {
                // Safe to attach to the already-initialized runtime.
                mpi::environment::Universe::new()
            }
        };

        // Query rank/size immediately; store meta as a normal Resource.
        let world = universe.world();
        let rank = world.rank();
        let size = world.size();

        app.insert_non_send_resource(MpiContext { universe })
            .add_systems(Last, mpi_frame_barrier_system);
    }
}

/// Blocks at the end of a frame until all MPI ranks reach this point.
fn mpi_frame_barrier_system(ctx: NonSend<MpiContext>) {
    // Borrow the world communicator for the duration of this system.
    // We don't store it; we just use it and drop it, so lifetimes are simple and safe.
    let world = ctx.universe.world();
    world.barrier();
}

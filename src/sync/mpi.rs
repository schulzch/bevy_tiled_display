use super::SyncBackend;
use bevy::prelude::*;
use mpi::traits::*;

#[derive(Clone)]
pub struct MpiSync;

/// Non-send MPI context (must live on the main thread).
/// Holding `Universe` ensures MPI is finalized on drop.
struct MpiContext {
    universe: Option<mpi::environment::Universe>,
}

impl SyncBackend for MpiSync {
    fn setup(&self, app: &mut App) {
        // Initialize MPI once during app construction.
        // `Some` when this call initialized MPI and returns an owned `Universe`.
        // `None` when MPI was already initialized by some other runtime.
        let universe = mpi::initialize();
        let world = match &universe {
            Some(universe) => universe.world(),
            None => mpi::topology::SimpleCommunicator::world(),
        };
        let rank = world.rank();
        let size = world.size();
        info!("MPI initialized. Rank: {} Size: {}", rank, size);

        app.insert_non_send_resource(MpiContext { universe })
            .add_systems(Last, mpi_frame_barrier_system);
    }
}

/// Blocks at the end of a frame until all MPI ranks reach this point.
fn mpi_frame_barrier_system(ctx: NonSend<MpiContext>) {
    // Borrow the world communicator for the duration of this system.
    let world = match &ctx.universe {
        Some(universe) => universe.world(),
        None => mpi::topology::SimpleCommunicator::world(),
    };
    world.barrier();
}

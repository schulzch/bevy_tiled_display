use super::SyncBackend;
use bevy::prelude::*;
use mpi::environment::Universe;
use mpi::request::Request;
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct MpiSync;

/// Non-send MPI context (must live on the main thread).
/// Holding `Universe` ensures MPI is finalized on drop.
struct MpiContext {
    universe: Option<Universe>,
}

impl SyncBackend for MpiSync {
    fn setup(&self, app: &mut App) {
        // Initialize MPI once during app construction.
        // `Some` when this call initialized MPI and returns an owned `Universe`.
        // `None` when MPI was already initialized by some other runtime.
        let universe = mpi::initialize();
        let world = get_world(&universe);

        app.insert_non_send_resource(MpiContext { universe })
            .add_systems(Last, mpi_frame_barrier_system);

        info!("Rank {} initialized (size {})", world.rank(), world.size());
    }
}

fn get_world(universe: &Option<Universe>) -> SimpleCommunicator {
    match universe {
        Some(universe) => universe.world(),
        None => SimpleCommunicator::world(),
    }
}

fn busy_barrier(world: &impl Communicator, timeout: Duration) -> bool {
    let mut request: Request<()> = world.immediate_barrier();
    let start = Instant::now();
    loop {
        match request.test() {
            Ok(_) => return true,
            Err(r) => request = r,
        }
        if start.elapsed() > timeout {
            request.cancel();
            return false;
        }
        // Busy-wait hint
        std::hint::spin_loop();
    }
}

/// Blocks at the end of a frame until all MPI ranks reach this point.
fn mpi_frame_barrier_system(ctx: NonSend<MpiContext>) {
    let world = get_world(&ctx.universe);
    if !busy_barrier(&world, Duration::from_millis(200)) {
        error!("Barrier failed or timed out. Exiting.");
        std::process::exit(1);
    }
}

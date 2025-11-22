use super::SyncBackend;
use bevy::prelude::*;
use mpi::environment::Universe;
use mpi::request::Request;
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;
use std::time::{Duration, Instant};

/// Holding `Universe` ensures MPI is finalized on drop.
#[derive(Clone)]
pub struct MpiSync {
    universe: Option<Universe>,
}

impl MpiSync {
    fn world(&self) -> SimpleCommunicator {
        match self.universe {
            Some(universe) => universe.world(),
            None => SimpleCommunicator::world(),
        }
    }
}

impl SyncBackend for MpiSync {
    fn new() -> Self {
        // Initialize MPI once during app construction.
        // `Some` when this call initialized MPI and returns an owned `Universe`.
        // `None` when MPI was already initialized by some other runtime.
        let sync = MpiSync { universe };
        let world = sync.world();
        info!("Rank {} initialized (size {})", world.rank(), world.size());
        sync
    }

    fn barrier(&self) {
        /// Blocks at the end of a frame until all MPI ranks reach this point.
        let world = world(&ctx.universe);
        if !busy_barrier(&world, Duration::from_millis(200)) {
            error!("Barrier failed or timed out. Exiting.");
            std::process::exit(1);
        }
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

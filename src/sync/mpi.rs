use super::*;
use bevy::prelude::*;
use mpi::environment::Universe;
use mpi::request::Request;
use mpi::topology::SimpleCommunicator;
use mpi::traits::*;
use std::time::{Duration, Instant};

/// A MPI-based synchronization backend.
///
/// Holding `Universe` ensures MPI is finalized on drop.
#[derive(Clone)]
pub struct MpiSync {
    universe: Option<Universe>,
}

impl MpiSync {
    fn new() -> Self {
        // Initialize MPI once during app construction.
        // `Some` when this call initialized MPI and returns an owned `Universe`.
        // `None` when MPI was already initialized by some other runtime.
        let sync = MpiSync { universe };
        let world = sync.world();
        info!("Rank {} initialized (size {})", world.rank(), world.size());
        sync
    }

    fn world(&self) -> SimpleCommunicator {
        match self.universe {
            Some(universe) => universe.world(),
            None => SimpleCommunicator::world(),
        }
    }
}

impl SyncBackend for MpiSync {
    fn barrier(&self) {
        let world = world(&ctx.universe);
        if !busy_barrier(&world, TIMEOUT) {
            error!("Barrier failed or timed out. Exiting.");
            std::process::exit(1);
        }
    }

    fn broadcast(&self, bytes: &[u8]) -> Vec<u8> {
        let world = self.world();
        let root = world.process_at_rank(0);

        // Broadcast length, allocate on non-root ranks, then broadcast bytes.
        let mut len = bytes.len() as u64;
        root.broadcast_into(&mut len);

        let mut buf = if world.rank() == 0 {
            bytes.to_vec()
        } else {
            vec![0u8; len as usize]
        };

        root.broadcast_into(&mut buf[..]);
        buf
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

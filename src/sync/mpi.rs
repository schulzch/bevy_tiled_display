use super::*;
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
    fn rank(&self) -> u32 {
        let rank_i = self.world().rank();
        u32::try_from(rank_i).unwrap_or_else(|_| panic!("MPI rank is negative: {}", rank_i))
    }

    fn barrier(&self) -> Result<(), SyncError> {
        let world = world(&ctx.universe);
        if !busy_barrier(&world, TIMEOUT) {
            return Err(SyncError::Timeout);
        }
        Ok(())
    }

    fn broadcast(&self, data: &[u8]) -> Result<Vec<u8>, SyncError> {
        let world = self.world();
        let root = world.process_at_rank(0);

        // Broadcast length.
        let mut len = data.len() as u64;
        root.broadcast_into(&mut len);

        // Allocate on non-root ranks.
        let mut buf = if world.rank() == 0 {
            data.to_vec()
        } else {
            let size = usize::try_from(len)
                .map_err(|e| SyncError::Error(format!("Cannot convert: {:?}", e)))?;
            vec![0u8; size]
        };

        // Broadcast data.
        root.broadcast_into(&mut buf[..]);
        Ok(buf)
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

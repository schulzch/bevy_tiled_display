use super::*;
use ::mpi::environment::Universe;
use ::mpi::request::Request;
use ::mpi::topology::SimpleCommunicator;
use ::mpi::traits::*;
use std::time::Instant;

/// A MPI-based synchronization backend.
///
/// Holding `Universe` ensures MPI is finalized on drop.
pub struct MpiSync {
    universe: Option<Universe>,
}

impl MpiSync {
    pub fn new() -> Self {
        // Initialize MPI once during app construction.
        // `Some` when this call initialized MPI and returns an owned `Universe`.
        // `None` when MPI was already initialized by some other runtime.
        let universe = ::mpi::initialize();
        Self { universe }
    }

    fn world(&self) -> SimpleCommunicator {
        match self.universe {
            Some(ref universe) => universe.world(),
            None => SimpleCommunicator::world(),
        }
    }
}

impl SyncBackend for MpiSync {
    fn rank(&self) -> u32 {
        self.world()
            .rank()
            .try_into()
            .expect("MPI rank must be non-negative")
    }

    fn world_size(&self) -> u32 {
        self.world().size() as u32
    }

    fn barrier(&self) -> Result<(), SyncError> {
        let mut request: Request<()> = self.world().immediate_barrier();
        let start = Instant::now();
        loop {
            match request.test() {
                Ok(_) => return Ok(()),
                Err(r) => request = r,
            }
            if start.elapsed() > TIMEOUT {
                request.cancel();
                return Err(SyncError::Timeout);
            }
            // Busy-wait hint
            std::hint::spin_loop();
        }
    }

    fn broadcast(&self, data: &mut Vec<u8>) -> Result<(), SyncError> {
        let world = self.world();
        let root = world.process_at_rank(0);

        // Broadcast length.
        let mut len = data.len() as u64;
        root.broadcast_into(&mut len);

        // Resize on non-root ranks.
        if world.rank() != 0 {
            let size = usize::try_from(len).map_err(|e| {
                SyncError::Failed(format!(
                    "Cannot convert broadcast buffer size ({}) to usize: {:?}",
                    len, e
                ))
            })?;
            data.resize(size, 0);
        }

        // Broadcast data.
        root.broadcast_into(&mut data[..]);

        Ok(())
    }
}

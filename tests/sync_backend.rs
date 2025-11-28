use bevy_tiled_display::*;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// In-process test backend that simulates multiple participants.
/// This lets us deterministically test barrier blocking and collective broadcast.
struct MockCoordinator {
    total: usize,
    // barrier state
    state: Mutex<MockState>,
    cv: Condvar,
}

struct MockState {
    barrier_count: usize,
    barrier_gen: usize,
    // broadcast state
    broadcast_gen: usize,
    broadcast_data: Option<Vec<u8>>,
}

#[derive(Clone)]
struct MockSync {
    coord: Arc<MockCoordinator>,
    id: usize,
}

impl MockCoordinator {
    fn new(total: usize) -> Arc<Self> {
        Arc::new(MockCoordinator {
            total,
            state: Mutex::new(MockState {
                barrier_count: 0,
                barrier_gen: 0,
                broadcast_gen: 0,
                broadcast_data: None,
            }),
            cv: Condvar::new(),
        })
    }

    /// Called by each participant when they reach the barrier.
    /// Blocks the caller until all `total` participants have called this method.
    fn barrier(&self) {
        let mut s = self.state.lock().unwrap();
        let cur_gen = s.barrier_gen;
        s.barrier_count += 1;
        if s.barrier_count == self.total {
            // release everyone and advance generation
            s.barrier_count = 0;
            s.barrier_gen = cur_gen.wrapping_add(1);
            self.cv.notify_all();
            return;
        }
        while s.barrier_gen == cur_gen {
            s = self.cv.wait(s).unwrap();
        }
    }

    /// Collective broadcast: one participant (usually id == 0) provides `bytes`.
    /// All participants must call this method; non-root callers should pass an empty slice.
    /// Returns the broadcasted bytes for every caller.
    fn broadcast(&self, id: usize, bytes: &[u8]) -> Vec<u8> {
        let mut s = self.state.lock().unwrap();
        let cur_gen = s.broadcast_gen;
        // If root, set data and advance generation and notify.
        if id == 0 {
            s.broadcast_data = Some(bytes.to_vec());
            s.broadcast_gen = cur_gen.wrapping_add(1);
            self.cv.notify_all();
            // return a copy for root
            return s.broadcast_data.as_ref().unwrap().clone();
        } else {
            // wait until generation advanced
            while s.broadcast_gen == cur_gen {
                s = self.cv.wait(s).unwrap();
            }
            return s.broadcast_data.as_ref().unwrap().clone();
        }
    }
}

impl MockSync {
    fn new(coord: Arc<MockCoordinator>, id: usize) -> Self {
        MockSync { coord, id }
    }
}

impl SyncBackend for MockSync {
    fn rank(&self) -> u32 {
        self.id as u32
    }

    fn barrier(&self) -> Result<(), SyncError> {
        self.coord.barrier();
        Ok(())
    }

    fn broadcast(&self, data: &mut Vec<u8>) -> Result<(), SyncError> {
        let res = self.coord.broadcast(self.id, &*data);
        *data = res;
        Ok(())
    }
}

#[test]
fn mock_barrier_blocks_until_all_participants_arrive() {
    let participants = 2;
    let coord = MockCoordinator::new(participants);

    // Spawn two threads to simulate two processes.
    let coord_clone = coord.clone();
    let handle = thread::spawn(move || {
        // participant 1: call barrier immediately and measure blocking time
        let sync = MockSync::new(coord_clone, 1);
        let start = Instant::now();
        sync.barrier().unwrap();
        let elapsed = start.elapsed();
        elapsed
    });

    // participant 0: sleep then call barrier
    let sync0 = MockSync::new(coord, 0);
    thread::sleep(Duration::from_millis(250));
    sync0.barrier().unwrap();

    let elapsed = handle.join().expect("thread panicked");
    // Ensure the other participant was blocked for at least ~200ms.
    assert!(
        elapsed >= Duration::from_millis(200),
        "barrier did not block as expected (elapsed {:?})",
        elapsed
    );
}

#[test]
fn mock_broadcast_is_collective_and_delivers_root_data() {
    let participants = 3;
    let coord = MockCoordinator::new(participants);

    // channels to collect results
    let (tx, rx) = std::sync::mpsc::channel();

    // spawn non-root threads
    for id in 1..participants {
        let coord_clone = coord.clone();
        let tx = tx.clone();
        thread::spawn(move || {
            let sync = MockSync::new(coord_clone, id);
            // Non-root passes empty buffer and receives data into it
            let mut buf = Vec::new();
            sync.broadcast(&mut buf).unwrap();
            tx.send((id, buf)).expect("send failed");
        });
    }

    // root thread (id == 0) sends data
    let sync0 = MockSync::new(coord, 0);
    let data = vec![0xAAu8, 0xBB, 0xCC];
    let mut root_buf = data.clone();
    sync0.broadcast(&mut root_buf).unwrap();

    // Collect and assert
    assert_eq!(root_buf, data);
    for _ in 1..participants {
        let (id, received) = rx.recv().expect("didn't receive");
        assert_eq!(received, data, "participant {} got wrong data", id);
    }
}

#[cfg(feature = "mpi")]
#[test]
#[ignore = "requires MPI runtime (run with mpirun -n 2 ...)"]
fn sync_backend_mpi() {
    use std::time::Duration;

    // Construct the real backend (requires the `mpi` feature).
    let sync: Box<dyn SyncBackend> = SyncBackends::Mpi
        .build()
        .expect("backend construction failed");

    // Determine rank using mpi crate so we can write rank-aware assertions.
    let rank = sync.rank();

    // 1) Barrier blocking test:
    // Let rank 0 sleep a bit before calling barrier; rank 1 calls barrier early and should be blocked.

    if rank == 1 {
        let start = std::time::Instant::now();
        sync.barrier().unwrap();
        let elapsed = start.elapsed();
        // if rank 0 sleeps ~300ms before calling barrier, rank 1 should have been blocked.
        assert!(
            elapsed >= Duration::from_millis(200),
            "MPI barrier did not block long enough (elapsed {:?})",
            elapsed
        );
    } else if rank == 0 {
        // delay then call barrier
        std::thread::sleep(Duration::from_millis(300));
        sync.barrier().unwrap();
    } else {
        // other ranks simply call barrier
        sync.barrier().unwrap();
    }

    // 2) Broadcast collective test:
    let data = vec![0xAAu8, 0xBB, 0xCC];
    let recv = if rank == 0 {
        let mut buf = data.clone();
        sync.broadcast(&mut buf).unwrap();
        buf
    } else {
        // Non-root pass empty buffer; collective broadcast should still deliver the data.
        let mut buf = Vec::new();
        sync.broadcast(&mut buf).unwrap();
        buf
    };
    assert_eq!(recv, data);
}

/// Multiprocess sync backend tests
///
/// These tests are intended to be launched under a multiprocess test runner and
/// therefore are `#[ignore]` by default. They verify two properties of the
/// synchronization backends when run across multiple processes:
///
/// - Barrier correctness: the barrier should block processes until all
///   participants reach the barrier (we test this by having rank 0 delay and
///   checking another rank observes a measurable block time).
/// - Broadcast correctness: rank 0 broadcasts a byte payload and all other
///   ranks receive the identical payload. Broadcast is collective and blocks
///   appropriately until the root issues the data.
use bevy_tiled_display::*;
use duct::cmd;
use std::time::Duration;

fn backend_test(sync: Box<dyn SyncBackend + 'static>) {
    if sync.world_size() < 2 {
        eprintln!("skipped: world size >=2 not detected");
        return;
    }
    let rank = sync.rank();

    // Barrier blocking test: rank 0 delays, rank 1 measures blocking time.
    if rank == 1 {
        let start = std::time::Instant::now();
        sync.barrier().unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(200),
            "barrier did not block long enough (elapsed {:?})",
            elapsed
        );
    } else if rank == 0 {
        std::thread::sleep(Duration::from_millis(300));
        sync.barrier().unwrap();
    } else {
        sync.barrier().unwrap();
    }

    // Broadcast collective test.
    let data = vec![0xAAu8, 0xBB, 0xCC];
    let recv = if rank == 0 {
        let mut buf = data.clone();
        sync.broadcast(&mut buf).unwrap();
        buf
    } else {
        let mut buf = Vec::new();
        sync.broadcast(&mut buf).unwrap();
        buf
    };
    assert_eq!(recv, data);
}

#[cfg(feature = "mpi")]
#[test]
#[ignore = "MPI backend worker - requires MPI runtime"]
fn sync_backend_mpi_worker() {
    backend_test(SyncBackendType::Mpi.build());
}

#[cfg(feature = "mpi")]
#[test]
#[ignore = "MPI backend multiprocess orchestrator (cargo test -- --exact --ignored sync_backend_mpi)"]
fn sync_backend_mpi() {
    const WORLD_SIZE: usize = 2;
    const TIMEOUT_SECS: u64 = 5;

    // Build the test binary (no-run) so mpiexec can launch the test executable.
    let cargo_args: Vec<String> = "test --no-run -- --exact --ignored sync_backend_mpi_worker"
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let build = cmd("cargo", cargo_args)
        .stdout_capture()
        .stderr_capture()
        .run()
        .expect("failed to run cargo test --no-run");
    assert!(
        build.status.success(),
        "cargo test --no-run failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    // Build mpiexec command:
    // mpiexec /lines -n 1 <cmd> : -n 1 <cmd> : -n 1 <cmd>
    let chunk = "-n 1 cargo test -- --exact --ignored sync_backend_mpi_worker";
    let mpiexec_args: Vec<String> = std::iter::once("/lines")
        .chain(vec![chunk; WORLD_SIZE].join(" : ").split_whitespace())
        .map(|s| s.to_string())
        .collect();

    // Execute mpiexec with timeout
    let expr = cmd("mpiexec", mpiexec_args)
        .stdout_capture()
        .stderr_capture();
    let child = expr.start().expect("failed to spawn mpiexec process");
    let result = child.wait_timeout(Duration::from_secs(TIMEOUT_SECS));

    match result {
        Ok(Some(output)) => {
            assert!(
                output.status.success(),
                "mpiexec failed with status: {}",
                output.status
            );
        }
        Ok(None) => {
            panic!("mpiexec timed out after {} seconds", TIMEOUT_SECS);
        }
        Err(e) => {
            panic!("Failed to wait for mpiexec: {}", e);
        }
    }
}

#[test]
#[ignore = "UDP backend worker - requires RANK/WORLD_SIZE env vars"]
fn sync_backend_udp_worker() {
    backend_test(SyncBackendType::Udp.build());
}

#[test]
#[ignore = "UDP backend multiprocess orchestrator (cargo test -- --exact --ignored sync_backend_udp)"]
fn sync_backend_udp() {
    const WORLD_SIZE: usize = 2;
    const TIMEOUT_SECS: u64 = 5;

    // Spawn all processes
    let mut handles = Vec::new();

    for rank in 0..WORLD_SIZE {
        let handle = cmd!(
            "cargo",
            "test",
            "--",
            "--exact",
            "--ignored",
            "sync_backend_udp_worker"
        )
        .env("RANK", rank.to_string())
        .env("WORLD_SIZE", WORLD_SIZE.to_string())
        .stdout_capture()
        .stderr_capture()
        .start()
        .expect("failed to spawn test process");

        handles.push(handle);
    }

    // Wait for all processes with timeout
    let timeout = Duration::from_secs(TIMEOUT_SECS);
    let start = std::time::Instant::now();

    for (rank, handle) in handles.into_iter().enumerate() {
        let remaining = timeout.saturating_sub(start.elapsed());
        let result = handle.wait_timeout(remaining);

        match result {
            Ok(Some(output)) => {
                assert!(
                    output.status.success(),
                    "Process {} failed with status: {}",
                    rank,
                    output.status
                );
            }
            Ok(None) => {
                panic!("Process {} timed out after {:?}", rank, timeout);
            }
            Err(e) => {
                panic!("Failed to wait for process {}: {}", rank, e);
            }
        }
    }
}

use kfilesync_lib::domain::model::transfer::{TransferJob, TransferType, TransferState, TransferError, FileRequest};
use kfilesync_lib::domain::model::device::DeviceId;
use kfilesync_lib::domain::service::chunking::SizeBasedChunking;

fn make_job_with_two_files() -> TransferJob {
    let peer = DeviceId("peer1".to_string());
    let strategy = SizeBasedChunking::new();
    let files = vec![
        FileRequest {
            file_path: "test1.txt".to_string(),
            file_size: 100_000,
            sha256: "hash1".to_string(),
        },
        FileRequest {
            file_path: "test2.bin".to_string(),
            file_size: 200_000,
            sha256: "hash2".to_string(),
        }
    ];
    TransferJob::create_from_files(TransferType::Send, peer, files, &strategy)
}

#[test]
fn test_transfer_job_creation_and_state_machine() {
    let mut job = make_job_with_two_files();

    assert!(matches!(job.state, TransferState::Pending));
    assert_eq!(job.items.len(), 2);
    assert_eq!(job.items[0].chunk_manifest.chunks.len(), 1); // 100k -> no chunking -> 1 sentinel chunk
    assert_eq!(job.items[1].chunk_manifest.chunks.len(), 2); // 200k -> 128k chunk size -> 2 chunks

    // Pending -> Active
    job = job.accept().unwrap();
    assert!(matches!(job.state, TransferState::Active { .. }));

    // Record chunk done for file1 (1 chunk total -> immediately Verifying)
    let file1_id = job.items[0].file_id.clone();
    job = job.record_chunk_done(&file1_id, 0).unwrap();
    // File1 done but file2 still Pending, so job stays Active
    assert!(matches!(job.state, TransferState::Active { .. }));

    // Record chunks for file2
    let file2_id = job.items[1].file_id.clone();
    job = job.record_chunk_done(&file2_id, 0).unwrap();
    // File2 has 2 chunks, chunk 0 done -> still Transferring -> job still Active
    assert!(matches!(job.state, TransferState::Active { .. }));

    job = job.record_chunk_done(&file2_id, 1).unwrap();
    // Now all files are in Verifying -> job transitions to Verifying
    assert!(matches!(job.state, TransferState::Verifying));

    // Verifying -> Completed (A3: complete() only allowed from Verifying)
    job = job.complete().unwrap();
    assert!(matches!(job.state, TransferState::Completed { .. }));
}

#[test]
fn test_invalid_transitions_for_transfer() {
    let job = make_job_with_two_files();

    // Cannot complete a Pending job (must go through Verifying)
    let err = job.clone().complete();
    assert!(err.is_err(), "Should not complete a Pending job");

    // Cannot accept an Active job twice
    let active_job = job.clone().accept().unwrap();
    let err = active_job.accept();
    assert!(err.is_err(), "Should not accept an already Active job");

    // Cannot pause a Pending job
    let err = job.clone().pause(None);
    assert!(err.is_err(), "Should not pause a Pending job");

    // Cannot resume a non-Paused job
    let active_job = job.clone().accept().unwrap();
    let err = active_job.resume();
    assert!(err.is_err(), "Should not resume an Active job");
}

#[test]
fn test_pause_and_resume() {
    let job = make_job_with_two_files();
    let job = job.accept().unwrap();
    assert!(matches!(job.state, TransferState::Active { .. }));

    let job = job.pause(None).unwrap();
    assert!(matches!(job.state, TransferState::Paused { .. }));

    let job = job.resume().unwrap();
    assert!(matches!(job.state, TransferState::Active { .. }));
}

#[test]
fn test_fail_and_cancel() {
    let job = make_job_with_two_files();
    let job = job.accept().unwrap();

    // Test fail
    let failed = job.clone().fail(TransferError::ConnectionLost).unwrap();
    assert!(matches!(failed.state, TransferState::Failed { .. }));

    // Test cancel
    let cancelled = job.cancel().unwrap();
    assert!(matches!(cancelled.state, TransferState::Cancelled));
}

#[test]
fn test_progress_calculation() {
    let peer = DeviceId("peer1".to_string());
    let strategy = SizeBasedChunking::new();
    // file_size = 300_000 -> chunk_size = 131_072 -> 3 chunks (131072 + 131072 + 37856)
    let files = vec![FileRequest {
        file_path: "big.bin".to_string(),
        file_size: 300_000,
        sha256: "hash".to_string(),
    }];
    let job = TransferJob::create_from_files(TransferType::Send, peer, files, &strategy);
    let job = job.accept().unwrap();

    let progress = job.progress();
    assert_eq!(progress.total_bytes, 300_000);
    assert_eq!(progress.transferred_bytes, 0);

    let file_id = job.items[0].file_id.clone();
    let job = job.record_chunk_done(&file_id, 0).unwrap();
    let progress = job.progress();
    // 1 chunk done = 131_072 bytes transferred
    assert_eq!(progress.transferred_bytes, 131_072);
}

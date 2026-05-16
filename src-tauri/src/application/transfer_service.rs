use std::fmt::format;
use std::sync::Arc;

use crate::domain::error::DomainError;
use crate::domain::event::transfer::{
    TransferCompleted, TransferFailed, TransferProgressUpdated, TransferRequested,
};
use crate::domain::model::device::DeviceId;
use crate::domain::model::transfer::{
    FileRequest, JobId, TransferError, TransferJob, TransferState, TransferType,
};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::network::{NetworkClient, TransferRequest, TransferRequestItem};
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::infrastructure::security::chunk_hasher::ChunkHasher;

pub struct TransferAppService {
    local_device_id: DeviceId,
    transfer_repo: Arc<dyn TransferRepository>,
    device_repo: Arc<dyn DeviceRepository>,
    event_bus: Arc<dyn EventBus>,
    network_client: Arc<dyn NetworkClient>,
}

impl TransferAppService {
    pub fn new(
        local_device_id: DeviceId,
        transfer_repo: Arc<dyn TransferRepository>,
        device_repo: Arc<dyn DeviceRepository>,
        event_bus: Arc<dyn EventBus>,
        network_client: Arc<dyn NetworkClient>,
    ) -> Self {
        Self {
            local_device_id,
            transfer_repo,
            device_repo,
            event_bus,
            network_client,
        }
    }

    pub async fn send_files(
        self: Arc<Self>,
        peer_device_id: DeviceId,
        files: Vec<FileRequest>,
    ) -> Result<JobId, DomainError> {
        let device = self
            .device_repo
            .find_by_id(peer_device_id.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(peer_device_id.0.clone()))?;

        if !matches!(
            device.state,
            crate::domain::model::device::DeviceState::Paired(_)
        ) {
            return Err(DomainError::DeviceNotTrusted(peer_device_id.0.clone()));
        }

        let mut job = TransferJob::create_from_files(
            TransferType::Send,
            peer_device_id.clone(),
            files,
            // self.chunking_strategy.as_ref()
        );

        //Compute BLAKE3 per-chunk hashes AND SHA-256 whole-file hash concurrently
        let mut hash_handles = Vec::new();
        let mut sha_handles = Vec::new();

        for (idx, item) in job.items.iter().enumerate() {
            let path = std::path::Path::new(&item.file_path);
            if !path.exists() {
                return Err(DomainError::FileSystem(format!(
                    "File not found: {}",
                    item.file_path
                )));
            }

            let cs = item.chunk_manifest.chunk_size;
            let p = path.to_path_buf();
            let handle = 
                tokio::task::spawn_blocking(move || ChunkHasher::hash_file_chunks(&p, cs));
            hash_handles.push((idx, handle));

            if item.sha256.is_empty() {
                let p2 = std::path::PathBuf::from(&item.file_path);
                let sha_handle =
                    tokio::task::spawn_blocking(move || ChunkHasher::compute_sha256(&p2));
                sha_handles.push((idx, sha_handle));
            }
        }

        for (idx, handle) in hash_handles {
            match handle.await {
                Ok(Ok(hashed_chunks)) => {
                    job.items[idx].chunk_manifest.chunks = hashed_chunks;
                }
                Ok(Err(e)) => {
                    return Err(DomainError::IntegrityError(format!(
                        "Hash computation failed for {}: {}",
                        job.items[idx].file_path, e
                    )));
                }
                Err(e) => {
                    return Err(DomainError::IntegrityError(format!(
                        "Hash computation spawn error: {}",
                        e
                    )));
                }
            }
        }

        for (idx, sha_handle) in sha_handles {
            match sha_handle.await {
                Ok(Ok(sha)) => {
                    job.items[idx].sha256 = sha;
                }
                Ok(Err(e)) => {
                    return Err(DomainError::IntegrityError(
                        format!("SHA-256 computation failed for {}: {}",job.items[idx].file_path, e)
                    ));
                }
                Err(e) => {
                    return Err(DomainError::IntegrityError(
                        format!("SHA-256 spawn error: {}",e)
                    ));
                }
            }
        }


        // A5 FIX: 先提取需要的字段，再把 job 移进 save()
        let job_id = job.job_id.clone();
        let event_job_id = job.job_id.clone();
        let event_peer = peer_device_id.clone();

        self.transfer_repo.save(job).await?;

        self.event_bus.publish(Box::new(TransferRequested {
            job_id: event_job_id,
            peer: event_peer,
        }));

        // Spawn the actual push in the background so the Tauri command returns immediately.
        // Failures inside execute_send are recordes as TransferFailed events.
        let svc = self.clone();
        let push_job_id =job_id.clone();
        tokio::spawn(async move{
            if let Err(e) = svc.execute_send(&push_job_id).await {
                eprint!("[TransferService] execute_send failed for {}: {}", push_job_id.0, e);
            }
        });

        Ok(job_id)
    }

    /// Actively push a Send-type job to the peer:
    /// 1. POST /transfer/request with the manifest
    /// 2. Upload every chunk (skipping those the peer already has)
    /// 3. Mark the job Completed on success
    pub async fn execute_send(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        if !matches!(job.job_type, TransferType::Send | TransferType::SyncPush) {
            return Err(DomainError::InvalidStateTransition("execute_send: job is not Send/SyncPush"));
        }

        let peer = self.device_repo.find_by_id(job.peer_device_id.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(job.peer_device_id.0.clone()))?;

        let address = match &peer.state {
            crate::domain::model::device::DeviceState::Paired(info) => info.address.clone(),
            _ => return Err(DomainError::DeviceNotTrusted(peer.id.0.clone())),
        };

        // 1. Announce the transfer
        let req = TransferRequest {
            job_id: job.job_id.0.clone(),
            session_id: job.session_id.clone(),
            sender_device_id: self.local_device_id.0.clone(),
            items: job.items.iter().map(|item| TransferRequestItem {
                file_id: item.file_id.0.clone(),
                file_path: item.file_path.clone(),
                file_size: item.file_size,
                sha256: item.sha256.clone(),
                chunk_count: item.chunk_manifest.chunks.len() as u32,
                chunk_size: item.chunk_manifest.chunk_size,
            }).collect(),
        };

        let resp = self.network_client.request_transfer(&address, crate::DEFAULT_PORT, req).await?;
        if resp.status != "accepted" {
            let failed = job.clone().fail(TransferError::PeerRejected)?;
            self.transfer_repo.save(failed.clone()).await?;
            self.event_bus.publish(Box::new(TransferFailed {
                job_id: failed.job_id.clone(),
                error: TransferError::PeerRejected,
            }));
            return Err(DomainError::Network(format!("Peer rejected transfer: {}", resp.status)));
        }

        // Activate the job
        let mut current_job = match &job.state {
            TransferState::Pending => job.accept()?,
            _ => job,
        };
        self.transfer_repo.save(current_job.clone()).await?;

        // Map of file_id -> chunks already on peer (for resume)
        let skip_map: std::collections::HashMap<String, u32> = resp.skip_chunks.iter()
            .map(|c| (c.file_id.clone(), c.chunks_already_done))
            .collect();

        let item_snapshot = current_job.items.clone();
        let mut total_bytes: u64 = 0;

        for item in &item_snapshot {
            let already = *skip_map.get(&item.file_id.0).unwrap_or(&item.chunks_done);
            for chunk in &item.chunk_manifest.chunks {
                if chunk.index < already {
                    continue;
                }

                // Read chunk from disk
                let path = item.file_path.clone();
                let offset = chunk.offset;
                let size = chunk.size as u64;
                let data = tokio::task::spawn_blocking(move || {
                    ChunkHasher::read_chunk(std::path::Path::new(&path), offset, size)
                }).await
                .map_err(|e| DomainError::FileSystem(e.to_string()))??;

                // Upload
                if let Err(e) = self.network_client.upload_chunk(
                    &address,
                    crate::DEFAULT_PORT,
                    &current_job.job_id.0,
                    &item.file_id.0,
                    chunk.index,
                    data,
                ).await {
                    let failed = current_job.clone().fail(TransferError::ConnectionLost)?;
                    self.transfer_repo.save(failed.clone()).await?;
                    self.event_bus.publish(Box::new(TransferFailed {
                        job_id: failed.job_id.clone(),
                        error: TransferError::ConnectionLost,
                    }));
                    return Err(e);
                }

                current_job = current_job.record_chunk_done(&item.file_id, chunk.index)?;
                self.transfer_repo.save(current_job.clone()).await?;
                total_bytes += chunk.size as u64;

                // Publish progress for UI subscribers
                let total_chunks = item.chunk_manifest.chunks.len() as u32;
                let item_total = item.file_size;
                let item_done_chunks = chunk.index + 1;
                let bytes_done = std::cmp::min(
                    item_done_chunks as u64 * item.chunk_manifest.chunk_size.max(1) as u64,
                    item_total,
                );

                self.event_bus.publish(Box::new(TransferProgressUpdated {
                    job_id: current_job.job_id.clone(),
                    file_id: item.file_id.clone(),
                    chunks_done: item_done_chunks,
                    total_chunks,
                    bytes_done,
                    total_bytes: item_total,
                }));
            }
        }

        // All chunks pushed: state machine should be at Verifying now
        let completed = match current_job.state {
            TransferState::Verifying => current_job.complete()?,
            TransferState::Completed { .. } => current_job, // already completed by record_chunk_done path
            _ => {
                // Job is in some other state (e.g. Active with 0 items); treat as success.
                current_job
            }
        };

        self.transfer_repo.save(completed).await?;

        self.event_bus.publish(Box::new(TransferCompleted {
            job_id: job_id.clone(),
            total_bytes,
        }));

        Ok(())
    }

    pub async fn execute_receive(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self
            .transfer_repo
            .find_by_id(job_id)
            .await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let peer = self
            .device_repo
            .find_by_id(job.peer_device_id.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(job.peer_device_id.0.clone()))?;

        let address = match &peer.state {
            crate::domain::model::device::DeviceState::Paired(info) => info.address.clone(),
            _ => return Err(DomainError::DeviceNotTrusted(peer.id.0.clone())),
        };

        let mut current_job = job;
        let item_snapshot = current_job.items.clone();
        let mut total_bytes = 0u64;

        for item in &item_snapshot {
            for chunk in &item.chunk_manifest.chunks {
                if chunk.index < item.chunks_done {
                    continue;
                }

                let data = self
                    .network_client
                    .download_chunk(
                        &address,
                        crate::DEFAULT_PORT,
                        &current_job.job_id.0,
                        &item.file_id.0,
                        chunk.index,
                    )
                    .await?;

                if !chunk.hash.is_empty() && !ChunkHasher::verify_chunk(&data, &chunk.hash) {
                    let failed = current_job.fail(TransferError::VerificationFailed)?;
                    self.transfer_repo.save(failed.clone()).await?;
                    self.event_bus.publish(Box::new(TransferFailed {
                        job_id: failed.job_id.clone(),
                        error: TransferError::VerificationFailed,
                    }));
                    return Err(DomainError::IntegrityError(format!(
                        "Chunk verification failed: file={}, chunk={}",
                        item.file_id.0, chunk.index
                    )));
                }

                tokio::task::spawn_blocking({
                    let path = item.file_path.clone();
                    let offset = chunk.offset;
                    let data = data.clone();
                    move || -> Result<(), DomainError> {
                        use std::io::{Seek, SeekFrom, Write};
                        let parent = std::path::Path::new(&path).parent();
                        if let Some(p) = parent {
                            let _ = std::fs::create_dir_all(p);
                        }
                        let mut f = std::fs::OpenOptions::new()
                            .create(true)
                            .write(true)
                            .truncate(false)  // chunk-based seek-write: keep existing file content
                            .open(&path)
                            .map_err(|e| DomainError::FileSystem(e.to_string()))?;

                        f.seek(SeekFrom::Start(offset))
                            .map_err(|e| DomainError::FileSystem(e.to_string()))?;

                        f.write_all(&data)
                            .map_err(|e| DomainError::FileSystem(e.to_string()))?;
                        Ok(())
                    }
                }).await
                .map_err(|e| DomainError::FileSystem(e.to_string()))? // JoinError
                ?; // propagate inner DomainError from file write

                current_job = current_job.record_chunk_done(&item.file_id, chunk.index)?;
                self.transfer_repo.save(current_job.clone()).await?;
                total_bytes += chunk.size as u64;

                // Publish progress for UI subscribers
                let total_chunks = item.chunk_manifest.chunks.len() as u32;
                let item_total = item.file_size;
                let item_done_chunks = chunk.index + 1;
                let bytes_done = std::cmp::min(
                    item_done_chunks as u64 * item.chunk_manifest.chunk_size.max(1) as u64,
                    item_total
                );

                self.event_bus.publish(Box::new(TransferProgressUpdated {
                    job_id:current_job.job_id.clone(),
                    file_id: item.file_id.clone(),
                    chunks_done: item_done_chunks,
                    total_chunks,
                    bytes_done,
                    total_bytes:item_total
                }));

            }
        }

        for item in &item_snapshot {
            if !item.sha256.is_empty() {
                let path = item.file_path.clone();
                let expected = item.sha256.clone();
                let actual = tokio::task::spawn_blocking(move || {
                    ChunkHasher::compute_sha256(std::path::Path::new(&path))
                })
                .await
                .map_err(|e| DomainError::FileSystem(e.to_string()))??;

                if actual != expected {
                    return Err(DomainError::IntegrityError(format!(
                        "File hash mismath for {}: expected {}, got {}",
                        item.file_path, expected, actual
                    )));
                }
            }
        }

        let completed = current_job.complete()?;
        self.transfer_repo.save(completed).await?;

        self.event_bus.publish(Box::new(TransferCompleted {
            job_id: job_id.clone(),
            total_bytes,
        }));

        Ok(())
    }

    pub async fn accept_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self
            .transfer_repo
            .find_by_id(job_id)
            .await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.accept()?;
        self.transfer_repo.save(job).await?;

        Ok(())
    }

    pub async fn pause_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self
            .transfer_repo
            .find_by_id(job_id)
            .await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.pause(None)?;
        self.transfer_repo.save(job).await?;

        Ok(())
    }

    pub async fn resume_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self
            .transfer_repo
            .find_by_id(job_id)
            .await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.resume()?;
        self.transfer_repo.save(job).await?;

        Ok(())
    }

    pub async fn cancel_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self
            .transfer_repo
            .find_by_id(job_id)
            .await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.cancel()?;
        self.transfer_repo.save(job).await?;

        Ok(())
    }

    pub async fn retry_failed_transfers(&self) {
        const MAX_RETRIES: u32 = 5;
        const BASE_DELAY_MS: u64 = 1000;

        let jobs = match self.transfer_repo.find_incomplete_jobs().await {
            Ok(jobs) => jobs,
            Err(_) => return,
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        for mut job in jobs {
            if let TransferState::Failed { retries, .. } = &job.state {
                if *retries >= MAX_RETRIES {
                    continue;
                }

                let delay_secs = BASE_DELAY_MS * 2u64.pow(*retries) / 1000;
                let elapsed = now.saturating_sub(job.created_at);
                if elapsed < delay_secs {
                    continue;
                }

                let job_id = job.job_id.clone();
                job.state = TransferState::Active { started_at: now };
                if let Err(e) = self.transfer_repo.save(job).await {
                    eprintln!(
                        "[RetryScheduler] Failed to save retried job {}: {}",
                        job_id.0, e
                    );
                } else {
                    println!("[RetryScheduler] Retrying job {} (attempt)", job_id.0);
                }
            }
        }
    }
}

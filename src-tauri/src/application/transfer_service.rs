use std::fmt::format;
use std::sync::Arc;
use crate::domain::model::transfer::{TransferJob, TransferType, TransferError, FileRequest, JobId};
use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use crate::domain::service::chunking::ChunkingStrategy;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::network::NetworkClient;
use crate::domain::event::transfer::{TransferRequested, TransferCompleted, TransferFailed};
use crate::infrastructure::security::chunk_hasher::ChunkHasher;

pub struct TransferAppService {
    local_device_id: DeviceId,
    transfer_repo: Arc<dyn TransferRepository>,
    device_repo: Arc<dyn DeviceRepository>,
    event_bus: Arc<dyn EventBus>,
    chunking_strategy: Arc<dyn ChunkingStrategy>,
    network_client: Arc<dyn NetworkClient>,
}

impl TransferAppService {
    pub fn new(
        local_device_id: DeviceId,
        transfer_repo: Arc<dyn TransferRepository>,
        device_repo: Arc<dyn DeviceRepository>,
        event_bus: Arc<dyn EventBus>,
        chunking_strategy: Arc<dyn ChunkingStrategy>,
        network_client: Arc<dyn NetworkClient>,
    ) -> Self {
        Self { local_device_id, transfer_repo, device_repo, event_bus, chunking_strategy, network_client }
    }

    pub async fn send_files(&self, peer_device_id: DeviceId, files: Vec<FileRequest>) -> Result<JobId, DomainError> {
        let device = self.device_repo.find_by_id(peer_device_id.clone()).await?  
            .ok_or_else(|| DomainError::DeviceNotFound(peer_device_id.0.clone()))?;

        if !matches!(device.state, crate::domain::model::device::DeviceState::Paired(_)) {
            return Err(DomainError::DeviceNotTrusted(peer_device_id.0.clone()));
        }

        let mut job = TransferJob::create_from_files(
            TransferType::Send,
            peer_device_id.clone(),
            files,
            self.chunking_strategy.as_ref()
        );

        for item in &mut job.items {
            let path = std::path::Path::new(&item.file_path);
            if path.exists() {
                let cs = item.chunk_manifest.chunk_size;

                match tokio::task::spawn_blocking({
                    let p = path.to_path_buf();
                    move ||ChunkHasher::hash_file_chunks(&p, cs)
                }).await {
                    Ok(Ok(hashed_chunks)) => {
                        item.chunk_manifest.chunks = hashed_chunks;
                    }
                    Ok(Err(e)) => {
                        return Err(DomainError::IntegrityError(
                            format!("Hash computation failed for {}: {}", item.file_path, e)
                        ));
                    }
                    Err(e) => {
                       return Err(DomainError::IntegrityError(
                            format!("Hash computation spawn error: {}", e)
                        ));
                    }
                }
            }
            
        }

        // A5 FIX: 先提取需要的字段，再把 job 移进 save()
        let job_id = job.job_id.clone();
        let event_job_id = job.job_id.clone();
        let event_peer = peer_device_id;
        
        self.transfer_repo.save(job).await?;

        self.event_bus.publish(Box::new(TransferRequested {
            job_id: event_job_id,
            peer: event_peer,
        }));

        Ok(job_id)
    }

    pub async fn execute_receive(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let peer = self.device_repo.find_by_id(job.peer_device_id.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(job.peer_device_id.0.clone()))?;
        
        let address = match &peer.state{
            crate::domain::model::device::DeviceState::Paired(info) => info.address.clone(),
            _ => return Err(DomainError::DeviceNotTrusted(peer.id.0.clone())),
        };

        let mut current_job=job;
        let item_snapshot = current_job.items.clone();
        let mut total_bytes = 0u64;

        for item in &item_snapshot {
            for chunk in &item.chunk_manifest.chunks {
                if chunk.index < item.chunks_done{
                    continue;
                }

                let data = self.network_client.download_chunk(
                    &address,
                    crate::DEFAULT_PORT,
                    &current_job.job_id.0,
                    &item.file_id.0,
                    chunk.index,
                ).await?;

                if !chunk.hash.is_empty()&& !ChunkHasher::verify_chunk(&data, &chunk.hash){
                    let failed = current_job.fail(TransferError::VerificationFailed)?;
                    self.transfer_repo.save(failed.clone()).await?;
                    self.event_bus.publish(Box::new(TransferFailed {
                        job_id: failed.job_id.clone(),
                        error: TransferError::VerificationFailed,
                    }));
                    return Err(DomainError::IntegrityError(
                        format!("Chunk verification failed: file={}, chunk={}", item.file_id.0, chunk.index)
                    ));
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
                ?;  // propagate inner DomainError from file write

                current_job = current_job.record_chunk_done(&item.file_id, chunk.index)?;
                self.transfer_repo.save(current_job.clone()).await?;
                total_bytes += chunk.size as u64;
            }
        }

        for item in &item_snapshot{
            if !item.sha256.is_empty(){
                let path = item.file_path.clone();
                let expected = item.sha256.clone();
                let actual = tokio::task::spawn_blocking(move ||{
                    ChunkHasher::compute_sha256(std::path::Path::new(&path))
                }).await
                .map_err(|e| DomainError::FileSystem(e.to_string()))??;
            }
        }

        let completed = current_job.complete()?;
        self.transfer_repo.save(completed).await?;

        self.event_bus.publish(Box::new(TransferCompleted {
            job_id: job_id.clone(),
            total_bytes
        }));

        Ok(())
    }

    pub async fn accept_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.accept()?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }

    pub async fn pause_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.pause(None)?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }

    pub async fn resume_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.resume()?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }

    pub async fn cancel_transfer(&self, job_id: &JobId) -> Result<(), DomainError> {
        let job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| DomainError::TransferNotFound(job_id.0.clone()))?;

        let job = job.cancel()?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }
}

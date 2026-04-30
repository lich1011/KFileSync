use std::sync::Arc;
use crate::domain::model::transfer::{TransferJob, TransferType, FileRequest, JobId};
use crate::domain::model::device::DeviceId;
use crate::domain::service::chunking::ChunkingStrategy;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::event_bus::EventBus;
use crate::domain::event::transfer::TransferRequested;

pub struct TransferAppService {
    transfer_repo: Arc<dyn TransferRepository>,
    device_repo: Arc<dyn DeviceRepository>,
    event_bus: Arc<dyn EventBus>,
    chunking_strategy: Arc<dyn ChunkingStrategy>,
}

impl TransferAppService {
    pub fn new(
        transfer_repo: Arc<dyn TransferRepository>,
        device_repo: Arc<dyn DeviceRepository>,
        event_bus: Arc<dyn EventBus>,
        chunking_strategy: Arc<dyn ChunkingStrategy>,
    ) -> Self {
        Self { transfer_repo, device_repo, event_bus, chunking_strategy }
    }

    pub async fn send_files(&self, peer_device_id: DeviceId, files: Vec<FileRequest>) -> Result<JobId, String> {
        let device = self.device_repo.find_by_id(peer_device_id.clone()).await?
            .ok_or_else(|| "Device not found".to_string())?;

        if !matches!(device.state, crate::domain::model::device::DeviceState::Paired(_)) {
            return Err("Device is not paired".to_string());
        }

        let job = TransferJob::create_from_files(
            TransferType::Send,
            peer_device_id.clone(),
            files,
            self.chunking_strategy.as_ref()
        );

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

    pub async fn accept_transfer(&self, job_id: &JobId) -> Result<(), String> {
        let mut job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| "Transfer job not found".to_string())?;

        job = job.accept().map_err(|e| e.to_string())?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }

    pub async fn pause_transfer(&self, job_id: &JobId) -> Result<(), String> {
        let mut job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| "Transfer job not found".to_string())?;

        job = job.pause(None).map_err(|e| e.to_string())?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }

    pub async fn resume_transfer(&self, job_id: &JobId) -> Result<(), String> {
        let mut job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| "Transfer job not found".to_string())?;

        job = job.resume().map_err(|e| e.to_string())?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }

    pub async fn cancel_transfer(&self, job_id: &JobId) -> Result<(), String> {
        let mut job = self.transfer_repo.find_by_id(job_id).await?
            .ok_or_else(|| "Transfer job not found".to_string())?;

        job = job.cancel().map_err(|e| e.to_string())?;
        self.transfer_repo.save(job).await?;
        
        Ok(())
    }
}

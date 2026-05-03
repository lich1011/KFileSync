use std::sync::Arc;
use async_trait::async_trait;

use crate::domain::error::DomainError;
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{FileEntry, SyncAction, SyncPlan};
use crate::domain::model::share::{ShareId, SharePermission};
use crate::domain::model::transfer::{TransferJob, TransferType, FileRequest};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::network::NetworkClient;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::domain::service::chunking::ChunkingStrategy;
use crate::domain::service::sync_plan_generator::SyncPlanGenerator;
use crate::domain::event::transfer::TransferRequested;
use crate::infrastructure::network::dto::SyncIndexResponseDto;
use crate::application::sync_flow::SyncFlowTemplate;

pub struct HttpSyncFlow {
    local_device_id: DeviceId,
    network_client: Arc<dyn NetworkClient>,
    device_repo: Arc<dyn DeviceRepository>,
    share_repo: Arc<dyn ShareRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    transfer_repo: Arc<dyn TransferRepository>,
    event_bus: Arc<dyn EventBus>,
    chunking_strategy: Arc<dyn ChunkingStrategy>,
}

impl HttpSyncFlow {
    pub fn new(
        local_device_id: DeviceId,
        network_client: Arc<dyn NetworkClient>,
        device_repo: Arc<dyn DeviceRepository>,
        share_repo: Arc<dyn ShareRepository>,
        file_index_repo: Arc<dyn FileIndexRepository>,
        transfer_repo: Arc<dyn TransferRepository>,
        event_bus: Arc<dyn EventBus>,
        chunking_strategy: Arc<dyn ChunkingStrategy>,
    ) -> Self {
        Self {
            local_device_id,
            network_client,
            device_repo,
            share_repo,
            file_index_repo,
            transfer_repo,
            event_bus,
            chunking_strategy,
        }
    }

    fn make_file_requests(actions: &[SyncAction]) -> Vec<FileRequest> {
        actions.iter().map(|a| FileRequest {
            file_path: a.entry.path.clone(),
            file_size: a.entry.size,
            sha256: a.entry.sha256.clone().unwrap_or_default(),
        }).collect()
    }
}

#[async_trait]
impl SyncFlowTemplate for HttpSyncFlow {
    async fn verify_permission(&self, share_id: &ShareId, peer: &DeviceId) -> Result<(), DomainError> {
        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        if !share.members.iter().any(|m| m.device_id == *peer) {
            return Err(DomainError::PermissionDenied("Peer is not a member of this share".into()));
        }
        Ok(())
    }

    async fn fetch_remote_index(&self, share_id: &ShareId, peer: &DeviceId) -> Result<Vec<FileEntry>, DomainError> {
        let device = self.device_repo.find_by_id(peer.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(peer.0.clone()))?;

        let address = match device.state {
            crate::domain::model::device::DeviceState::Paired(data) => data.address,
            _ => return Err(DomainError::DeviceNotTrusted(peer.0.clone())),
        };

        let json_str = self.network_client.fetch_remote_index(&address, crate::DEFAULT_PORT, &share_id.0).await?;

        let res: SyncIndexResponseDto = serde_json::from_str(&json_str)
            .map_err(|e| DomainError::Network(format!("Failed to parse remote index: {}", e)))?;

        Ok(res.entries)
    }

    async fn generate_plan(&self, share_id: &ShareId, peer: &DeviceId, remote_index: &[FileEntry]) -> Result<SyncPlan, DomainError> {
        // Get local index
        let local_index = self.file_index_repo.find_all_by_share(share_id).await?;

        // Get local device's permission for this share
        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        let permission = share.members.iter()
            .find(|m| m.device_id == *peer)
            .map(|m| m.permission.clone())
            .unwrap_or(SharePermission::ReadOnly);

        // Use the pure-function SyncPlanGenerator
        let plan = SyncPlanGenerator::generate(
            &local_index,
            remote_index,
            &self.local_device_id,
            &permission,
        );

        Ok(plan)
    }

    async fn execute_plan(&self, plan: &SyncPlan, peer: &DeviceId) -> Result<(), DomainError> {
        // Create a SyncPull TransferJob for files we need to pull from the peer
        if !plan.to_pull.is_empty() {
            let pull_files: Vec<FileRequest> = plan.to_pull.iter().map(|action| {
                FileRequest {
                    file_path: action.entry.path.clone(),
                    file_size: action.entry.size,
                    sha256: action.entry.sha256.clone().unwrap_or_default(),
                }
            }).collect();

            let mut job = TransferJob::create_from_files(
                TransferType::SyncPull,
                peer.clone(),
                pull_files,
                self.chunking_strategy.as_ref(),
            );
            job.share_id = plan.to_pull.first().map(|a| a.entry.share_id.clone());
            self.transfer_repo.save(job).await?;
        }

        // Create a SyncPush TransferJob for files we need to push to the peer
        if !plan.to_push.is_empty() {
            let push_files: Vec<FileRequest> = plan.to_push.iter().map(|action| {
                FileRequest {
                    file_path: action.entry.path.clone(),
                    file_size: action.entry.size,
                    sha256: action.entry.sha256.clone().unwrap_or_default(),
                }
            }).collect();

            let mut job = TransferJob::create_from_files(
                TransferType::SyncPush,
                peer.clone(),
                push_files,
                self.chunking_strategy.as_ref(),
            );
            job.share_id = plan.to_push.first().map(|a| a.entry.share_id.clone());
            self.transfer_repo.save(job).await?;
        }

        for conflict in &plan.conflicts {
            match &conflict.resolution {
                ConflictResolution::KeepBoth { conflict_copy_path } => {
                    self.file_index_repo.save_conflict(conflict).await?;

                    let files = vec![FileRequest {
                        file_path: conflict_copy_path.clone(),
                        file_size: conflict.remote.size,
                        sha256: conflict.remote.sha256.clone().unwrap_or_default(),
                    }];

                    let mut job = TransferJob::create_from_files(
                        TransferType::SyncPull,
                        peer.clone(),
                        files,
                        self.chunking_strategy.as_ref(),
                    );
                    job.share_id = Some(conflict.local.share_id.clone());
                    self.transfer_repo.save(job).await?;
                }
                ConflictResolution::KeepRemote => {
                    self.file_index_repo.save_conflict(conflict).await?;

                    let files = vec![FileRequest {
                        file_path: conflict.remote.path.clone(),
                        file_size: conflict.remote.size,
                        sha256: conflict.remote.sha256.clone().unwrap_or_default(),
                    }];

                    let mut job = TransferJob::create_from_files(
                        TransferType::SyncPull,
                        peer.clone(),
                        files,
                        self.chunking_strategy.as_ref(),
                    );
                    job.share_id = Some(conflict.remote.share_id.clone());
                    self.transfer_repo.save(job).await?;
                }
                ConflictResolution::KeepLocal | ConflictResolution::Pending => {
                    self.file_index_repo.save_conflict(conflict).await?;
                }
            }
        }

        Ok(())
    }

    async fn update_versions(&self, share_id: &ShareId, plan: &SyncPlan) -> Result<(), DomainError> {
        // Merge remote version vectors into local index entries for successfully resolved items
        for action in &plan.to_pull {
            let local_entry: Option<FileEntry> = self.file_index_repo.find_by_path(share_id, &action.path).await?;

            let updated: FileEntry = match local_entry {
                Some(entry) => entry.apply_remote_version(&action.entry),
                None => action.entry.clone(),
            };
            self.file_index_repo.save(&updated).await?;
        }

        for action in &plan.to_push {
            let local_entry: Option<FileEntry> = self.file_index_repo.find_by_path(share_id, &action.path).await?;
            if let Some(entry) = local_entry {
                let updated: FileEntry = entry.apply_remote_version(&action.entry);
                self.file_index_repo.save(&updated).await?;
            }
        }

        for conflict in &plan.conflicts {
            let merged_version: VersionVector = conflict.local.version.merge(&conflict.remote.version);
            let local_entry: Option<FileEntry> = self.file_index_repo.find_by_path(share_id, &conflict.path).await?;
            if let Some(mut entry) = local_entry {
                entry.version = merged_version;
                self.file_index_repo.save(&entry).await?;
            }
        }
        Ok(())
    }


    async fn emit_events(&self, _plan: &SyncPlan) -> Result<(), DomainError> {
        // TransferRequested events are published in execute_plan; sync completion
        // events will be published by transfer_service when jobs finish.
        let files_synced: u32 = (plan.to_pull.len() + plan.to_push.len()) as u32;

        if let Some(first_action: &SyncAction) = plan.to_pull.first().or(plan.to_push.first()) {
            self.event_bus.publish(Box::new(SyncCompleted {
                share_id: first_action.entry.share_id.clone(),
                files_synced,
            }));
        }

        for conflict in &plan.conflicts {
            self.event_bus.publish(Box::new(ConflictDetected {
                share_id: conflict.local.share_id.clone(),
                path: conflict.path.clone(),
                local_version: conflict.local.version.clone(),
                remote_version: conflict.remote.version.clone(),
            }));
        }

        Ok(())
    }


}

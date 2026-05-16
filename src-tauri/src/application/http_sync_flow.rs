use std::sync::Arc;

use axum::extract::path;
use reqwest::redirect::Action;

use crate::domain::error::DomainError;
use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{
    ConflictResolution, FileEntry, SyncAction, SyncPlan, VersionVector,
};
use crate::domain::model::share::{ShareId, SharePermission};
use crate::domain::model::transfer::{TransferJob, TransferType, FileRequest};
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::network::NetworkClient;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::port::transfer_repo::TransferRepository;
use crate::domain::service::policy_enforcer::{PolicyEnforcer,SyncDirection};
// use crate::domain::service::specification::SyncDirection;
use crate::domain::service::sync_plan_generator::SyncPlanGenerator;
use crate::domain::event::sync_events::{SyncCompleted, ConflictDetected};
use crate::infrastructure::network::dto::SyncIndexResponseDto;
// use crate::application::sync_flow::SyncFlowTemplate;

pub struct HttpSyncFlow {
    local_device_id: DeviceId,
    network_client: Arc<dyn NetworkClient>,
    device_repo: Arc<dyn DeviceRepository>,
    share_repo: Arc<dyn ShareRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
    transfer_repo: Arc<dyn TransferRepository>,
    event_bus: Arc<dyn EventBus>,
    policy_enforcer: Arc<PolicyEnforcer>,
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
        policy_enforcer: Arc<PolicyEnforcer>,
    ) -> Self {
        Self {
            local_device_id,
            network_client,
            device_repo,
            share_repo,
            file_index_repo,
            transfer_repo,
            event_bus,
            policy_enforcer,
        }
    }

    // fn make_file_requests(actions: &[SyncAction]) -> Vec<FileRequest> {
    //     actions.iter().map(|a| FileRequest {
    //         file_path: a.entry.path.clone(),
    //         file_size: a.entry.size,
    //         sha256: a.entry.sha256.clone().unwrap_or_default(),
    //     }).collect()
    // }

    pub async fn execute (&self, share_id: &ShareId, peer: &DeviceId) -> Result<SyncPlan, DomainError>{
        self.verify_permission(share_id, peer).await?;
        let remote_index =self.fetch_remote_index(share_id, peer).await?;
        let plan = self.generate_plan(share_id, peer, &remote_index).await?;
        self.execute_plan(&plan, peer).await?;
        self.update_versions(share_id, &plan).await?;
        Ok(plan)

    }
// }

// #[async_trait]
// impl SyncFlowTemplate for HttpSyncFlow {
    async fn verify_permission(&self, share_id: &ShareId, peer: &DeviceId) -> Result<(), DomainError> {
        // let share = self.share_repo.find_by_id(share_id).await?
        //     .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        // if !share.members.iter().any(|m| m.device_id == *peer) {
        //     return Err(DomainError::PermissionDenied("Peer is not a member of this share".into()));
        // }
        // Ok(())
        self.policy_enforcer.check_sync(peer, share_id, SyncDirection::Pull).await
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

    fn validate_relative_path(p: &str) -> Result<(),DomainError>{
        let path=std::path::Path::new(p);
        if path.is_absolute(){
            return Err(DomainError::PermissionDenied(format!("absolute path not allow: {}",p)));   
        }

        for comp in path.components(){
            match comp {
                std::path::Component::ParentDir =>{
                    return Err(DomainError::PermissionDenied(format!("path traversal denied: {}",p)));
                }
                std::path::Component::Prefix(_) | std::path::Component::RootDir =>{
                    return Err(DomainError::PermissionDenied(format!("absolute/root path denied: {}",p)));
                }
                _ =>{}
            }
        }

        Ok(())
    }

    async fn execute_plan(&self, plan: &SyncPlan, peer: &DeviceId) -> Result<(), DomainError> {
        // validate every path the remote peer is asking us to read or write.
        for action in plan.to_pull.iter().chain(plan.to_push.iter()){
            Self::validate_relative_path(&action.path);
        }

        for conflict in &plan.conflicts{
            Self::validate_relative_path(&conflict.path);
            if let ConflictResolution::KeepBoth { conflict_copy_path } = &conflict.resolution{
                Self::validate_relative_path(conflict_copy_path);
            }
        }

        // Create a SyncPull TransferJob for files we need to pull from the peer
        if !plan.to_pull.is_empty() {
            // let pull_files: Vec<FileRequest> = plan.to_pull.iter().map(|action| {
            //     FileRequest {
            //         file_path: action.entry.path.clone(),
            //         file_size: action.entry.size,
            //         sha256: action.entry.sha256.clone().unwrap_or_default(),
            //     }
            // }).collect();

            let pull_files: Vec<FileRequest> = Self::make_file_requests(&plan.to_pull);

            let mut job = TransferJob::create_from_files(
                TransferType::SyncPull,
                peer.clone(),
                pull_files
                // self.chunking_strategy.as_ref(),
            );
            job.share_id = plan.to_pull.first().map(|a| a.entry.share_id.0.clone());
            self.transfer_repo.save(job).await?;
        }

        // Create a SyncPush TransferJob for files we need to push to the peer
        if !plan.to_push.is_empty() {
            // let push_files: Vec<FileRequest> = plan.to_push.iter().map(|action| {
            //     FileRequest {
            //         file_path: action.entry.path.clone(),
            //         file_size: action.entry.size,
            //         sha256: action.entry.sha256.clone().unwrap_or_default(),
            //     }
            // }).collect();
            let push_files: Vec<FileRequest> = Self::make_file_requests(&plan.to_push);

            let mut job = TransferJob::create_from_files(
                TransferType::SyncPush,
                peer.clone(),
                push_files,
            );
            job.share_id = plan.to_push.first().map(|a| a.entry.share_id.0.clone());
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
                        // self.chunking_strategy.as_ref(),
                    );
                    job.share_id = Some(conflict.local.share_id.0.clone());
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
                        // self.chunking_strategy.as_ref(),
                    );
                    job.share_id = Some(conflict.remote.share_id.0.clone());
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
            //let local_entry: Option<FileEntry> = self.file_index_repo.find_by_path(share_id, &action.path).await?;
            let local_entry= self.file_index_repo.find_by_path(share_id, &action.path).await?;
            let updated = match local_entry {
                Some(entry) => entry.apply_remote_version(&action.entry),
                None => action.entry.clone(),
            };
            self.file_index_repo.save(&updated).await?;
        }

        for action in &plan.to_push {
            //let local_entry: Option<FileEntry> = self.file_index_repo.find_by_path(share_id, &action.path).await?;
            let local_entry= self.file_index_repo.find_by_path(share_id, &action.path).await?;
            if let Some(entry) = local_entry {
                let updated = entry.apply_remote_version(&action.entry);
                self.file_index_repo.save(&updated).await?;
            }
        }

        for conflict in &plan.conflicts {
            let merged_version: VersionVector = conflict.local.version.merge(&conflict.remote.version);
            //let local_entry: Option<FileEntry> = self.file_index_repo.find_by_path(share_id, &conflict.path).await?;
            let local_entry= self.file_index_repo.find_by_path(share_id, &conflict.path).await?;
            if let Some(mut entry) = local_entry {
                entry.version = merged_version;
                self.file_index_repo.save(&entry).await?;
            }
        }
        Ok(())
    }


    async fn emit_events(&self, plan: &SyncPlan) -> Result<(), DomainError> {
        let files_synced: u32 = (plan.to_pull.len() + plan.to_push.len()) as u32;

        // Publish SyncCompleted if any files were transferred
        let first_action = plan.to_pull.first().or_else(|| plan.to_push.first());
        if let Some(action) = first_action {
            self.event_bus.publish(Box::new(SyncCompleted {
                share_id: action.entry.share_id.clone(),
                files_synced,
            }));
        }

        // Publish ConflictDetected for every unresolved conflict
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

    fn make_file_requests(actions: &[SyncAction]) -> Vec<FileRequest> {
        actions.iter().map(|action| FileRequest {
            file_path:action.entry.path.clone(),
            file_size:action.entry.size,
            sha256:action.entry.sha256.clone().unwrap_or_default()
        }).collect()
    }
}

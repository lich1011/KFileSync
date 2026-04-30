use std::sync::Arc;
use async_trait::async_trait;

use crate::domain::model::device::DeviceId;
use crate::domain::model::file_entry::{FileEntry, SyncPlan};
use crate::domain::model::share::{ShareId, SharePermission};
use crate::domain::port::network::NetworkClient;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::port::file_index_repo::FileIndexRepository;
use crate::domain::service::sync_plan_generator::SyncPlanGenerator;
use crate::infrastructure::network::dto::SyncIndexResponseDto;
use crate::application::sync_flow::SyncFlowTemplate;

pub struct HttpSyncFlow {
    local_device_id: DeviceId,
    network_client: Arc<dyn NetworkClient>,
    device_repo: Arc<dyn DeviceRepository>,
    share_repo: Arc<dyn ShareRepository>,
    file_index_repo: Arc<dyn FileIndexRepository>,
}

impl HttpSyncFlow {
    pub fn new(
        local_device_id: DeviceId,
        network_client: Arc<dyn NetworkClient>,
        device_repo: Arc<dyn DeviceRepository>,
        share_repo: Arc<dyn ShareRepository>,
        file_index_repo: Arc<dyn FileIndexRepository>,
    ) -> Self {
        Self {
            local_device_id,
            network_client,
            device_repo,
            share_repo,
            file_index_repo,
        }
    }
}

#[async_trait]
impl SyncFlowTemplate for HttpSyncFlow {
    async fn verify_permission(&self, share_id: &ShareId, peer: &DeviceId) -> Result<(), String> {
        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| "Share not found".to_string())?;

        if !share.members.iter().any(|m| m.device_id == *peer) {
            return Err("Peer is not a member of this share".to_string());
        }
        Ok(())
    }

    async fn fetch_remote_index(&self, share_id: &ShareId, peer: &DeviceId) -> Result<Vec<FileEntry>, String> {
        let device = self.device_repo.find_by_id(peer.clone()).await?
            .ok_or_else(|| "Device not found".to_string())?;

        let address = match device.state {
            crate::domain::model::device::DeviceState::Paired(data) => data.address,
            _ => return Err("Device is not paired".to_string()),
        };

        let json_str = self.network_client.fetch_remote_index(&address, crate::DEFAULT_PORT, &share_id.0).await?;

        let res: SyncIndexResponseDto = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse remote index: {}", e))?;

        Ok(res.entries)
    }

    async fn generate_plan(&self, share_id: &ShareId, peer: &DeviceId, remote_index: &[FileEntry]) -> Result<SyncPlan, String> {
        // Get local index
        let local_index = self.file_index_repo.find_all_by_share(share_id).await?;

        // Get local device's permission for this share
        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| "Share not found".to_string())?;

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

    async fn execute_plan(&self, _plan: &SyncPlan, _peer: &DeviceId) -> Result<(), String> {
        // TODO: Create TransferJobs for each file in to_pull/to_push
        Ok(())
    }

    async fn update_versions(&self, _share_id: &ShareId, _plan: &SyncPlan) -> Result<(), String> {
        // TODO: Merge version vectors after successful transfer
        Ok(())
    }

    async fn emit_events(&self, _plan: &SyncPlan) -> Result<(), String> {
        // TODO: Publish SyncCompleted domain events
        Ok(())
    }
}

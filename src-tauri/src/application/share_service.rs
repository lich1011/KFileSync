use std::sync::Arc;

use crate::domain::model::device::DeviceId;
use crate::domain::model::share::{Share, ShareId, SharePermission, SyncMode};
use crate::domain::error::DomainError;
use crate::domain::port::event_bus::EventBus;
use crate::domain::port::network::{NetworkClient, ShareInvite};
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::service::policy_enforcer::PolicyEnforcer;
use crate::domain::event::sharing::{ShareCreated, MemberAuthorized, MemberRevoked};

pub struct ShareAppService {
    local_device_id: DeviceId,
    share_repo: Arc<dyn ShareRepository>,
    device_repo: Arc<dyn DeviceRepository>,
    network_client: Arc<dyn NetworkClient>,
    policy_enforcer: Arc<PolicyEnforcer>,
    event_bus: Arc<dyn EventBus>,
}

impl ShareAppService {
    pub fn new(
        local_device_id: DeviceId,
        share_repo: Arc<dyn ShareRepository>,
        device_repo: Arc<dyn DeviceRepository>,
        network_client: Arc<dyn NetworkClient>,
        policy_enforcer: Arc<PolicyEnforcer>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            local_device_id,
            share_repo,
            device_repo,
            network_client,
            policy_enforcer,
            event_bus,
        }
    }

    pub async fn create_share(
        &self,
        share_id: String,
        share_name: String,
        local_path: String,
        sync_mode: SyncMode,
    ) -> Result<ShareId, DomainError> {
        let sid = ShareId(share_id);
        let share = Share::create(
            sid.clone(),
            share_name,
            local_path,
            sync_mode,
            self.local_device_id.clone(),
        );

        self.share_repo.save(&share).await?;

        self.event_bus.publish(Box::new(ShareCreated {
            share_id: sid.clone(),
            created_by: self.local_device_id.clone(),
        }));

        Ok(sid)
    }

    /// The Sharing Invite Saga
    pub async fn invite_device(
        &self,
        share_id: &ShareId,
        peer_id: &DeviceId,
        permission: SharePermission,
    ) -> Result<(), DomainError> {
        // Step 1: PolicyEnforcer check
        self.policy_enforcer.check_transfer(peer_id).await?;

        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        // Step 2: Send POST /share/invite
        let peer_device = self.device_repo.find_by_id(peer_id.clone()).await?
            .ok_or_else(|| DomainError::DeviceNotFound(peer_id.0.clone()))?;
            
        let address = match peer_device.state {
            crate::domain::model::device::DeviceState::Paired(data) => data.address,
            _ => return Err(DomainError::DeviceNotTrusted(peer_id.0.clone())),
        };
        
        let req = ShareInvite {
            share_id: share.share_id.0.clone(),
            share_name: share.share_name.clone(),
            permission: match permission {
                SharePermission::ReadOnly => "read_only".to_string(),
                SharePermission::ReadWrite => "read_write".to_string(),
                SharePermission::SendOnly => "send_only".to_string(),
                SharePermission::ReceiveOnly => "receive_only".to_string(),
            },
            invited_by: self.local_device_id.0.clone(),
        };

        if let Err(_e) = self.network_client.invite_to_share(&address, crate::DEFAULT_PORT, req).await {
            return Err(DomainError::Network(format!("Peer {} rejected the invite", peer_id.0)));
        }

        // Step 3: Peer accepted, authorize member
        let updated_share = share.authorize_member(
            peer_id.clone(),
            permission.clone(),
            self.local_device_id.clone(),
        ).map_err(|e| DomainError::BusinessRuleViolation(format!("{:?}", e)))?;

        // Step 4: Save and publish event
        if let Err(e) = self.share_repo.save(&updated_share).await {
            // COMPENSATION: Failed to persist after peer accepted.
            // In a real system, we must send a CANCEL/ROLLBACK message to the peer here.
            eprintln!("[ShareService] Failed to persist. Rolling back peer {}", peer_id.0);
            if let Err(rollbakc_err) = self.network_client.cancel_share_invite(
                &address, crate::DEFAULT_PORT,
                &share_id.0, &self.local_device_id.0
            ).await{
                eprintln!("[ShareService] CRITICAL: Saga rollback also failed for peer {}: {}. Manual cleanup required.", peer_id.0, rollbakc_err);
            };
            return Err(e);
        }

        self.event_bus.publish(Box::new(MemberAuthorized {
            share_id: share_id.clone(),
            device_id: peer_id.clone(),
            permission,
        }));

        Ok(())
    }

    pub async fn remove_member(&self, share_id: &ShareId, peer_id: &DeviceId) -> Result<(), DomainError> {
        let share = self.share_repo.find_by_id(share_id).await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        let updated_share = share.remove_member(peer_id)?;

        self.share_repo.save(&updated_share).await?;

        self.event_bus.publish(Box::new(MemberRevoked {
            share_id: share_id.clone(),
            device_id: peer_id.clone(),
        }));

        Ok(())
    }
}

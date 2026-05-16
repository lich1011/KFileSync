use crate::domain::error::DomainError;
use crate::domain::model::device::{DeviceId, DeviceState};
use crate::domain::model::share::{ShareId, SyncMode};
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::service::specification::IgnoreSpec;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDirection {
    Push,
    Pull,
}

pub struct PolicyEnforcer {
    device_repo: Arc<dyn DeviceRepository>,
    share_repo: Arc<dyn ShareRepository>,
}

impl PolicyEnforcer {
    pub fn new(
        device_repo: Arc<dyn DeviceRepository>,
        share_repo: Arc<dyn ShareRepository>,
    ) -> Self {
        Self {
            device_repo,
            share_repo,
        }
    }

    /// Check if a transfer from/to the given device is allowed (must be Paired).
    pub async fn check_transfer(&self, peer: &DeviceId) -> Result<(), DomainError> {
        let device = self
            .device_repo
            .find_by_id(peer.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(peer.0.clone()))?;

        // We can use a dummy context for transfer check, but since we refactored
        // TrustedDeviceSpec to use SyncContext, we should probably refactor TrustedDeviceSpec
        // to not require a Share in its context, or just pass a dummy one if we only check transfer.
        // Wait! The prompt says we just check device pairing.
        // Let's create a quick check without full context.
        if !matches!(device.state, DeviceState::Paired(_)) {
            return Err(DomainError::DeviceNotTrusted(peer.0.clone()));
        }

        Ok(())
    }

    /// Check if a sync action is allowed for the given device on the given share.
    /// This uses the AndSpec composite to evaluate Trust -> Member -> Permission.
    pub async fn check_sync(
        &self,
        peer: &DeviceId,
        share_id: &ShareId,
        action: SyncDirection,
    ) -> Result<(), DomainError> {
        let device = self
            .device_repo
            .find_by_id(peer.clone())
            .await?
            .ok_or_else(|| DomainError::DeviceNotFound(peer.0.clone()))?;

        if !matches!(device.state, DeviceState::Paired(_)) {
            return Err(DomainError::DeviceNotTrusted(peer.0.clone()));
        }

        let share = self
            .share_repo
            .find_by_id(share_id)
            .await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        if !share.has_member(&device.id) {
            return Err(DomainError::PermissionDenied(format!(
                "Device {} is not a member of share {}",
                peer.0, share_id.0
            )));
        }

        // SyncMOde semantics : "this share's purpose is X relative to remote peer".
        // SendOnly -> we (this device) push, peers may not to push to us -> reject Push from peer.
        // ReceiveOnly -> we receive, peers may not allow to pull from us -> rejec Pull by peer.

        match share.sync_mode {
            SyncMode::SendOnly if action == SyncDirection::Push => {
                return Err(DomainError::PermissionDenied(
                    "Share is SendOnly: peers cannot push".into()
                ));
            }

            SyncMode::ReceiveOnly if action == SyncDirection::Pull => {
                return Err(DomainError::PermissionDenied(
                    "Share is ReceiveOnly: peers cannot pull".into()
                ));
            }

            _ => {}
        }

        if let Some(permission) = share.get_permission(&device.id) {
            let allowed = match action {
                SyncDirection::Push => permission.can_push(),
                SyncDirection::Pull => permission.can_pull(),
            };

            if !allowed {
                return Err(DomainError::PermissionDenied(format!(
                    "Sync action {:?} denied for device {} on share {} ",action, peer.0, share_id.0
                )));
            }
        } else {
            return Err(DomainError::PermissionDenied(format!(
                "No permission found for device {} on share {}", peer.0, share_id.0
            )));
        }

        Ok(())
    }

    /// Check if a file is ignored by the .syncignore rules.
    pub fn check_file_ignored(path: &Path, is_dir: bool, ignore_spec: &IgnoreSpec) -> bool {
        ignore_spec.is_ignored(path, is_dir)
    }
}

use std::path::Path;
use std::sync::Arc;
use crate::domain::error::DomainError;
use crate::domain::model::device::DeviceId;
use crate::domain::model::share::ShareId;
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use crate::domain::service::specification::{
    AndSpec, IgnoreContext, IgnoreSpec, ShareMemberSpec, Specification,
    SyncDirection, SyncContext, TrustedDeviceSpec, PermissionSpec,
};

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
        if !matches!(device.state, crate::domain::model::device::DeviceState::Paired(_)) {
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

        let share = self
            .share_repo
            .find_by_id(share_id)
            .await?
            .ok_or_else(|| DomainError::ShareNotFound(share_id.0.clone()))?;

        let ctx = SyncContext {
            device: &device,
            share: &share,
            action,
        };

        let combined_spec = AndSpec(TrustedDeviceSpec, AndSpec(ShareMemberSpec, PermissionSpec));

        if !combined_spec.is_satisfied_by(&ctx) {
            return Err(DomainError::PermissionDenied(format!(
                "Sync action {:?} denied for device {} on share {}",
                action, peer.0, share_id.0
            )));
        }

        Ok(())
    }

    /// Check if a file is ignored by the .syncignore rules.
    pub fn check_file_ignored(
        &self,
        path: &Path,
        is_dir: bool,
        ignore_spec: &IgnoreSpec,
    ) -> bool {
        let ctx = IgnoreContext { path, is_dir };
        ignore_spec.is_satisfied_by(&ctx)
    }
}

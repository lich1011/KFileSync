use crate::domain::error::DomainError;
use crate::domain::model::device::{Device, DeviceId, DeviceState};
use crate::domain::model::share::{Share, ShareId, ShareStatus, SyncMode};
use crate::domain::port::repository::DeviceRepository;
use crate::domain::port::share_repo::ShareRepository;
use kfilesync_core::service::ignore_spec::IgnoreSpec;
use std::sync::Arc;

use kfilesync_core::domain::Device as CoreDevice;
use kfilesync_core::domain::DevicePlatform as CoreDevicePlatform;
use kfilesync_core::domain::DeviceState as CoreDeviceState;
use kfilesync_core::domain::DeviceType as CoreDeviceType;
use kfilesync_core::domain::Share as CoreShare;
use kfilesync_core::domain::SharePermission as CoreSharePermission;
use kfilesync_core::domain::ShareStatus as CoreShareStatus;
use kfilesync_core::domain::SyncMode as CoreSyncMode;
use kfilesync_core::service::policy_enforcer::{
    evaluate_policy, PolicyDecision, SyncDirection as CoreSyncDirection,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDirection {
    Push,
    Pull,
}

/// Sprint 3 desktop upgrade: adapts a desktop `Device` into core's flat
/// `Device` shape so `evaluate_policy` can read its trust state.
/// `evaluate_policy` only ever inspects `state` (see its doc comment), so
/// every other field here is an unread placeholder — desktop's richer
/// per-state data (certificate, alias, address, timestamps) lives in
/// `DeviceState`'s variants and is intentionally not duplicated onto core's
/// shape.
fn to_core_device(device: &Device) -> CoreDevice {
    let state = match &device.state {
        DeviceState::Discovered(_) => CoreDeviceState::Discovered,
        DeviceState::Paired(_) => CoreDeviceState::Paired,
        DeviceState::Revoked(_) => CoreDeviceState::Revoked,
    };
    CoreDevice {
        id: kfilesync_core::domain::DeviceId(device.id.0.clone()),
        alias: String::new(),
        device_type: CoreDeviceType::Desktop,
        platform: CoreDevicePlatform::Linux,
        state,
        cert_fingerprint_hex: None,
    }
}

/// Sprint 3 desktop upgrade: adapts a desktop `Share` into core's flat
/// `Share` shape. `evaluate_policy` only reads `status` and `sync_mode`
/// (membership lives outside `Share` entirely — passed as a separate
/// `member_permission` parameter, per ADR-013); `id`/`name`/
/// `default_permission` here are unread placeholders.
fn to_core_share(share: &Share) -> CoreShare {
    let status = match &share.status {
        ShareStatus::Active => CoreShareStatus::Active,
        ShareStatus::Paused => CoreShareStatus::Paused,
        // Desktop's `Error` status has no core equivalent; treat it as
        // blocking (same as Paused) since an errored share must not sync.
        ShareStatus::Error(_) => CoreShareStatus::Paused,
    };
    let sync_mode = match share.sync_mode {
        SyncMode::TwoWay => CoreSyncMode::TwoWay,
        SyncMode::SendOnly => CoreSyncMode::SendOnly,
        SyncMode::ReceiveOnly => CoreSyncMode::ReceiveOnly,
    };
    CoreShare {
        id: kfilesync_core::domain::ShareId(share.share_id.0.clone()),
        name: share.share_name.clone(),
        sync_mode,
        default_permission: CoreSharePermission::ReadWrite,
        status,
    }
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
    ///
    /// Sprint 3 desktop upgrade: the unified checks (device paired, share
    /// exists, `ShareStatus == Active`, membership, `SyncMode` direction) are
    /// delegated to `kfilesync_core::service::policy_enforcer::evaluate_policy`
    /// (ADR-013) — this closes a real gap the plan doc flagged: the desktop
    /// previously never checked `ShareStatus` at all, so a `Paused` share
    /// could still be synced. Desktop's own `SharePermission` has two extra
    /// variants (`SendOnly`/`ReceiveOnly`) core's binary `ReadOnly`/`ReadWrite`
    /// can't express, so that finer-grained per-member direction check is
    /// still enforced here afterward, unchanged from before.
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

        let share = self.share_repo.find_by_id(share_id).await?;
        let member_permission = share.as_ref().and_then(|s| s.get_permission(&device.id));

        let core_direction = match action {
            SyncDirection::Push => CoreSyncDirection::Push,
            SyncDirection::Pull => CoreSyncDirection::Pull,
        };

        // Any membership at all maps to `ReadWrite` here so core's coarse
        // permission check never blocks SendOnly/ReceiveOnly members — the
        // real direction restriction for those is enforced below using
        // desktop's own `can_push`/`can_pull`.
        let core_permission = member_permission.as_ref().map(|_| CoreSharePermission::ReadWrite);
        let core_device = to_core_device(&device);
        let core_share = share.as_ref().map(to_core_share);

        match evaluate_policy(&core_device, core_share, core_permission, core_direction) {
            PolicyDecision::Allowed => {}
            PolicyDecision::DeviceNotPaired => return Err(DomainError::DeviceNotTrusted(peer.0.clone())),
            PolicyDecision::ShareNotFound => return Err(DomainError::ShareNotFound(share_id.0.clone())),
            PolicyDecision::SharePaused => {
                return Err(DomainError::PermissionDenied(format!(
                    "Share {} is not active", share_id.0
                )));
            }
            PolicyDecision::NotAMember => {
                return Err(DomainError::PermissionDenied(format!(
                    "Device {} is not a member of share {}", peer.0, share_id.0
                )));
            }
            PolicyDecision::PermissionDeniedForDirection | PolicyDecision::SyncModeForbidsDirection => {
                return Err(DomainError::PermissionDenied(format!(
                    "Sync action {:?} denied for device {} on share {}", action, peer.0, share_id.0
                )));
            }
        }

        if let Some(permission) = member_permission {
            let allowed = match action {
                SyncDirection::Push => permission.can_push(),
                SyncDirection::Pull => permission.can_pull(),
            };

            if !allowed {
                return Err(DomainError::PermissionDenied(format!(
                    "Sync action {:?} denied for device {} on share {}", action, peer.0, share_id.0
                )));
            }
        }

        Ok(())
    }

    /// Check if a file is ignored by the .syncignore rules.
    pub fn check_file_ignored(path: &str, is_dir: bool, ignore_spec: &IgnoreSpec) -> bool {
        ignore_spec.is_ignored(path, is_dir)
    }
}
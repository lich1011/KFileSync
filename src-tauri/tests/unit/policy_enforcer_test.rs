use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kfilesync_lib::domain::error::DomainError;
use kfilesync_lib::domain::model::device::{Device, DeviceId, DeviceState, DiscoveredData, PairedData};
use kfilesync_lib::domain::model::share::{Share, ShareId, ShareStatus, SyncMode};
use kfilesync_lib::domain::port::repository::DeviceRepository;
use kfilesync_lib::domain::port::share_repo::ShareRepository;
use kfilesync_lib::domain::service::policy_enforcer::{PolicyEnforcer, SyncDirection};

struct MockDeviceRepo(Mutex<Option<Device>>);

#[async_trait]
impl DeviceRepository for MockDeviceRepo {
    async fn find_by_id(&self, _id: DeviceId) -> Result<Option<Device>, DomainError> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn find_paired(&self) -> Result<Vec<Device>, DomainError> {
        Ok(Vec::new())
    }
    async fn save(&self, device: Device) -> Result<(), DomainError> {
        *self.0.lock().unwrap() = Some(device);
        Ok(())
    }
    async fn update_trust_status(&self, _id: DeviceId, _status: DeviceState) -> Result<(), DomainError> {
        Ok(())
    }
}

struct MockShareRepo(Mutex<Option<Share>>);

#[async_trait]
impl ShareRepository for MockShareRepo {
    async fn save(&self, share: &Share) -> Result<(), DomainError> {
        *self.0.lock().unwrap() = Some(share.clone());
        Ok(())
    }
    async fn find_by_id(&self, _id: &ShareId) -> Result<Option<Share>, DomainError> {
        Ok(self.0.lock().unwrap().clone())
    }
    async fn find_by_member(&self, _device_id: &DeviceId) -> Result<Vec<Share>, DomainError> {
        Ok(Vec::new())
    }
    async fn find_all(&self) -> Result<Vec<Share>, DomainError> {
        Ok(Vec::new())
    }
}

fn paired_device(id: &str) -> Device {
    Device {
        id: DeviceId(id.to_string()),
        state: DeviceState::Paired(PairedData {
            certificate: kfilesync_lib::domain::model::device::Certificate::mock("cert"),
            paired_at: 0,
            alias: "peer".to_string(),
            address: "127.0.0.1:9000".to_string(),
            last_seen_at: None,
        }),
    }
}

fn active_share(peer: &DeviceId) -> Share {
    let creator = DeviceId("local".to_string());
    Share::create(
        ShareId("s1".to_string()),
        "Share".to_string(),
        "/tmp/share".to_string(),
        SyncMode::TwoWay,
        creator.clone(),
    )
    .authorize_member(
        peer.clone(),
        kfilesync_lib::domain::model::share::SharePermission::ReadWrite,
        creator,
    )
    .unwrap()
}

fn make_enforcer(device: Option<Device>, share: Option<Share>) -> PolicyEnforcer {
    PolicyEnforcer::new(
        Arc::new(MockDeviceRepo(Mutex::new(device))),
        Arc::new(MockShareRepo(Mutex::new(share))),
    )
}

#[tokio::test]
async fn test_paused_share_rejects_both_directions() {
    let peer = DeviceId("peer".to_string());
    let device = paired_device(&peer.0);
    let mut share = active_share(&peer);
    share.status = ShareStatus::Paused;
    let enforcer = make_enforcer(Some(device), Some(share));

    let push_err = enforcer.check_sync(&peer, &ShareId("s1".to_string()), SyncDirection::Push).await;
    assert!(push_err.is_err());
    let pull_err = enforcer.check_sync(&peer, &ShareId("s1".to_string()), SyncDirection::Pull).await;
    assert!(pull_err.is_err());
}

#[tokio::test]
async fn test_unpaired_device_is_rejected() {
    let peer = DeviceId("peer".to_string());
    let device = Device {
        id: peer.clone(),
        state: DeviceState::Discovered(DiscoveredData {
            alias: "peer".to_string(),
            address: "127.0.0.1:9000".to_string(),
        }),
    };
    let share = active_share(&peer);
    let enforcer = make_enforcer(Some(device), Some(share));

    let err = enforcer.check_sync(&peer, &ShareId("s1".to_string()), SyncDirection::Pull).await;
    assert!(matches!(err, Err(DomainError::DeviceNotTrusted(_))));
}

#[tokio::test]
async fn test_active_two_way_readwrite_member_is_allowed() {
    let peer = DeviceId("peer".to_string());
    let device = paired_device(&peer.0);
    let share = active_share(&peer);
    let enforcer = make_enforcer(Some(device), Some(share));

    assert!(enforcer.check_sync(&peer, &ShareId("s1".to_string()), SyncDirection::Push).await.is_ok());
    assert!(enforcer.check_sync(&peer, &ShareId("s1".to_string()), SyncDirection::Pull).await.is_ok());
}

#[tokio::test]
async fn test_missing_share_is_rejected() {
    let peer = DeviceId("peer".to_string());
    let device = paired_device(&peer.0);
    let enforcer = make_enforcer(Some(device), None);

    let err = enforcer.check_sync(&peer, &ShareId("s1".to_string()), SyncDirection::Pull).await;
    assert!(matches!(err, Err(DomainError::ShareNotFound(_))));
}
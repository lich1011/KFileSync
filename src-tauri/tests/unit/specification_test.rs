use std::path::PathBuf;
use kfilesync_lib::domain::model::device::{Device, DeviceId, DeviceState, PairedData, Certificate};
use kfilesync_lib::domain::model::share::{Share, ShareId, SharePermission, SyncMode};
use kfilesync_lib::domain::service::specification::{
    AndSpec, IgnoreContext, IgnoreSpec, SyncContext, SyncDirection,
    ShareMemberSpec, Specification, TrustedDeviceSpec, PermissionSpec,
};

#[test]
fn test_sync_context_composition() {
    let paired_device = Device {
        id: DeviceId("dev2".to_string()),
        state: DeviceState::Paired(PairedData {
            alias: "dev2".to_string(),
            certificate: Certificate::from_pem("-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----".to_string()).unwrap(),
            paired_at: 0,
            address: "192.168.1.100".to_string(),
        }),
    };

    let creator_id = DeviceId("creator".to_string());
    let mut share = Share::create(
        ShareId("share1".to_string()),
        "My Share".to_string(),
        "/tmp".to_string(),
        SyncMode::TwoWay,
        creator_id.clone(),
    );

    // Give the paired device read-only permission
    share = share.authorize_member(paired_device.id.clone(), SharePermission::ReadOnly, creator_id.clone()).unwrap();

    let combined_spec = AndSpec(TrustedDeviceSpec, AndSpec(ShareMemberSpec, PermissionSpec));

    let ctx_pull = SyncContext {
        device: &paired_device,
        share: &share,
        action: SyncDirection::Pull,
    };

    let ctx_push = SyncContext {
        device: &paired_device,
        share: &share,
        action: SyncDirection::Push,
    };

    // Paired, is member, has ReadOnly (so can Pull) -> Should satisfy
    assert!(combined_spec.is_satisfied_by(&ctx_pull));

    // Paired, is member, has ReadOnly (so CANNOT Push) -> Should fail
    assert!(!combined_spec.is_satisfied_by(&ctx_push));
}

#[test]
fn test_global_sync_mode_override() {
    let paired_device = Device {
        id: DeviceId("dev2".to_string()),
        state: DeviceState::Paired(PairedData {
            alias: "dev2".to_string(),
            certificate: Certificate::from_pem("-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----".to_string()).unwrap(),
            paired_at: 0,
            address: "192.168.1.100".to_string(),
        }),
    };

    let creator_id = DeviceId("creator".to_string());
    let mut share = Share::create(
        ShareId("share1".to_string()),
        "My Share".to_string(),
        "/tmp".to_string(),
        SyncMode::SendOnly, // Local device sends only! So peers can only pull.
        creator_id.clone(),
    );

    // Peer has ReadWrite per-member, but the share is SendOnly
    share = share.authorize_member(paired_device.id.clone(), SharePermission::ReadWrite, creator_id.clone()).unwrap();

    let combined_spec = AndSpec(TrustedDeviceSpec, AndSpec(ShareMemberSpec, PermissionSpec));

    // Pull is allowed (local sending -> remote pulling)
    let ctx_pull = SyncContext {
        device: &paired_device,
        share: &share,
        action: SyncDirection::Pull,
    };
    assert!(combined_spec.is_satisfied_by(&ctx_pull));

    // Push is DENIED because share is SendOnly
    let ctx_push = SyncContext {
        device: &paired_device,
        share: &share,
        action: SyncDirection::Push,
    };
    assert!(!combined_spec.is_satisfied_by(&ctx_push));
}

#[test]
fn test_ignore_spec() {
    // We create a spec with some additional rules
    let spec = IgnoreSpec::new(PathBuf::from("/base").as_path(), &["*.tmp", "build/"]).unwrap();

    // 1. Built-in ignores
    assert!(spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from(".DS_Store").as_path(), is_dir: false }));
    assert!(spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from(".lansync-tmp/file.part").as_path(), is_dir: false }));

    // 2. Custom ignores
    assert!(spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from("a.tmp").as_path(), is_dir: false }));
    assert!(spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from("build").as_path(), is_dir: true }));
    assert!(spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from("build/output.bin").as_path(), is_dir: false }));

    // 3. Allowed files
    assert!(!spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from("main.rs").as_path(), is_dir: false }));
    assert!(!spec.is_satisfied_by(&IgnoreContext { path: PathBuf::from("src/a.txt").as_path(), is_dir: false }));
}

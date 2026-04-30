use kfilesync_lib::domain::model::device::DeviceId;
use kfilesync_lib::domain::model::file_entry::{BlockList, ConflictResolution, EntryType, FileEntry};
use kfilesync_lib::domain::model::share::{ShareId, SharePermission};
use kfilesync_lib::domain::service::sync_plan_generator::SyncPlanGenerator;

fn setup_entries() -> (DeviceId, SharePermission, FileEntry, FileEntry) {
    let local_dev = DeviceId("local_dev".to_string());
    let _remote_dev = DeviceId("remote_dev".to_string());
    let perm = SharePermission::ReadWrite;

    let base_entry = FileEntry::new(
        ShareId("s1".to_string()),
        "file.txt".to_string(),
        EntryType::File,
        &local_dev,
    );

    let local = base_entry.clone();
    let remote = base_entry.clone();

    (local_dev, perm, local, remote)
}

#[test]
fn test_only_local_has_it() {
    let (local_dev, perm, local, _) = setup_entries();
    
    let plan = SyncPlanGenerator::generate(&[local.clone()], &[], &local_dev, &perm);
    
    assert_eq!(plan.to_push.len(), 1);
    assert_eq!(plan.to_pull.len(), 0);
    assert_eq!(plan.conflicts.len(), 0);
    assert_eq!(plan.unchanged.len(), 0);
    assert_eq!(plan.to_push[0].path, "file.txt");
}

#[test]
fn test_only_remote_has_it() {
    let (local_dev, perm, _, remote) = setup_entries();
    
    let plan = SyncPlanGenerator::generate(&[], &[remote.clone()], &local_dev, &perm);
    
    assert_eq!(plan.to_push.len(), 0);
    assert_eq!(plan.to_pull.len(), 1);
    assert_eq!(plan.conflicts.len(), 0);
    assert_eq!(plan.unchanged.len(), 0);
    assert_eq!(plan.to_pull[0].path, "file.txt");
}

#[test]
fn test_equal_versions() {
    let (local_dev, perm, local, remote) = setup_entries();
    
    let plan = SyncPlanGenerator::generate(&[local], &[remote], &local_dev, &perm);
    
    assert_eq!(plan.to_push.len(), 0);
    assert_eq!(plan.to_pull.len(), 0);
    assert_eq!(plan.conflicts.len(), 0);
    assert_eq!(plan.unchanged.len(), 1);
    assert_eq!(plan.unchanged[0], "file.txt");
}

#[test]
fn test_local_newer() {
    let (local_dev, perm, mut local, remote) = setup_entries();
    
    local = local.update_content(100, "hash1".to_string(), BlockList::default(), &local_dev);
    
    let plan = SyncPlanGenerator::generate(&[local], &[remote], &local_dev, &perm);
    
    assert_eq!(plan.to_push.len(), 1);
    assert_eq!(plan.to_pull.len(), 0);
    assert_eq!(plan.conflicts.len(), 0);
}

#[test]
fn test_remote_newer() {
    let (local_dev, perm, local, mut remote) = setup_entries();
    
    let remote_dev = DeviceId("remote_dev".to_string());
    remote = remote.update_content(100, "hash1".to_string(), BlockList::default(), &remote_dev);
    
    let plan = SyncPlanGenerator::generate(&[local], &[remote], &local_dev, &perm);
    
    assert_eq!(plan.to_push.len(), 0);
    assert_eq!(plan.to_pull.len(), 1);
    assert_eq!(plan.conflicts.len(), 0);
}

#[test]
fn test_conflict() {
    let (local_dev, perm, mut local, mut remote) = setup_entries();
    
    let remote_dev = DeviceId("remote_dev".to_string());
    
    local = local.update_content(100, "hashL".to_string(), BlockList::default(), &local_dev);
    remote = remote.update_content(200, "hashR".to_string(), BlockList::default(), &remote_dev);
    
    let plan = SyncPlanGenerator::generate(&[local], &[remote], &local_dev, &perm);
    
    assert_eq!(plan.to_push.len(), 0);
    assert_eq!(plan.to_pull.len(), 0);
    assert_eq!(plan.conflicts.len(), 1);
    
    // Check resolution is determined (will likely be KeepBoth)
    match plan.conflicts[0].resolution {
        ConflictResolution::KeepBoth { .. } => {}, // Expected
        _ => panic!("Expected KeepBoth resolution"),
    }
}

#[test]
fn test_permissions() {
    let (local_dev, _, local, remote) = setup_entries();
    
    let remote_dev = DeviceId("remote_dev".to_string());
    
    // ReadOnly: cannot push
    let perm_ro = SharePermission::ReadOnly;
    let local_newer = local.clone().update_content(100, "hashL".to_string(), BlockList::default(), &local_dev);
    let plan_ro = SyncPlanGenerator::generate(&[local_newer.clone()], &[remote.clone()], &local_dev, &perm_ro);
    assert_eq!(plan_ro.to_push.len(), 0); // PUSH BLOCKED
    
    // SendOnly: cannot pull
    let perm_so = SharePermission::SendOnly;
    let remote_newer = remote.clone().update_content(200, "hashR".to_string(), BlockList::default(), &remote_dev);
    let plan_so = SyncPlanGenerator::generate(&[local.clone()], &[remote_newer.clone()], &local_dev, &perm_so);
    assert_eq!(plan_so.to_pull.len(), 0); // PULL BLOCKED
}

#[test]
fn test_tombstone_not_resurrected() {
    let (local_dev, perm, local, _) = setup_entries();
    
    // Local has a deleted (tombstoned) file. Remote doesn't have it at all.
    // The file should NOT be pushed to remote — it's dead.
    let tombstone = local.mark_deleted(&local_dev);
    let plan = SyncPlanGenerator::generate(&[tombstone], &[], &local_dev, &perm);
    assert_eq!(plan.to_push.len(), 0, "Tombstoned local file should not be pushed");
    
    // Remote has a deleted file. Local doesn't have it at all.
    // The file should NOT be pulled to local — it's dead.
    let (_, _, _, remote) = setup_entries();
    let remote_tombstone = remote.mark_deleted(&DeviceId("remote_dev".to_string()));
    let plan2 = SyncPlanGenerator::generate(&[], &[remote_tombstone], &local_dev, &perm);
    assert_eq!(plan2.to_pull.len(), 0, "Tombstoned remote file should not be pulled");
}

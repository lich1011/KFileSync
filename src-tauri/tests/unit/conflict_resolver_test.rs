use kfilesync_lib::domain::model::device::DeviceId;
use kfilesync_lib::domain::model::file_entry::{ConflictResolution, EntryType, FileEntry};
use kfilesync_lib::domain::model::share::ShareId;
use kfilesync_lib::domain::service::conflict_resolver::ConflictResolver;

fn make_entry(path: &str, timestamp: u64, device: &str, deleted: bool) -> FileEntry {
    let mut entry = FileEntry::new(
        ShareId("s1".to_string()),
        path.to_string(),
        EntryType::File,
        &DeviceId(device.to_string()),
    );
    entry.modified_at = timestamp;
    entry.deleted = deleted;
    entry
}

#[test]
fn test_delete_vs_modify() {
    // Local deleted, remote modified
    let local = make_entry("file.txt", 100, "dev1", true);
    let remote = make_entry("file.txt", 110, "dev2", false);
    assert_eq!(ConflictResolver::resolve(&local, &remote), ConflictResolution::KeepRemote);

    // Local modified, remote deleted
    let local2 = make_entry("file.txt", 110, "dev1", false);
    let remote2 = make_entry("file.txt", 100, "dev2", true);
    assert_eq!(ConflictResolver::resolve(&local2, &remote2), ConflictResolution::KeepLocal);

    // Both deleted
    let local3 = make_entry("file.txt", 100, "dev1", true);
    let remote3 = make_entry("file.txt", 110, "dev2", true);
    assert_eq!(ConflictResolver::resolve(&local3, &remote3), ConflictResolution::KeepLocal);
}

#[test]
fn test_newer_wins() {
    let t_older = 1600000000;
    let t_newer = 1600000100;

    // Local newer
    let local = make_entry("doc.txt", t_newer, "devA", false);
    let remote = make_entry("doc.txt", t_older, "devB", false);
    
    let res1 = ConflictResolver::resolve(&local, &remote);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res1 {
        assert!(conflict_copy_path.contains("doc.sync-conflict-20200913-122640-devB.txt"));
    } else {
        panic!("Expected KeepBoth");
    }

    // Remote newer
    let local2 = make_entry("folder/doc.txt", t_older, "devA", false);
    let remote2 = make_entry("folder/doc.txt", t_newer, "devB", false);
    
    let res2 = ConflictResolver::resolve(&local2, &remote2);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res2 {
        assert!(conflict_copy_path.contains("folder/doc.sync-conflict-20200913-122640-devA.txt"));
    } else {
        panic!("Expected KeepBoth");
    }
}

#[test]
fn test_timestamp_tiebreaker() {
    let t_same = 1600000000;

    // devB > devA, so devB wins (keeps name), devA becomes conflict copy
    let local = make_entry("doc.txt", t_same, "devA", false);
    let remote = make_entry("doc.txt", t_same, "devB", false);

    let res = ConflictResolver::resolve(&local, &remote);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res {
        assert!(conflict_copy_path.contains("doc.sync-conflict-20200913-122640-devA.txt"));
    } else {
        panic!("Expected KeepBoth");
    }

    // devC > devB, local is devC, so local wins
    let local2 = make_entry("doc.txt", t_same, "devC", false);
    let remote2 = make_entry("doc.txt", t_same, "devB", false);

    let res2 = ConflictResolver::resolve(&local2, &remote2);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res2 {
        assert!(conflict_copy_path.contains("doc.sync-conflict-20200913-122640-devB.txt"));
    } else {
        panic!("Expected KeepBoth");
    }

    // Same device, same time
    let local3 = make_entry("doc.txt", t_same, "devA", false);
    let remote3 = make_entry("doc.txt", t_same, "devA", false);
    assert_eq!(ConflictResolver::resolve(&local3, &remote3), ConflictResolution::KeepLocal);
}

#[test]
fn test_conflict_path_formatting() {
    let t = 1600000000;
    
    // Without extension
    let local = make_entry("no_extension_file", t, "device_very_long_id", false);
    let remote = make_entry("no_extension_file", t+100, "devB", false);
    
    let res = ConflictResolver::resolve(&local, &remote);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res {
        assert_eq!(conflict_copy_path, "no_extension_file.sync-conflict-20200913-122640-device_v");
    } else {
        panic!("Expected KeepBoth");
    }
}

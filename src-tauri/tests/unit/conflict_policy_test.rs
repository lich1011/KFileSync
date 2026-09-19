use kfilesync_lib::domain::model::device::DeviceId;
use kfilesync_lib::domain::model::file_entry::{ConflictResolution, EntryType, FileEntry};
use kfilesync_lib::domain::model::share::ShareId;
use kfilesync_lib::domain::service::conflict_policy;

fn make_entry(path: &str, timestamp_ms: u64, device: &str, deleted: bool) -> FileEntry {
    let mut entry = FileEntry::new(
        ShareId("s1".to_string()),
        path.to_string(),
        EntryType::File,
        &DeviceId(device.to_string()),
    );
    entry.modified_at = timestamp_ms;
    entry.deleted = deleted;
    entry
}

#[test]
fn test_delete_vs_modify() {
    // Local deleted, remote modified
    let local = make_entry("file.txt", 100, "dev1", true);
    let remote = make_entry("file.txt", 110, "dev2", false);
    assert_eq!(conflict_policy::decide(&local, &remote), ConflictResolution::KeepRemote);

    // Local modified, remote deleted
    let local2 = make_entry("file.txt", 110, "dev1", false);
    let remote2 = make_entry("file.txt", 100, "dev2", true);
    assert_eq!(conflict_policy::decide(&local2, &remote2), ConflictResolution::KeepLocal);

    // Both deleted
    let local3 = make_entry("file.txt", 100, "dev1", true);
    let remote3 = make_entry("file.txt", 110, "dev2", true);
    assert_eq!(conflict_policy::decide(&local3, &remote3), ConflictResolution::KeepLocal);
}

#[test]
fn test_newer_wins() {
    // Local newer (remote loses -> conflict copy named from remote's own timestamp/device).
    let local = make_entry("doc.txt", 1_000, "devA", false);
    let remote = make_entry("doc.txt", 0, "devB", false);

    let res1 = conflict_policy::decide(&local, &remote);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res1 {
        assert_eq!(conflict_copy_path, "doc.sync-conflict-19700101-000000-devB.txt");
    } else {
        panic!("Expected KeepBoth");
    }

    // Remote newer (local loses).
    let local2 = make_entry("folder/doc.txt", 0, "devA", false);
    let remote2 = make_entry("folder/doc.txt", 1_000, "devB", false);

    let res2 = conflict_policy::decide(&local2, &remote2);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res2 {
        assert_eq!(conflict_copy_path, "folder/doc.sync-conflict-19700101-000000-devA.txt");
    } else {
        panic!("Expected KeepBoth");
    }
}

#[test]
fn test_timestamp_tiebreaker() {
    // devB > devA lexically, so devA loses (becomes the conflict copy).
    let local = make_entry("doc.txt", 0, "devA", false);
    let remote = make_entry("doc.txt", 0, "devB", false);

    let res = conflict_policy::decide(&local, &remote);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res {
        assert_eq!(conflict_copy_path, "doc.sync-conflict-19700101-000000-devA.txt");
    } else {
        panic!("Expected KeepBoth");
    }

    // devC > devB, local is devC, so remote (devB) loses.
    let local2 = make_entry("doc.txt", 0, "devC", false);
    let remote2 = make_entry("doc.txt", 0, "devB", false);

    let res2 = conflict_policy::decide(&local2, &remote2);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res2 {
        assert_eq!(conflict_copy_path, "doc.sync-conflict-19700101-000000-devB.txt");
    } else {
        panic!("Expected KeepBoth");
    }

    // Same device, same time.
    let local3 = make_entry("doc.txt", 0, "devA", false);
    let remote3 = make_entry("doc.txt", 0, "devA", false);
    assert_eq!(conflict_policy::decide(&local3, &remote3), ConflictResolution::KeepLocal);
}

#[test]
fn test_conflict_path_formatting() {
    // Without extension; remote is newer so local (the long-id device) loses.
    let local = make_entry("no_extension_file", 0, "device_very_long_id", false);
    let remote = make_entry("no_extension_file", 100, "devB", false);

    let res = conflict_policy::decide(&local, &remote);
    if let ConflictResolution::KeepBoth { conflict_copy_path } = res {
        assert_eq!(conflict_copy_path, "no_extension_file.sync-conflict-19700101-000000-device_v");
    } else {
        panic!("Expected KeepBoth");
    }
}
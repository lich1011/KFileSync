use std::path::PathBuf;
use kfilesync_lib::domain::model::device::{Device, DeviceId, DeviceState, PairedData, Certificate};
use kfilesync_lib::domain::model::share::{Share, ShareId, SharePermission, SyncMode};
use kfilesync_lib::domain::service::specification::IgnoreSpec;

#[test]
fn test_ignore_spec() {
    // We create a spec with some additional rules
    let spec = IgnoreSpec::new(PathBuf::from("/base").as_path(), &["*.tmp", "build/"]).unwrap();

    // 1. Built-in ignores
    assert!(spec.is_ignored(PathBuf::from(".DS_Store").as_path(), false ));
    assert!(spec.is_ignored(PathBuf::from(".lansync-tmp/file.part").as_path(), false ));

    // 2. Custom ignores
    assert!(spec.is_ignored(PathBuf::from("a.tmp").as_path(), false ));
    assert!(spec.is_ignored(PathBuf::from("build").as_path(), true ));
    assert!(spec.is_ignored(PathBuf::from("build/output.bin").as_path(), false ));

    // 3. Allowed files
    assert!(!spec.is_ignored(PathBuf::from("main.rs").as_path(), false ));
    assert!(!spec.is_ignored(PathBuf::from("src/a.txt").as_path(), false ));
}

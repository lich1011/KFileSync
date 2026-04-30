use kfilesync_lib::domain::model::device::DeviceId;
use kfilesync_lib::domain::model::file_entry::VersionVector;

#[test]
fn test_is_ancestor_of() {
    let d1 = DeviceId("dev1".to_string());
    let v1 = VersionVector::new().increment(&d1);
    let v2 = v1.increment(&d1);

    assert!(v1.is_ancestor_of(&v2));
    assert!(!v2.is_ancestor_of(&v1));
    assert!(v1.is_ancestor_of(&v1));
}

#[test]
fn test_conflicts_with() {
    let d1 = DeviceId("dev1".to_string());
    let d2 = DeviceId("dev2".to_string());

    let v_base = VersionVector::new();
    let v_a = v_base.increment(&d1);
    let v_b = v_base.increment(&d2);

    assert!(v_a.conflicts_with(&v_b));
    assert!(v_b.conflicts_with(&v_a));

    let v_a_next = v_a.increment(&d1);
    assert!(v_a_next.conflicts_with(&v_b));
}

#[test]
fn test_merge() {
    let d1 = DeviceId("dev1".to_string());
    let d2 = DeviceId("dev2".to_string());

    let v_base = VersionVector::new();
    let v_a = v_base.increment(&d1).increment(&d1);
    let v_b = v_base.increment(&d2);

    let v_merged = v_a.merge(&v_b);
    assert_eq!(v_merged.0.get(&d1), Some(&2));
    assert_eq!(v_merged.0.get(&d2), Some(&1));

    assert!(v_a.is_ancestor_of(&v_merged));
    assert!(v_b.is_ancestor_of(&v_merged));
}

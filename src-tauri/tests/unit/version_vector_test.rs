use kfilesync_lib::domain::model::file_entry::VersionVector;

#[test]
fn test_is_ancestor_of() {
    let v1 = VersionVector::new().increment("dev1");
    let v2 = v1.increment("dev1");

    assert!(v1.is_ancestor_of(&v2));
    assert!(!v2.is_ancestor_of(&v1));
    assert!(v1.is_ancestor_of(&v1));
}

#[test]
fn test_conflicts_with() {
    let v_base = VersionVector::new();
    let v_a = v_base.increment("dev1");
    let v_b = v_base.increment("dev2");

    assert!(v_a.conflicts_with(&v_b));
    assert!(v_b.conflicts_with(&v_a));

    let v_a_next = v_a.increment("dev1");
    assert!(v_a_next.conflicts_with(&v_b));
}

#[test]
fn test_merge() {
    let v_base = VersionVector::new();
    let v_a = v_base.increment("dev1").increment("dev1");
    let v_b = v_base.increment("dev2");

    let v_merged = v_a.merge(&v_b);
    assert_eq!(v_merged.get("dev1"), 2);
    assert_eq!(v_merged.get("dev2"), 1);

    assert!(v_a.is_ancestor_of(&v_merged));
    assert!(v_b.is_ancestor_of(&v_merged));
}
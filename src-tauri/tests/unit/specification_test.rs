use kfilesync_core::service::ignore_spec::IgnoreSpec;

/// Sprint 6 desktop upgrade: desktop's own `IgnoreSpec` was replaced by
/// `kfilesync_core::service::ignore_spec::IgnoreSpec`. This is the
/// acceptance-criteria regression test confirming `.lansync-tmp/` is still
/// ignored when desktop calls it with `is_mobile = false` (only
/// `COMMON_DEFAULTS` applied).
#[test]
fn test_ignore_spec() {
    let spec = IgnoreSpec::build("/base", None, &["*.tmp", "build/"], false).unwrap();

    // 1. Built-in ignores (COMMON_DEFAULTS), including the .lansync-tmp/
    //    regression this test exists to guard.
    assert!(spec.is_ignored(".DS_Store", false));
    assert!(spec.is_ignored(".lansync-tmp", true));
    assert!(spec.is_ignored(".lansync-tmp/file.part", false));

    // 2. Custom (user) ignores.
    assert!(spec.is_ignored("a.tmp", false));
    assert!(spec.is_ignored("build", true));
    assert!(spec.is_ignored("build/output.bin", false));

    // 3. Allowed files.
    assert!(!spec.is_ignored("main.rs", false));
    assert!(!spec.is_ignored("src/a.txt", false));
}

#[test]
fn test_mobile_defaults_not_applied_on_desktop() {
    // Desktop always passes is_mobile = false, so MOBILE_DEFAULTS (e.g.
    // "build") must not be ignored unless the user opted in explicitly.
    let spec = IgnoreSpec::build("/base", None, &[], false).unwrap();
    assert!(!spec.is_ignored("build", true));
}
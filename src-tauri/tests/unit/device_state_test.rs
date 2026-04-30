use kfilesync_lib::domain::model::device::{Certificate, DeviceState, DiscoveredData};

#[test]
fn test_valid_transitions() {
    let initial = DeviceState::Discovered(DiscoveredData { alias: "test".into(), address: "192.168.1.5".into() });
    
    let paired = initial.clone().confirm_pairing(Certificate::mock("cert"), 12345).unwrap();
    if let DeviceState::Paired(data) = &paired {
        assert_eq!(data.alias, "test");
        assert_eq!(data.paired_at, 12345);
    } else {
        panic!("Should be Paired");
    }

    let revoked = paired.revoke(67890).unwrap();
    if let DeviceState::Revoked(data) = &revoked {
        assert_eq!(data.alias, "test");
        assert_eq!(data.revoked_at, 67890);
    } else {
        panic!("Should be Revoked");
    }
}

#[test]
fn test_invalid_transitions() {
    let initial = DeviceState::Discovered(DiscoveredData { alias: "test".into(), address: "".into() });
    let result = initial.clone().revoke(12345);
    assert!(result.is_err(), "Cannot revoke Discovered");

    let paired = initial
        .confirm_pairing(Certificate::mock("cert"), 12345).unwrap();
    
    let double_pair = paired.clone().confirm_pairing(Certificate::mock("new"), 67890);
    assert!(double_pair.is_err(), "Cannot pair an already paired device");
}

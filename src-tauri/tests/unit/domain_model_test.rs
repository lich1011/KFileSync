use kfilesync_lib::domain::error::DomainError;

#[test]
fn test_domain_error_display() {
    let e = DomainError::InvalidStateTransition("only Discovered can be paired");
    assert!(e.to_string().contains("only Discovered can be paired"));

    let e = DomainError::SessionExpired;
    assert!(e.to_string().contains("expired"));

    let e = DomainError::InvalidPinCode;
    assert!(e.to_string().contains("PIN"));

    let e = DomainError::BusinessRuleViolation("must be PEM".into());
    assert!(e.to_string().contains("must be PEM"));
}

#[test]
fn test_certificate_pem_validation() {
    use kfilesync_lib::domain::model::device::Certificate;

    // Valid PEM should succeed
    let valid = "-----BEGIN CERTIFICATE-----\nMIIBIjANBgkq\n-----END CERTIFICATE-----";
    assert!(Certificate::from_pem(valid.to_string()).is_ok());

    // Missing prefix should fail
    let invalid = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA...";
    let err = Certificate::from_pem(invalid.to_string());
    assert!(err.is_err());
    match err.unwrap_err() {
        DomainError::BusinessRuleViolation(msg) => {
            assert!(msg.contains("PEM"));
        }
        other => panic!("Expected BusinessRuleViolation, got {:?}", other),
    }

    // Empty string should fail
    let err = Certificate::from_pem("".to_string());
    assert!(err.is_err());
}

#[test]
fn test_pairing_session_verify() {
    use kfilesync_lib::domain::model::device::DeviceId;
    use kfilesync_lib::domain::model::pairing::PairingSession;

    let target = DeviceId("device_abc".to_string());
    let pin = "123456".to_string();
    let expires_at = 9999999999u64;
    let mut session = PairingSession::new(target, pin.clone(), expires_at);

    // Correct pin within time — should pass
    assert!(session.verify(&pin, 1000000000).is_ok());

    // Wrong pin — should fail with InvalidPinCode
    let mut session2 = PairingSession::new(
        DeviceId("device_abc".to_string()), pin.clone(),expires_at
    );
    let err = session2.verify("000000", 1000000000).unwrap_err();
    assert_eq!(err, DomainError::InvalidPinCode);

    // Expired — should fail with SessionExpired
    let mut session3 = PairingSession::new(
        DeviceId("device_abc".to_string()), pin.clone(),expires_at
    );
    let err = session3.verify(&pin, expires_at + 1).unwrap_err();
    assert_eq!(err, DomainError::SessionExpired);
}

#[test]
fn test_pairing_session_max_attempts(){
    use kfilesync_lib::domain::model::device::DeviceId;
    use kfilesync_lib::domain::model::pairing::PairingSession;

    let target = DeviceId("device_xyz".to_string());
    let pin = "654321".to_string();
    let mut session = PairingSession::new(target, pin.clone(), 9999999999u64);

    // 3 wrong attempts should exhuast max_attempts
    assert_eq!(session.verify("111111", 1000000000).unwrap_err(),DomainError::InvalidPinCode);
    assert_eq!(session.verify("222222", 1000000000).unwrap_err(),DomainError::InvalidPinCode);
    assert_eq!(session.verify("333333", 1000000000).unwrap_err(),DomainError::InvalidPinCode);

    // Even correct PIN should be reject after max attempts
    let err=session.verify(&pin, 1000000000).unwrap_err();
    assert!(matches!(err, DomainError::BusinessRuleViolation(_)));
    
}
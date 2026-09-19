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
    use kfilesync_core::domain::{PairingSession, SecretPin};
    use kfilesync_core::trust::pairing_state::{verify_peer_pin, PinVerdict};

    let pin = "123456".to_string();
    let expires_at = 9999999999i64;
    let make_session = || PairingSession {
        request_id: "req1".to_string(),
        target_device_id: "device_abc".to_string(),
        our_pin: SecretPin::new(pin.clone()),
        expected_their_pin: None,
        expires_at_ms: expires_at,
        attempts: 0,
        max_attempts: 3,
    };

    // Correct pin within time should pass
    let mut session = make_session();
    assert!(matches!(verify_peer_pin(&mut session, &pin, 1_000_000_000), PinVerdict::Accepted));

    // Wrong pin should fail with Wrong
    let mut session2 = make_session();
    assert!(matches!(verify_peer_pin(&mut session2, "000000", 1_000_000_000), PinVerdict::Wrong));

    // Expired should fail with Expired
    let mut session3 = make_session();
    assert!(matches!(verify_peer_pin(&mut session3, &pin, expires_at + 1), PinVerdict::Expired));
}

#[test]
fn test_pairing_session_max_attempts() {
    use kfilesync_core::domain::{PairingSession, SecretPin};
    use kfilesync_core::trust::pairing_state::{verify_peer_pin, PinVerdict};

    let pin = "654321".to_string();
    let mut session = PairingSession {
        request_id: "req2".to_string(),
        target_device_id: "device_xyz".to_string(),
        our_pin: SecretPin::new(pin.clone()),
        expected_their_pin: None,
        expires_at_ms: 9999999999,
        attempts: 0,
        max_attempts: 3,
    };

    // 3 wrong attempts should exhaust max_attempts
    assert!(matches!(verify_peer_pin(&mut session, "111111", 1_000_000_000), PinVerdict::Wrong));
    assert!(matches!(verify_peer_pin(&mut session, "222222", 1_000_000_000), PinVerdict::Wrong));
    assert!(matches!(verify_peer_pin(&mut session, "333333", 1_000_000_000), PinVerdict::Wrong));

    // Even correct PIN should be rejected after max attempts
    assert!(matches!(
        verify_peer_pin(&mut session, &pin, 1_000_000_000),
        PinVerdict::MaxAttemptsExceeded
    ));
}
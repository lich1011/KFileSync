use kfilesync_lib::infrastructure::security::nonce_validator::NonceValidator;
use kfilesync_lib::domain::error::DomainError;
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

#[test]
fn test_accept_valid_nonce() {
    let v = NonceValidator::new(300);
    assert!(v.validate("nonce1", now()).is_ok());
}

#[test]
fn test_reject_replay() {
    let v = NonceValidator::new(300);
    let ts = now();
    assert!(v.validate("nonce_dup", ts).is_ok());
    let err = v.validate("nonce_dup", ts).unwrap_err();
    assert_eq!(err, DomainError::NonceReplay);
}

#[test]
fn test_reject_old_timestamp() {
    let v = NonceValidator::new(300);
    let old = now() - 600;
    let err = v.validate("nonce_old", old).unwrap_err();
    assert_eq!(err, DomainError::TimestampOutOfWindow);
}

#[test]
fn test_reject_future_timestamp() {
    let v = NonceValidator::new(300);
    let future = now() + 600;
    let err = v.validate("nonce_future", future).unwrap_err();
    assert_eq!(err, DomainError::TimestampOutOfWindow);
}

#[test]
fn test_different_nonces_accepted() {
    let v = NonceValidator::new(300);
    let ts = now();
    assert!(v.validate("a", ts).is_ok());
    assert!(v.validate("b", ts).is_ok());
    assert!(v.validate("c", ts).is_ok());
}
use kfilesync_lib::infrastructure::security::nonce_guard::NonceGuard;
use kfilesync_lib::domain::error::DomainError;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

#[test]
fn test_accept_valid_nonce() {
    let g = NonceGuard::new(300_000, 300_000);
    assert!(g.validate("dev1", "nonce1", now_ms()).is_ok());
}

#[test]
fn test_reject_replay() {
    let g = NonceGuard::new(300_000, 300_000);
    let ts = now_ms();
    assert!(g.validate("dev1", "nonce_dup", ts).is_ok());
    let err = g.validate("dev1", "nonce_dup", ts).unwrap_err();
    assert_eq!(err, DomainError::NonceReplay);
}

#[test]
fn test_reject_old_timestamp() {
    let g = NonceGuard::new(300_000, 300_000);
    let old = now_ms() - 600_000;
    let err = g.validate("dev1", "nonce_old", old).unwrap_err();
    assert_eq!(err, DomainError::TimestampOutOfWindow);
}

#[test]
fn test_reject_future_timestamp() {
    let g = NonceGuard::new(300_000, 300_000);
    let future = now_ms() + 600_000;
    let err = g.validate("dev1", "nonce_future", future).unwrap_err();
    assert_eq!(err, DomainError::TimestampOutOfWindow);
}

#[test]
fn test_different_nonces_accepted() {
    let g = NonceGuard::new(300_000, 300_000);
    let ts = now_ms();
    assert!(g.validate("dev1", "a", ts).is_ok());
    assert!(g.validate("dev1", "b", ts).is_ok());
    assert!(g.validate("dev1", "c", ts).is_ok());
}

#[test]
fn test_different_devices_do_not_collide_on_same_nonce() {
    let g = NonceGuard::new(300_000, 300_000);
    let ts = now_ms();
    assert!(g.validate("dev1", "shared", ts).is_ok());
    assert!(g.validate("dev2", "shared", ts).is_ok());
}
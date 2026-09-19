use kfilesync_core::domain::{PairingSession, SecretPin};
use kfilesync_lib::domain::model::pairing::PairingSessionStore;

fn make_session(request_id: &str, target_device_id: &str, expires_at_ms: i64) -> PairingSession {
    PairingSession {
        request_id: request_id.to_string(),
        target_device_id: target_device_id.to_string(),
        our_pin: SecretPin::new("123456".to_string()),
        expected_their_pin: None,
        expires_at_ms,
        attempts: 0,
        max_attempts: 3,
    }
}

#[test]
fn insert_and_get_clone_round_trips() {
    let store = PairingSessionStore::new();
    store.insert(make_session("req1", "device_a", 9_999_999_999));

    let cloned = store.get_clone("req1").expect("session should be present");
    assert_eq!(cloned.request_id, "req1");
    assert_eq!(cloned.target_device_id, "device_a");

    assert!(store.get_clone("missing").is_none());
}

#[test]
fn find_request_id_by_device_matches_target() {
    let store = PairingSessionStore::new();
    store.insert(make_session("req1", "device_a", 9_999_999_999));
    store.insert(make_session("req2", "device_b", 9_999_999_999));

    assert_eq!(store.find_request_id_by_device("device_b"), Some("req2".to_string()));
    assert_eq!(store.find_request_id_by_device("unknown"), None);
}

#[test]
fn with_mut_mutates_in_place() {
    let store = PairingSessionStore::new();
    store.insert(make_session("req1", "device_a", 9_999_999_999));

    let result = store.with_mut("req1", |session| {
        session.attempts += 1;
        session.attempts
    });
    assert_eq!(result, Some(1));

    let cloned = store.get_clone("req1").unwrap();
    assert_eq!(cloned.attempts, 1);

    assert_eq!(store.with_mut("missing", |s| s.attempts), None);
}

#[test]
fn remove_takes_session_out() {
    let store = PairingSessionStore::new();
    store.insert(make_session("req1", "device_a", 9_999_999_999));

    let removed = store.remove("req1").expect("should remove existing session");
    assert_eq!(removed.request_id, "req1");
    assert!(store.get_clone("req1").is_none());
    assert!(store.remove("req1").is_none());
}

#[test]
fn sweep_expired_drops_only_expired_sessions() {
    let store = PairingSessionStore::new();
    store.insert(make_session("expired", "device_a", 1_000));
    store.insert(make_session("fresh", "device_b", 9_999_999_999));

    store.sweep_expired(2_000);

    assert!(store.get_clone("expired").is_none());
    assert!(store.get_clone("fresh").is_some());
}
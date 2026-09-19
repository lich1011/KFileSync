use std::collections::HashMap;
use std::sync::Mutex;

use kfilesync_core::domain::PairingSession;

/// Sprint 5: shared store for in-flight dual-PIN OOB pairing sessions
/// (`kfilesync_core::trust::pairing_state`, ADR-010), keyed by `request_id`.
///
/// A session may be created by either role — `start_outbound` on the
/// initiating device, or `receive_incoming` inside the inbound HTTP handler
/// on the responding device — and must later be reachable by that SAME
/// physical device's own `confirm_pairing` command, regardless of which side
/// created it. So this store is constructed once per app instance and shared
/// (via `Arc`) between the Tauri-command side (`DeviceAppService`) and the
/// inbound HTTP server (`ServerAppState`).
#[derive(Default)]
pub struct PairingSessionStore {
    sessions: Mutex<HashMap<String, PairingSession>>,
}

impl PairingSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, session: PairingSession) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(session.request_id.clone(), session);
        }
    }

    pub fn remove(&self, request_id: &str) -> Option<PairingSession> {
        self.sessions.lock().ok().and_then(|mut sessions| sessions.remove(request_id))
    }

    /// Find the `request_id` of the (most recent) session targeting the given
    /// device — used by Tauri commands, which only know the target device id,
    /// not the `request_id` `start_outbound`/`receive_incoming` generated.
    pub fn find_request_id_by_device(&self, target_device_id: &str) -> Option<String> {
        self.sessions.lock().ok().and_then(|sessions| {
            sessions
                .values()
                .find(|s| s.target_device_id == target_device_id)
                .map(|s| s.request_id.clone())
        })
    }

    /// Mutate a session in place — needed because `record_peer_pin`/
    /// `verify_peer_pin`/`prepare_confirm` all take `&mut PairingSession`.
    pub fn with_mut<T>(&self, request_id: &str, f: impl FnOnce(&mut PairingSession) -> T) -> Option<T> {
        self.sessions.lock().ok().and_then(|mut sessions| sessions.get_mut(request_id).map(f))
    }

    pub fn get_clone(&self, request_id: &str) -> Option<PairingSession> {
        self.sessions.lock().ok().and_then(|sessions| sessions.get(request_id).cloned())
    }

    /// Drop sessions whose PIN has expired, per `session.expires_at_ms`.
    pub fn sweep_expired(&self, now_ms: i64) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.retain(|_, s| s.expires_at_ms > now_ms);
        }
    }
}
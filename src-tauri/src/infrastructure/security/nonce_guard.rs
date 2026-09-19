//! Anti-replay guard backed by kfilesync-core's `nonce_window` service.
//!
//! Sprint 2 desktop upgrade: replaces the local `NonceValidator`. Timestamps
//! are now **milliseconds** (core's `verify_and_record` contract), not
//! seconds — callers must pass `timestamp_ms`.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use kfilesync_core::service::nonce_window::{verify_and_record, NonceVerdict, NonceWindowState};

use crate::domain::error::DomainError;

pub struct NonceGuard {
    state: Mutex<NonceWindowState>,
    window_size_ms: i64,
    clock_skew_ms: i64,
}

impl NonceGuard {
    pub fn new(window_size_ms: i64, clock_skew_ms: i64) -> Self {
        Self {
            state: Mutex::new(NonceWindowState::default()),
            window_size_ms,
            clock_skew_ms,
        }
    }

    pub fn validate(&self, device_id: &str, nonce: &str, timestamp_ms: i64) -> Result<(), DomainError> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let mut state = self.state.lock().unwrap();
        match verify_and_record(
            &mut state,
            device_id,
            timestamp_ms,
            nonce,
            now_ms,
            self.window_size_ms,
            self.clock_skew_ms,
        ) {
            NonceVerdict::Fresh => Ok(()),
            NonceVerdict::Replay => Err(DomainError::NonceReplay),
            NonceVerdict::StaleTimestamp | NonceVerdict::FutureTimestamp => {
                Err(DomainError::TimestampOutOfWindow)
            }
            NonceVerdict::BlankNonce => Err(DomainError::Security("blank nonce".to_string())),
        }
    }
}

impl Default for NonceGuard {
    /// 5-minute window and 5-minute clock-skew tolerance, matching the old
    /// `NonceValidator::default()` window of 300s.
    fn default() -> Self {
        Self::new(5 * 60 * 1000, 5 * 60 * 1000)
    }
}
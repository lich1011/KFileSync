use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{UNIX_EPOCH, SystemTime};

use crate::domain::error::DomainError;

pub struct NonceValidator {
    seen: Mutex<HashMap<String, u64>>,
    window_secs:u64,
}

impl NonceValidator {
    pub fn new(window_secs: u64) -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            window_secs,
        }
    }

    pub fn validate(&self, nonce: &str, timestamp: u64) -> Result<(), DomainError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let diff = now.abs_diff(timestamp);
        if diff > self.window_secs {
            return Err(DomainError::TimestampOutOfWindow);
        }

        let mut seen = self.seen.lock().unwrap();

        self.cleanup_locked(&mut seen, now);

        if seen.contains_key(nonce) {
            return Err(DomainError::NonceReplay);
        }

        seen.insert(nonce.to_string(), timestamp);
        Ok(())
    }

    fn cleanup_locked(&self, seen: &mut HashMap<String, u64>, now: u64) {
        let cutoff = now.saturating_sub(self.window_secs);
        seen.retain(|_, ts| *ts >= cutoff);
    }
}

impl Default for NonceValidator {
    fn default() -> Self {
        Self::new(60 * 5)
    }
}
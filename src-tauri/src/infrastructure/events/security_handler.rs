use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::domain::port::event_bus::DomainEvent;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

const THRESHOLD: u32 = 5;
const WINDOW_SECS: u64 = 600;

struct DeviceErrorTracker {
    errors: Vec<u64>,
}

impl DeviceErrorTracker {
    fn new() -> Self {
        Self { errors: Vec::new() }
    }

    fn record(&mut self, now: u64) -> u32 {
        let cutoff = now.saturating_sub(WINDOW_SECS);
        self.errors.retain(|&t| t > cutoff);
        self.errors.push(now);
        self.errors.len() as u32
    }
}

pub struct SecurityEventHandler {
    trackers: Mutex<HashMap<String, DeviceErrorTracker>>,
}

impl SecurityEventHandler {
    pub fn new() -> Self {
        Self {
            trackers: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start(&self, mut rx: Receiver<Arc<dyn DomainEvent>>) {
        while let Ok(event) = rx.recv().await {
            match event.event_type() {
                "TrustRevoked" | "TransferFailed" | "ChunkVerificationFailed" => {
                    self.handle_security_event(&event);
                }
                _ => {}
            }
        }
    }

    fn handle_security_event(&self, event: &Arc<dyn DomainEvent>) {
        let device_id = event.aggregate_id().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let count = {
            let mut trackers = self.trackers.lock().unwrap();
            let tracker = trackers.entry(device_id.clone()).or_insert_with(DeviceErrorTracker::new);
            tracker.record(now)
        };

        if count >= THRESHOLD {
            eprintln!(
                "[SECURITY] Device {} has {} security events in the last {} seconds - possible malicious activity",
                device_id, count, WINDOW_SECS
            );
        } else {
            println!(
                "[Security] Event '{}' for device {} (count: {}/{})",
                event.event_type(), device_id, count, THRESHOLD
            );
        }
    }
}
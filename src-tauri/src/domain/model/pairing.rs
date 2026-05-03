use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingSession {
    pub id: String,
    pub target_device: DeviceId,
    pub pin_code: String,
    pub expires_at: u64,
    pub attempts: u32,
    pub max_attempts: u32,
}

impl PairingSession {
    pub fn new(target_device: DeviceId, pin_code: String, expires_at: u64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            target_device,
            pin_code,
            expires_at,
            attempts: 0,
            max_attempts: 3,
        }
    }

    pub fn verify(&mut self, code: &str, current_time: u64) -> Result<(), DomainError> {
        if current_time > self.expires_at {
            return Err(DomainError::SessionExpired);
        }

        if self.attempts >= self.max_attempts {
            return Err(DomainError::BusinessRuleViolation(
                "Maximum PIN attempts exceeded".into()
            ));
        }
        
        self.attempts += 1;
        if self.pin_code != code {
            return Err(DomainError::InvalidPinCode);
        }
        Ok(())
    }
}

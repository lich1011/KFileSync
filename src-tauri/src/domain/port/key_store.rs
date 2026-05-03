use crate::domain::model::device::DeviceId;
use crate::domain::error::DomainError;

pub trait KeyStore: Send + Sync {
    fn store_private_key(&self, id: &DeviceId, key: &[u8]) -> Result<(), DomainError>;
    fn load_private_key(&self, id: &DeviceId) -> Result<Vec<u8>, DomainError>;
    fn delete_private_key(&self, id: &DeviceId) -> Result<(), DomainError>;
}

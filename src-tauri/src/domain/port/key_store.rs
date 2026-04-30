use crate::domain::model::device::DeviceId;

pub trait KeyStore: Send + Sync {
    fn store_private_key(&self, id: &DeviceId, key: &[u8]) -> Result<(), String>;
    fn load_private_key(&self, id: &DeviceId) -> Result<Vec<u8>, String>;
    fn delete_private_key(&self, id: &DeviceId) -> Result<(), String>;
}

use crate::domain::error::DomainError;
use crate::domain::port::key_store::KeyStore;
use crate::domain::model::device::DeviceId;
use base64::Engine;

const SERVICE_NAME: &str = "com.kfilesync.passphrase";

pub struct PlatformKeyStore;

impl PlatformKeyStore {
    pub fn new() -> Self {
        Self
    }   

    pub fn is_available() -> bool {
        let test_entry= keyring::Entry::new(SERVICE_NAME, "__availability_check__");
        test_entry.is_ok()   
    }
    
    fn entry(&self, id: &DeviceId) -> Result<keyring::Entry,DomainError> {
        keyring::Entry::new(SERVICE_NAME, &id.0)
            .map_err(|e| DomainError::Security(format!("Failed to create keyring entry: {}",e)))
    } 

}

impl Default for PlatformKeyStore {
    fn default() -> Self { Self::new() }
}

impl KeyStore for PlatformKeyStore {
    fn store_private_key(&self, id: &DeviceId, key: &[u8]) -> Result<(), DomainError> {
        let encoded: String = base64::engine::general_purpose::STANDARD.encode(key);
        let entry = self.entry(id)?;
        entry.set_password(encoded.as_str())
            .map_err(|e| DomainError::Security(format!("Failed to store key in platform keystore: {}", e)))
    }

    fn load_private_key(&self, id: &DeviceId) -> Result<Vec<u8>, DomainError> {
        let entry = self.entry(id)?;
        let encoded = entry.get_password()
            .map_err(|e| DomainError::Security(format!("Failed to load key from platform keystore: {}", e)))?;
        let encoded_str = encoded;
        base64::engine::general_purpose::STANDARD
            .decode(&encoded_str)
            .map_err(|e| DomainError::Security(format!("Failed to decode key from platform keystore: {}", e)))
    }

    fn delete_private_key(&self, id: &DeviceId) -> Result<(), DomainError> {
        let entry = self.entry(id)?;
        match entry.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(DomainError::Security(format!("Failed to delete key from platform keystore: {}", e))),
        }
    }
}


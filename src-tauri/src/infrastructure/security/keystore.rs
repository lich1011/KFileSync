use crate::domain::model::device::DeviceId;
use crate::domain::port::key_store::KeyStore;
use crate::domain::error::DomainError;  
use std::fs;
use std::path::PathBuf;
// rcgen is used in generate_self_signed_cert()

pub struct FileKeyStore {
    storage_dir: PathBuf,
}

impl FileKeyStore {
    pub fn new(storage_dir: PathBuf) -> Self {
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir).expect("Failed to create keystore directory");
        }
        Self { storage_dir }
    }

    fn key_path(&self, id: &DeviceId) -> PathBuf {
        self.storage_dir.join(format!("{}.key", id.0))
    }
}

impl KeyStore for FileKeyStore {
    fn store_private_key(&self, id: &DeviceId, key: &[u8]) -> Result<(), DomainError> {
        let path = self.key_path(id);
        fs::write(&path, key).map_err(|e| DomainError::Security(format!("Failed to write key: {}", e)))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&path, perms)
                .map_err(|e| DomainError::Security(format!("Failed to set key file permission: {}",e)))?;
        }
        Ok(())
    }

    fn load_private_key(&self, id: &DeviceId) -> Result<Vec<u8>, DomainError> {
        let path = self.key_path(id);
        fs::read(path).map_err(|e| DomainError::Security(format!("Failed to read key: {}", e)))
    }

    fn delete_private_key(&self, id: &DeviceId) -> Result<(), DomainError> {
        let path = self.key_path(id);
        if path.exists() {
            fs::remove_file(path).map_err(|e| DomainError::Security(format!("Failed to delete key: {}", e)))
        } else {
            Ok(())
        }
    }
}

/// Helper function to generate a self-signed Ed25519 certificate
pub fn generate_self_signed_cert() -> Result<(String, String, Vec<u8>), DomainError> {
    let subject_alt_names = vec!["lansync.local".to_string()];  
    
    let cert = rcgen::generate_simple_self_signed(subject_alt_names)
        .map_err(|e| DomainError::Security(format!("Cert generation failed: {}", e)))?;

    let cert_der = cert.serialize_der()
        .map_err(|e| DomainError::Security(format!("Cert DER serialization failed: {}", e)))?;  

    let cert_pem = cert.serialize_pem()
        .map_err(|e| DomainError::Security(format!("Cert serialization failed: {}", e)))?;

    let pk_pem = cert.serialize_private_key_pem();

    Ok((cert_pem, pk_pem, cert_der))
}

pub fn device_id_from_cert_der(cert_der: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(cert_der);
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn fingerprint_short(device_id: &str) -> String {
    let hex = if device_id.len() >= 16 { &device_id[0..16]} else { device_id };
    hex.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or("????"))
        .collect::<Vec<_>>()
        .join("-")
}   
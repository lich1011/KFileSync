use crate::domain::model::device::DeviceId;
use crate::domain::port::key_store::KeyStore;
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
    fn store_private_key(&self, id: &DeviceId, key: &[u8]) -> Result<(), String> {
        let path = self.key_path(id);
        fs::write(path, key).map_err(|e| format!("Failed to write key: {}", e))
    }

    fn load_private_key(&self, id: &DeviceId) -> Result<Vec<u8>, String> {
        let path = self.key_path(id);
        fs::read(path).map_err(|e| format!("Failed to read key: {}", e))
    }

    fn delete_private_key(&self, id: &DeviceId) -> Result<(), String> {
        let path = self.key_path(id);
        if path.exists() {
            fs::remove_file(path).map_err(|e| format!("Failed to delete key: {}", e))
        } else {
            Ok(())
        }
    }
}

/// Helper function to generate a self-signed certificate.
/// Returns (cert_pem, key_pem) both as valid PEM strings.
pub fn generate_self_signed_cert() -> Result<(String, String), String> {
    let subject_alt_names = vec!["lansync.local".to_string()];
    let cert = rcgen::generate_simple_self_signed(subject_alt_names)
        .map_err(|e| format!("Cert generation failed: {}", e))?;
    let cert_pem = cert.serialize_pem()
        .map_err(|e| format!("Cert serialization failed: {}", e))?;
    let pk_pem = cert.serialize_private_key_pem();
    Ok((cert_pem, pk_pem))
}

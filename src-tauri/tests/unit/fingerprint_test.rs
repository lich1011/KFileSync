use kfilesync_lib::infrastructure::security::keystore::fingerprint_short;

#[test]
fn test_fingerprint_short_normal() {
    let device_id = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
    let fp = fingerprint_short(device_id);
    assert_eq!(fp, "abcd-ef01-2345-6789");
}

#[test]
fn test_fingerprint_short_exactly_16() {
    let device_id = "1234567890abcdef";
    let fp = fingerprint_short(device_id);
    assert_eq!(fp, "1234-5678-90ab-cdef");
}

#[test]
fn test_fingerprint_short_shorter_than_16() {
    let device_id = "abcd1234";
    let fp = fingerprint_short(device_id);
    assert_eq!(fp, "abcd-1234");
}

#[test]
fn test_compute_sha256() {
    use kfilesync_lib::infrastructure::security::chunk_hasher::ChunkHasher;
    use sha2::{Sha256, Digest};

    let dir = std::env::temp_dir().join("kfilesync_test_sha256");
    let _ = std::fs::create_dir_all(&dir);
    let file_path = dir.join("sha256_test.bin");
    let data = b"hello world sha256 test data";
    std::fs::write(&file_path, data).unwrap();

    let hash = ChunkHasher::compute_sha256(&file_path).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(data);
    let expected: String = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect();

    assert_eq!(hash, expected);

    let _ = std::fs::remove_file(&file_path);
}
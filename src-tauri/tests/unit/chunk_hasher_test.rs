use kfilesync_lib::infrastructure::security::chunk_hasher::ChunkHasher;

#[test]
fn test_hash_file_single_chunk() {
    let dir = std::env::temp_dir().join("kfilesync_test_hash");
    let _ = std::fs::create_dir_all(&dir);
    let file_path = dir.join("single_chunk.bin");
    let data = b"hello world, this is test data for BLAKE3 hashing";
    std::fs::write(&file_path, data).unwrap();

    let chunks = ChunkHasher::hash_file_chunks(&file_path, 0).unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].index, 0);
    assert_eq!(chunks[0].offset, 0);
    assert_eq!(chunks[0].size, data.len() as u32);

    let expected = blake3::hash(data).to_hex().to_string();
    assert_eq!(chunks[0].hash, expected);

    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn test_hash_file_multiple_chunks() {
    let dir = std::env::temp_dir().join("kfilesync_test_hash");
    let _ = std::fs::create_dir_all(&dir);
    let file_path = dir.join("multi_chunk.bin");

    let data = vec![0xABu8; 1000];
    std::fs::write(&file_path, &data).unwrap();

    let chunks = ChunkHasher::hash_file_chunks(&file_path, 400).unwrap();
    assert_eq!(chunks.len(), 3); // 400 + 400 + 200

    assert_eq!(chunks[0].size, 400);
    assert_eq!(chunks[1].size, 400);
    assert_eq!(chunks[2].size, 200);

    assert_eq!(chunks[0].offset, 0);
    assert_eq!(chunks[1].offset, 400);
    assert_eq!(chunks[2].offset, 800);

    let expected_0 = blake3::hash(&data[0..400]).to_hex().to_string();
    let expected_1 = blake3::hash(&data[400..800]).to_hex().to_string();
    let expected_2 = blake3::hash(&data[800..1000]).to_hex().to_string();
    
    assert_eq!(chunks[0].hash, expected_0);
    assert_eq!(chunks[1].hash, expected_1);
    assert_eq!(chunks[2].hash, expected_2);

    let _ = std::fs::remove_file(&file_path);
}

#[test]
fn test_verify_chunk() {
    let data = b"some chunk data for verification";
    let hash = blake3::hash(data).to_hex().to_string();

    assert!(ChunkHasher::verify_chunk(data, &hash));
    assert!(!ChunkHasher::verify_chunk(b"wrong data", &hash));
    assert!(!ChunkHasher::verify_chunk(data, "0000000000000000000000000000000000000000000000000000000000000000"));
}

#[test]
fn test_device_id_from_cert_der() {
    use kfilesync_lib::infrastructure::security::keystore::device_id_from_cert_der;

    let known_der = b"test certificate DER bytes";
    let id = device_id_from_cert_der(known_der);

    // Verify it's a valid 64-char hex string (SHA-256)
    assert_eq!(id.len(), 64);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));

    // Same input produces same output
    let id2 = device_id_from_cert_der(known_der);
    assert_eq!(id, id2);

    // Different input produces different output
    let id3 = device_id_from_cert_der(b"different DER bytes");
    assert_ne!(id, id3);
}
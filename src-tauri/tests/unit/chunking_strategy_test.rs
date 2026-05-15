use kfilesync_lib::domain::service::chunking::compute_chunk_size;

#[test]
fn test_size_based_chunking() {
    // let strategy = SizeBasedChunking::new();

    // < 128K
    assert_eq!(compute_chunk_size(100_000), 0);
    assert_eq!(compute_chunk_size(131_072), 0);

    // ~ 128K
    assert_eq!(compute_chunk_size(131_073), 131_072);
    assert_eq!(compute_chunk_size(268_435_456), 131_072);

    // ~ 1M
    assert_eq!(compute_chunk_size(268_435_457), 1_048_576);
    assert_eq!(compute_chunk_size(1_073_741_824), 1_048_576);

    // ~ 4M
    assert_eq!(compute_chunk_size(1_073_741_825), 4_194_304);
    assert_eq!(compute_chunk_size(17_179_869_184), 4_194_304);

    // ~ 16M
    assert_eq!(compute_chunk_size(17_179_869_185), 16_777_216);
}

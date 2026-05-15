pub fn compute_chunk_size(file_size: u64) -> u32 {
    match file_size {
        0..=131_072 => 0,                         // < 128K: no chunking
        131_073..=268_435_456 => 131_072,         // ~ 128K
        268_435_457..=1_073_741_824 => 1_048_576, // ~ 1M
        1_073_741_825..=17_179_869_184 => 4_194_304, // ~ 4M
        _ => 16_777_216,                          // ~ 16M
    }
}
// pub trait ChunkingStrategy: Send + Sync {
//     fn compute_chunk_size(&self, file_size: u64) -> u32;
// }

// pub struct SizeBasedChunking;

// impl SizeBasedChunking {
//     pub fn new() -> Self {
//         Self {}
//     }
// }

// impl Default for SizeBasedChunking {
//     fn default() -> Self {
//         Self::new()
//     }
// }

// impl ChunkingStrategy for SizeBasedChunking {
    
// }

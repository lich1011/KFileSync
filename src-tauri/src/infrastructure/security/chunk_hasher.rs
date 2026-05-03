use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use crate::domain::model::transfer::ChunkInfo;
use crate::domain::error::DomainError;

fn fs_error(e: impl std::fmt::Display) -> DomainError {
    DomainError::FileSystem(e.to_string())
}
  
pub struct ChunkHasher;

impl ChunkHasher {

    pub fn hash_file_chunks(file_path: &Path, chunk_size: u32) ->Result<Vec<ChunkInfo>, DomainError> {
        let file = std::fs::File::open(file_path)
            .map_err(|e| DomainError::FileSystem(format!("Failed to open {:?}: {}", file_path.display(), e)))?;
        let file_size = file.metadata()
            .map_err(fs_error)?.len();
        let mut reader = BufReader::new(file);
        let mut chunks = Vec::new();

        if chunk_size == 0 {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).map_err(fs_error)?;
            let chunk_hash = blake3::hash(&buf);
            chunks.push(ChunkInfo {
                index: 0,
                offset: 0,
                size: file_size as u32,
                hash: chunk_hash.to_hex().to_string(),
            });
        } else {
            let mut offset = 0u64;
            let mut index = 0u32;
            let mut buf = vec![0u8; chunk_size as usize];

            while offset < file_size {
                let to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
                reader.read_exact(&mut buf[..to_read]).map_err(fs_error)?;
                let chunk_hash = blake3::hash(&buf[..to_read]);
                chunks.push(ChunkInfo {
                    index,
                    offset,
                    size: to_read as u32,
                    hash: chunk_hash.to_hex().to_string(),
                });
                offset += to_read as u64;
                index += 1;
                
            } 
        }
    
        Ok(chunks)
    }

    pub fn verify_chunk(data: &[u8],expected_hash: &str)->bool {
        let hash = blake3::hash(data);
        hash.to_hex().as_str() == expected_hash
    }

    pub fn read_chunk(file_path: &Path,offset: u64,size: u64)->Result<Vec<u8>,DomainError> {
        let mut file = std::fs::File::open(file_path)
            .map_err(|e| DomainError::FileSystem(format!("Failed to open {:?}: {}", file_path.display(), e)))?;
        file.seek(SeekFrom::Start(offset)).map_err(fs_error)?;
        let mut buf = vec![0u8; size as usize];
        file.read_exact(&mut buf).map_err(fs_error)?;
        Ok(buf)
    }

    pub fn compute_sha256(file_path: &Path)->Result<String,DomainError> {
        use sha2::{Digest, Sha256};

        let file = std::fs::File::open(file_path)
            .map_err(|e| DomainError::FileSystem(format!("Failed to open {:?}: {}", file_path.display(), e)))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let bytes_read = reader.read(&mut buf).map_err(fs_error)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buf[..bytes_read]);
        }
        let result = hasher.finalize();
        Ok(result.iter().map(|b| format!("{:02x}", b)).collect())
    }   

}


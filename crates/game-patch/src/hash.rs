use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

use crate::Result;

pub type Hash = [u8; 32];

pub fn bytes(data: &[u8]) -> Hash {
    Sha256::digest(data).into()
}

pub fn file(path: &Path) -> Result<Hash> {
    let mut input = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().into())
}

pub fn hex(hash: &Hash) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn parse(value: &str) -> Result<Hash> {
    if value.len() != 64 {
        return Err(format!("invalid SHA-256 length {}", value.len()));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("invalid SHA-256 {value}"))?;
    }
    Ok(output)
}

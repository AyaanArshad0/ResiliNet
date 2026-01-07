use std::io::Cursor;
use zstd::stream::copy_encode;
use zstd::stream::copy_decode;
use anyhow::{Result, Context};

/// Known high-entropy headers.
/// If we see these, we skip compression to save CPU cycles.
const MAGIC_HEADERS: &[&[u8]] = &[
    &[0xFF, 0xD8, 0xFF],       // JPEG
    &[0x89, 0x50, 0x4E, 0x47], // PNG
    &[0x50, 0x4B, 0x03, 0x04], // ZIP/JAR
    &[0x1F, 0x8B],             // GZIP
];

/// Heuristic check for high entropy data.
fn is_high_entropy(data: &[u8]) -> bool {
    if data.len() < 4 { return false; }
    for magic in MAGIC_HEADERS {
        if data.starts_with(magic) { return true; }
    }
    false
}

/// "Adaptive" Compression.
/// 
/// STRATEGY:
/// 1. Check if data looks like it's already compressed (Images, Zip).
/// 2. If yes, skip Zstd (CPU expensive, 0% gain).
/// 3. If no, compress with Zstd Level 3 (Sweet spot for real-time traffic).
///
/// Returns: [FLAG (1B) | PAYLOAD]
pub fn adaptive_compress(data: &[u8]) -> Result<Vec<u8>> {
    // Flag: 0 = Raw, 1 = Compressed
    
    if is_high_entropy(data) {
        let mut out = Vec::with_capacity(data.len() + 1);
        out.push(0u8); 
        out.extend_from_slice(data);
        return Ok(out);
    }

    let mut out = Vec::with_capacity(data.len());
    out.push(1u8); 
    
    // Zstd Level 3 is standard. 
    // TODO: Make compression level configurable via TunOptions.
    copy_encode(Cursor::new(data), &mut out, 3).context("Zstd::EncodeFail")?;
    
    Ok(out)
}

pub fn adaptive_decompress(data: &[u8]) -> Result<Vec<u8>> {
    if data.is_empty() { return Ok(vec![]); }

    let flag = data[0];
    let content = &data[1..];

    match flag {
        0 => Ok(content.to_vec()), // Pass-through
        1 => {
            let mut out = Vec::new();
            copy_decode(Cursor::new(content), &mut out).context("Zstd::DecodeFail")?;
            Ok(out)
        }
        _ => anyhow::bail!("Compression::UnknownFlag: {}", flag),
    }
}

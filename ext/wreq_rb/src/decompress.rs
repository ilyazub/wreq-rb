use brotli::Decompressor;
use flate2::read::{DeflateDecoder, GzDecoder};
use std::io::Read;

pub fn decompress(data: &[u8], encoding: &str) -> Result<Vec<u8>, String> {
    // Handle multiple encodings (split on comma, reverse order)
    if encoding.contains(',') {
        let encodings: Vec<&str> = encoding.split(',').map(|s| s.trim()).collect();
        let mut result = data.to_vec();
        for enc in encodings.iter().rev() {
            result = decompress_single(&result, enc)?;
        }
        return Ok(result);
    }

    decompress_single(data, encoding)
}

fn decompress_single(data: &[u8], encoding: &str) -> Result<Vec<u8>, String> {
    let encoding_lower = encoding.trim().to_lowercase();

    match encoding_lower.as_str() {
        "gzip" | "x-gzip" => {
            let mut decoder = GzDecoder::new(data);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| format!("gzip decompression error: {}", e))?;
            Ok(decompressed)
        }
        "deflate" => {
            let mut decoder = DeflateDecoder::new(data);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| format!("deflate decompression error: {}", e))?;
            Ok(decompressed)
        }
        "br" => {
            let mut decoder = Decompressor::new(data, 4096);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| format!("brotli decompression error: {}", e))?;
            Ok(decompressed)
        }
        "zstd" => {
            zstd::stream::decode_all(data).map_err(|e| format!("zstd decompression error: {}", e))
        }
        "identity" | "" => Ok(data.to_vec()),
        _ => Err(format!("unsupported encoding: {}", encoding)),
    }
}

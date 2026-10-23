//! Decoding bodies for display. The proxy forwards them untouched.

use std::io::Read;

use emulabzip_core::{Error, Result};

/// A decoded body is cut off here, which also guards against decompression bombs.
pub const DECODED_LIMIT: u64 = 50 * 1024 * 1024;

/// Undoes a `Content-Encoding` (several can be stacked, listed in the order they were applied).
pub fn decode(content_encoding: Option<&str>, bytes: &[u8]) -> Result<Vec<u8>> {
    let mut data = bytes.to_vec();
    let Some(encodings) = content_encoding else { return Ok(data) };
    for encoding in encodings.split(',').map(|e| e.trim().to_ascii_lowercase()).rev() {
        data = match encoding.as_str() {
            "" | "identity" => data,
            "gzip" | "x-gzip" => read_all(flate2::read::GzDecoder::new(&data[..]))?,
            "deflate" => {
                // Servers disagree on whether "deflate" means zlib or raw deflate.
                read_all(flate2::read::ZlibDecoder::new(&data[..]))
                    .or_else(|_| read_all(flate2::read::DeflateDecoder::new(&data[..])))?
            }
            "br" => read_all(brotli::Decompressor::new(&data[..], 4096))?,
            "zstd" => read_all(zstd::Decoder::new(&data[..])?)?,
            other => return Err(Error::Invalid { what: "content encoding", value: other.to_string() }),
        };
    }
    Ok(data)
}

fn read_all(reader: impl Read) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    reader.take(DECODED_LIMIT).read_to_end(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const TEXT: &[u8] = b"{\"hello\":\"world\",\"n\":[1,2,3]}";

    #[test]
    fn passes_plain_bodies_through() {
        assert_eq!(decode(None, TEXT).unwrap(), TEXT);
        assert_eq!(decode(Some("identity"), TEXT).unwrap(), TEXT);
    }

    #[test]
    fn decodes_gzip_deflate_brotli_and_zstd() {
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(TEXT).unwrap();
        assert_eq!(decode(Some("gzip"), &gz.finish().unwrap()).unwrap(), TEXT);

        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(TEXT).unwrap();
        assert_eq!(decode(Some("deflate"), &z.finish().unwrap()).unwrap(), TEXT);

        let mut raw = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        raw.write_all(TEXT).unwrap();
        assert_eq!(decode(Some("deflate"), &raw.finish().unwrap()).unwrap(), TEXT);

        let mut br = Vec::new();
        brotli::CompressorWriter::new(&mut br, 4096, 5, 22).write_all(TEXT).unwrap();
        assert_eq!(decode(Some("br"), &br).unwrap(), TEXT);

        assert_eq!(decode(Some("zstd"), &zstd::encode_all(TEXT, 3).unwrap()).unwrap(), TEXT);
    }

    #[test]
    fn rejects_unknown_encodings_and_corrupt_data() {
        assert!(decode(Some("rot13"), TEXT).is_err());
        assert!(decode(Some("gzip"), b"not gzip").is_err());
    }
}

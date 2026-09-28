use crate::error::{Error, Result};
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Strategy for stream data decompression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    FlateDecode,
    AsciiHexDecode,
    Ascii85Decode,
    Identity,
}

/// A zero-copy or bounded-memory stream reader.
/// Defers decompression until explicitly queried, enabling parsing of 10GB+ files
/// without blowing memory limits.
pub struct StreamView<'a> {
    raw_data: &'a [u8],
    filters: Vec<FilterKind>,
}

impl<'a> StreamView<'a> {
    pub fn new(raw_data: &'a [u8]) -> Self {
        Self {
            raw_data,
            filters: Vec::new(),
        }
    }

    pub fn with_filter(mut self, filter: FilterKind) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn raw_bytes(&self) -> &'a [u8] {
        self.raw_data
    }

    pub fn len(&self) -> usize {
        self.raw_data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.raw_data.is_empty()
    }

    /// Maximum number of bytes that will be decompressed from a single stream.
    /// Prevents zip-bomb OOM on malformed or adversarial PDFs.
    pub const MAX_DECOMPRESS_BYTES: usize = 256 * 1024 * 1024; // 256 MB

    /// Decompresses stream payload into memory only when demanded.
    /// Returns `Error::Unsupported` if decompressed size exceeds `MAX_DECOMPRESS_BYTES`.
    pub fn decode(&self) -> Result<Vec<u8>> {
        if self.filters.is_empty() || self.filters.contains(&FilterKind::Identity) {
            return Ok(self.raw_data.to_vec());
        }

        let mut current = self.raw_data.to_vec();

        for filter in &self.filters {
            match filter {
                FilterKind::FlateDecode => {
                    let mut decoder = ZlibDecoder::new(&current[..]);
                    let mut decompressed = Vec::new();
                    // Read up to MAX_DECOMPRESS_BYTES + 1 to detect overflow without panicking.
                    decoder
                        .by_ref()
                        .take((Self::MAX_DECOMPRESS_BYTES + 1) as u64)
                        .read_to_end(&mut decompressed)
                        .map_err(|e| Error::Io(format!("Flate decompression failed: {e}")))?;
                    if decompressed.len() > Self::MAX_DECOMPRESS_BYTES {
                        return Err(Error::Unsupported(
                            "decompressed stream exceeds 256 MB safety limit",
                        ));
                    }
                    current = decompressed;
                }
                FilterKind::Identity => {}
                _ => return Err(Error::Unsupported("unsupported stream filter")),
            }
        }

        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    #[test]
    fn test_lazy_flate_stream() {
        let payload = b"BT /F1 24 Tf 100 700 Td (Hello oxpdf lazy stream!) Tj ET";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).unwrap();
        let compressed = encoder.finish().unwrap();

        let stream_view = StreamView::new(&compressed).with_filter(FilterKind::FlateDecode);
        assert_eq!(stream_view.len(), compressed.len());

        let decoded = stream_view.decode().unwrap();
        assert_eq!(decoded, payload);
    }
}

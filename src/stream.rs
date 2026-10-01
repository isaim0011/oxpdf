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
        if self.raw_data.len() > Self::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "stream exceeds 256 MB safety limit",
            ));
        }

        if self.filters.is_empty()
            || (self.filters.len() == 1 && self.filters[0] == FilterKind::Identity)
        {
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
                FilterKind::AsciiHexDecode => {
                    current = decode_ascii_hex(&current)?;
                }
                FilterKind::Ascii85Decode => {
                    current = decode_ascii85(&current)?;
                }
                FilterKind::Identity => {}
            }

            if current.len() > Self::MAX_DECOMPRESS_BYTES {
                return Err(Error::Unsupported(
                    "decompressed stream exceeds 256 MB safety limit",
                ));
            }
        }

        Ok(current)
    }
}

/// Decodes standard PDF AsciiHexDecode byte streams with strict safety bounds.
fn decode_ascii_hex(input: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity((input.len() / 2).min(StreamView::MAX_DECOMPRESS_BYTES));
    let mut first_nibble: Option<u8> = None;

    for &b in input {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'>' {
            break;
        }
        let val = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => {
                return Err(Error::InvalidHexString(out.len()));
            }
        };

        if let Some(high) = first_nibble.take() {
            if out.len() >= StreamView::MAX_DECOMPRESS_BYTES {
                return Err(Error::Unsupported(
                    "decompressed stream exceeds 256 MB safety limit",
                ));
            }
            out.push((high << 4) | val);
        } else {
            first_nibble = Some(val);
        }
    }

    // PDF spec: if odd number of hex digits before '>', pad with '0'
    if let Some(high) = first_nibble {
        if out.len() >= StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "decompressed stream exceeds 256 MB safety limit",
            ));
        }
        out.push(high << 4);
    }

    Ok(out)
}

/// Decodes standard PDF Ascii85Decode (btoa) streams with strict safety bounds.
fn decode_ascii85(input: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len().min(StreamView::MAX_DECOMPRESS_BYTES));
    let mut tuple = 0u32;
    let mut count = 0;
    let mut i = 0;

    while i < input.len() {
        let b = input[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'~' && i + 1 < input.len() && input[i + 1] == b'>' {
            break;
        }
        if b == b'z' && count == 0 {
            if out.len() + 4 > StreamView::MAX_DECOMPRESS_BYTES {
                return Err(Error::Unsupported(
                    "decompressed stream exceeds 256 MB safety limit",
                ));
            }
            out.extend_from_slice(&[0, 0, 0, 0]);
            i += 1;
            continue;
        }
        if !(b'!'..=b'u').contains(&b) {
            return Err(Error::Unsupported("Invalid Ascii85 character"));
        }

        tuple = tuple.saturating_mul(85).saturating_add((b - b'!') as u32);
        count += 1;

        if count == 5 {
            if out.len() + 4 > StreamView::MAX_DECOMPRESS_BYTES {
                return Err(Error::Unsupported(
                    "decompressed stream exceeds 256 MB safety limit",
                ));
            }
            out.extend_from_slice(&tuple.to_be_bytes());
            tuple = 0;
            count = 0;
        }
        i += 1;
    }

    // Handle partial group at end
    if count > 1 {
        for _ in count..5 {
            tuple = tuple.saturating_mul(85).saturating_add(84); // pad with 'u' - '!' = 84
        }
        let bytes = tuple.to_be_bytes();
        let needed = count - 1;
        if out.len() + needed > StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "decompressed stream exceeds 256 MB safety limit",
            ));
        }
        out.extend_from_slice(&bytes[..needed]);
    }

    Ok(out)
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

    #[test]
    fn test_ascii_hex_stream() {
        let hex_payload = b"48656c6c6f206f7870646621>"; // "Hello oxpdf!"
        let stream = StreamView::new(hex_payload).with_filter(FilterKind::AsciiHexDecode);
        let decoded = stream.decode().unwrap();
        assert_eq!(decoded, b"Hello oxpdf!");
    }

    #[test]
    fn test_ascii85_stream() {
        // "Hello world!" in ASCII85 is "87cURD]j7BEbo80~>"
        let a85_payload = b"87cURD]j7BEbo80~>";
        let stream = StreamView::new(a85_payload).with_filter(FilterKind::Ascii85Decode);
        let decoded = stream.decode().unwrap();
        assert_eq!(decoded, b"Hello world!");
    }
}

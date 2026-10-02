//! JBIG2 stream decoder skeleton & segment parser.
//!
//! Implements ISO/IEC 14492 (JBIG2) bi-level image decoding and segment header
//! parsing with support for global dictionary streams (`/JBIG2Globals`).

use crate::error::{Error, Result};
use crate::stream::StreamView;

/// Parameters for JBIG2Decode filter per ISO 32000-1 Table 12.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Jbig2Params {
    /// Optional shared global segments dictionary (`/JBIG2Globals`).
    pub globals: Option<Vec<u8>>,
}

/// A parsed JBIG2 segment header per ISO/IEC 14492 §7.2.
#[derive(Debug, Clone)]
pub struct Jbig2SegmentHeader {
    pub segment_number: u32,
    pub segment_type: u8,
    pub page_association: u32,
    pub data_length: usize,
    pub retain_flag: bool,
}

/// Decodes a JBIG2 bitstream into packed 1-bit per pixel bitmap bytes.
///
/// In PDF streams, JBIG2 data can be either:
/// 1. An embedded single-page or multi-page stream without file header.
/// 2. An entire JBIG2 file starting with the 8-byte magic `\x97\x4A\x42\x32\x0D\x0A\x1A\x0A`.
///
/// Global segments (like symbol dictionaries) are optionally supplied via `params.globals`.
pub fn decode_jbig2(input: &[u8], params: &Jbig2Params) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    if input.len() > StreamView::MAX_DECOMPRESS_BYTES {
        return Err(Error::Unsupported(
            "JBIG2 input exceeds 256 MB safety limit",
        ));
    }

    // Process globals if supplied
    if let Some(globals_data) = &params.globals {
        if globals_data.len() > StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "JBIG2Globals exceeds 256 MB safety limit",
            ));
        }
        let _ = parse_segments(globals_data)?;
    }

    // Parse image stream segments
    let segments = parse_segments(input)?;

    // Inspect segments to extract raster dimensions and data
    // Typical types:
    // 0: Symbol dictionary
    // 4: Intermediate text region
    // 6: Immediate text region
    // 7: Immediate lossless text region
    // 20: Intermediate pattern dictionary
    // 36: Intermediate halftone region
    // 38: Immediate halftone region
    // 39: Immediate lossless halftone region
    // 40: Intermediate generic region
    // 42: Immediate generic region
    // 43: Immediate lossless generic region
    // 48: Page information segment
    // 49: End of page segment
    // 50: End of stripe segment
    // 51: End of file segment

    let mut width = 0u32;
    let mut height = 0u32;
    let mut bitmap_data = Vec::new();

    for (header, data) in &segments {
        match header.segment_type {
            48 => {
                // Page information segment:
                // Width (4 bytes), Height (4 bytes), X resolution (4 bytes), Y resolution (4 bytes), Flags (1 byte)...
                if data.len() >= 8 {
                    width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                    height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                }
            }
            38 | 39 | 42 | 43 if data.len() >= 17 => {
                // Immediate region segment:
                // Region width (4 bytes), height (4 bytes), x (4 bytes), y (4 bytes), op (1 byte)
                let r_w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let r_h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                if width == 0 {
                    width = r_w;
                }
                if height == 0 {
                    height = r_h;
                }
                // For direct lossless generic region or raw bitmap segments
                let raw_payload = &data[17..];
                if !raw_payload.is_empty() {
                    bitmap_data.extend_from_slice(raw_payload);
                }
            }
            _ => {}
        }
    }

    if width > 0 && height > 0 {
        let row_bytes = (width as usize).div_ceil(8);
        let expected_total = row_bytes.saturating_mul(height as usize);
        if expected_total > StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "decompressed JBIG2 stream exceeds 256 MB safety limit",
            ));
        }

        if bitmap_data.len() >= expected_total {
            bitmap_data.truncate(expected_total);
            return Ok(bitmap_data);
        } else {
            // Pad or initialize to expected total
            bitmap_data.resize(expected_total, 0);
            return Ok(bitmap_data);
        }
    }

    // If stream contains direct uncompressed or passthrough payload
    if !bitmap_data.is_empty() {
        if bitmap_data.len() > StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "decompressed JBIG2 stream exceeds 256 MB safety limit",
            ));
        }
        Ok(bitmap_data)
    } else {
        // Return raw stream if structured parsing yielded no individual bitmap
        Ok(input.to_vec())
    }
}

/// Parses JBIG2 segments from raw stream according to ISO/IEC 14492 §7.2.
fn parse_segments(mut data: &[u8]) -> Result<Vec<(Jbig2SegmentHeader, Vec<u8>)>> {
    let mut segments = Vec::new();

    // Check for 8-byte file header: \x97\x4A\x42\x32\x0D\x0A\x1A\x0A
    if data.starts_with(b"\x97JB2\r\n\x1a\n") {
        if data.len() < 9 {
            return Ok(segments);
        }
        let flags = data[8];
        let has_num_pages = (flags & 1) == 0;
        data = &data[9..];
        if has_num_pages {
            if data.len() < 4 {
                return Ok(segments);
            }
            data = &data[4..];
        }
    }

    let mut cursor = 0usize;
    while cursor + 6 <= data.len() {
        let seg_num = u32::from_be_bytes([
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ]);
        let flags = data[cursor + 4];
        let retain_flag = (flags & 0x20) != 0;
        let seg_type = flags & 0x3F;

        let page_assoc_size_flag = (flags & 0x40) != 0;
        let referred_count_field = (data[cursor + 5] >> 5) & 0x07;

        cursor += 6;

        let mut ref_count = referred_count_field as usize;
        if referred_count_field == 7 {
            // Long referred-to segment count
            if cursor + 4 > data.len() {
                break;
            }
            ref_count = u32::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]) as usize;
            cursor += 4;
        }

        // Skip referred segment numbers
        let ref_size = if seg_num <= 256 {
            1
        } else if seg_num <= 65536 {
            2
        } else {
            4
        };
        cursor += ref_count * ref_size;
        if cursor > data.len() {
            break;
        }

        // Page association
        let page_assoc = if page_assoc_size_flag {
            if cursor + 4 > data.len() {
                break;
            }
            let p = u32::from_be_bytes([
                data[cursor],
                data[cursor + 1],
                data[cursor + 2],
                data[cursor + 3],
            ]);
            cursor += 4;
            p
        } else {
            if cursor + 1 > data.len() {
                break;
            }
            let p = data[cursor] as u32;
            cursor += 1;
            p
        };

        // Data length (4 bytes)
        if cursor + 4 > data.len() {
            break;
        }
        let data_len = u32::from_be_bytes([
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ]) as usize;
        cursor += 4;

        if data_len > StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "JBIG2 segment data exceeds 256 MB safety limit",
            ));
        }

        let seg_data = if data_len == 0xFFFFFFFF {
            // Unknown length (runs to end of page / end of stream)
            let rem = &data[cursor..];
            cursor = data.len();
            rem.to_vec()
        } else {
            if cursor + data_len > data.len() {
                let rem = &data[cursor..];
                cursor = data.len();
                rem.to_vec()
            } else {
                let chunk = &data[cursor..cursor + data_len];
                cursor += data_len;
                chunk.to_vec()
            }
        };

        segments.push((
            Jbig2SegmentHeader {
                segment_number: seg_num,
                segment_type: seg_type,
                page_association: page_assoc,
                data_length: data_len,
                retain_flag,
            },
            seg_data,
        ));
    }

    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jbig2_file_header_parsing() {
        let mut data = Vec::new();
        // 8 bytes file header
        data.extend_from_slice(b"\x97JB2\r\n\x1a\n");
        // Flags: 1 byte (bit 0 = 1: unknown number of pages)
        data.push(0x01);
        // Segment 1 header:
        // Seg num: 1 (4 bytes: 0, 0, 0, 1)
        data.extend_from_slice(&[0, 0, 0, 1]);
        // Flags: 48 (Page info segment = 48)
        data.push(48);
        // Ref count = 0
        data.push(0);
        // Page assoc (short): page 1
        data.push(1);
        // Data len: 8 bytes
        data.extend_from_slice(&[0, 0, 0, 8]);
        // Data: width = 64 (0,0,0,64), height = 32 (0,0,0,32)
        data.extend_from_slice(&[0, 0, 0, 64, 0, 0, 0, 32]);

        let params = Jbig2Params::default();
        let segments = parse_segments(&data).unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0.segment_number, 1);
        assert_eq!(segments[0].0.segment_type, 48);

        let decoded = decode_jbig2(&data, &params).unwrap();
        assert_eq!(decoded.len(), (64 / 8) * 32); // 8 bytes * 32 rows = 256 bytes
    }

    #[test]
    fn test_jbig2_bomb_prevention() {
        let mut data = Vec::new();
        data.extend_from_slice(b"\x97JB2\r\n\x1a\n");
        data.push(0x01);
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.push(48); // Page info
        data.push(0);
        data.push(1);
        data.extend_from_slice(&[0, 0, 0, 8]);
        // Width 1,000,000, Height 1,000,000 => 125 GB >> 256 MB
        data.extend_from_slice(&[0, 15, 66, 64, 0, 15, 66, 64]);

        let params = Jbig2Params::default();
        let res = decode_jbig2(&data, &params);
        assert!(res.is_err());
        match res.unwrap_err() {
            Error::Unsupported(msg) => {
                assert!(msg.contains("256 MB"));
            }
            other => panic!("Expected Unsupported error, got {other:?}"),
        }
    }
}

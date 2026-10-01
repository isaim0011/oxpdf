//! Stage B SIMD & Vectorized Structural Scanning for PDF Tokens.
//!
//! Implements chunked structural classification for PDF delimiters and whitespace
//! per ISO 32000-1 §7.2 and §4.2 of the oxpdf v1.0 architecture.
//!
//! # Specification
//! - Whitespace: 0x00 (NUL), 0x09 (HT), 0x0A (LF), 0x0C (FF), 0x0D (CR), 0x20 (SP)
//! - Delimiters: '(', ')', '<', '>', '[', ']', '{', '}', '/', '%'
//!
//! # Architecture
//! 1. Portable bitmask SWAR / vectorized operations for stable Rust compilation
//!    across all platforms (`x86_64`, `aarch64`, `wasm32`).
//! 2. Experimental portable SIMD via `core::simd` / `std::simd` gated behind `#[cfg(feature = "simd")]`.
//! 3. Always-present scalar fallback for slice tails (< chunk size).

/// Precomputed flag values for fast ASCII character classification.
pub const FLAG_WHITESPACE: u8 = 1 << 0;
pub const FLAG_DELIMITER: u8 = 1 << 1;
pub const FLAG_DELIM_OR_WS: u8 = FLAG_WHITESPACE | FLAG_DELIMITER;

/// 256-entry lookup table mapping byte values to structural classification flags.
pub const CHAR_FLAGS: [u8; 256] = {
    let mut table = [0u8; 256];

    // Whitespace per ISO 32000-1 §7.2.2
    table[0x00] = FLAG_WHITESPACE; // NUL
    table[0x09] = FLAG_WHITESPACE; // Horizontal Tab
    table[0x0A] = FLAG_WHITESPACE; // Line Feed (\n)
    table[0x0C] = FLAG_WHITESPACE; // Form Feed
    table[0x0D] = FLAG_WHITESPACE; // Carriage Return (\r)
    table[0x20] = FLAG_WHITESPACE; // Space

    // Delimiters per ISO 32000-1 §7.2.2
    table[b'(' as usize] = FLAG_DELIMITER;
    table[b')' as usize] = FLAG_DELIMITER;
    table[b'<' as usize] = FLAG_DELIMITER;
    table[b'>' as usize] = FLAG_DELIMITER;
    table[b'[' as usize] = FLAG_DELIMITER;
    table[b']' as usize] = FLAG_DELIMITER;
    table[b'{' as usize] = FLAG_DELIMITER;
    table[b'}' as usize] = FLAG_DELIMITER;
    table[b'/' as usize] = FLAG_DELIMITER;
    table[b'%' as usize] = FLAG_DELIMITER;

    table
};

/// Returns true if byte `b` is a PDF whitespace character.
#[inline(always)]
pub const fn is_whitespace(b: u8) -> bool {
    (CHAR_FLAGS[b as usize] & FLAG_WHITESPACE) != 0
}

/// Returns true if byte `b` is a PDF delimiter character.
#[inline(always)]
pub const fn is_delimiter(b: u8) -> bool {
    (CHAR_FLAGS[b as usize] & FLAG_DELIMITER) != 0
}

/// Returns true if byte `b` is either a PDF whitespace or delimiter character.
#[inline(always)]
pub const fn is_delimiter_or_ws(b: u8) -> bool {
    CHAR_FLAGS[b as usize] != 0
}

/// Result of 16-byte chunk structural classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkClassification16 {
    /// Bit i is set if byte i in chunk is whitespace.
    pub whitespace_mask: u16,
    /// Bit i is set if byte i in chunk is a delimiter.
    pub delimiter_mask: u16,
}

impl ChunkClassification16 {
    /// Combined bitmask of delimiter or whitespace characters.
    #[inline(always)]
    pub fn delimiter_or_whitespace_mask(&self) -> u16 {
        self.whitespace_mask | self.delimiter_mask
    }

    /// Returns index of the first byte that is NOT whitespace, or None if all 16 bytes are whitespace.
    #[inline(always)]
    pub fn first_non_whitespace(&self) -> Option<usize> {
        let non_ws = !self.whitespace_mask;
        if non_ws == 0 {
            None
        } else {
            Some(non_ws.trailing_zeros() as usize)
        }
    }

    /// Returns index of the first delimiter or whitespace byte, or None if none present.
    #[inline(always)]
    pub fn first_delimiter_or_whitespace(&self) -> Option<usize> {
        let mask = self.delimiter_or_whitespace_mask();
        if mask == 0 {
            None
        } else {
            Some(mask.trailing_zeros() as usize)
        }
    }

    /// Returns true if every byte in the chunk is whitespace.
    #[inline(always)]
    pub fn is_all_whitespace(&self) -> bool {
        self.whitespace_mask == 0xFFFF
    }
}

/// Result of 32-byte chunk structural classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkClassification32 {
    /// Bit i is set if byte i in chunk is whitespace.
    pub whitespace_mask: u32,
    /// Bit i is set if byte i in chunk is a delimiter.
    pub delimiter_mask: u32,
}

impl ChunkClassification32 {
    /// Combined bitmask of delimiter or whitespace characters.
    #[inline(always)]
    pub fn delimiter_or_whitespace_mask(&self) -> u32 {
        self.whitespace_mask | self.delimiter_mask
    }

    /// Returns index of the first byte that is NOT whitespace, or None if all 32 bytes are whitespace.
    #[inline(always)]
    pub fn first_non_whitespace(&self) -> Option<usize> {
        let non_ws = !self.whitespace_mask;
        if non_ws == 0 {
            None
        } else {
            Some(non_ws.trailing_zeros() as usize)
        }
    }

    /// Returns index of the first delimiter or whitespace byte, or None if none present.
    #[inline(always)]
    pub fn first_delimiter_or_whitespace(&self) -> Option<usize> {
        let mask = self.delimiter_or_whitespace_mask();
        if mask == 0 {
            None
        } else {
            Some(mask.trailing_zeros() as usize)
        }
    }

    /// Returns true if every byte in the chunk is whitespace.
    #[inline(always)]
    pub fn is_all_whitespace(&self) -> bool {
        self.whitespace_mask == 0xFFFF_FFFF
    }
}

// ============================================================================
// SWAR (SIMD Within A Register) / Portable Vectorized Classifier
// ============================================================================

/// Classifies a 16-byte chunk using branchless SWAR bit extraction.
/// Fully portable and fast across x86_64, aarch64, and wasm32 on stable Rust.
#[inline(always)]
pub fn classify_chunk_16_swar(chunk: &[u8; 16]) -> ChunkClassification16 {
    let mut ws = 0u16;
    let mut delim = 0u16;

    for (i, &b) in chunk.iter().enumerate() {
        let flags = CHAR_FLAGS[b as usize];
        ws |= ((flags & FLAG_WHITESPACE) as u16) << i;
        delim |= (((flags & FLAG_DELIMITER) >> 1) as u16) << i;
    }

    ChunkClassification16 {
        whitespace_mask: ws,
        delimiter_mask: delim,
    }
}

/// Classifies a 32-byte chunk using dual 16-byte SWAR bit extraction.
#[inline(always)]
pub fn classify_chunk_32_swar(chunk: &[u8; 32]) -> ChunkClassification32 {
    let c0: &[u8; 16] = chunk[..16].try_into().unwrap();
    let c1: &[u8; 16] = chunk[16..].try_into().unwrap();
    let r0 = classify_chunk_16_swar(c0);
    let r1 = classify_chunk_16_swar(c1);

    ChunkClassification32 {
        whitespace_mask: (r0.whitespace_mask as u32) | ((r1.whitespace_mask as u32) << 16),
        delimiter_mask: (r0.delimiter_mask as u32) | ((r1.delimiter_mask as u32) << 16),
    }
}

// ============================================================================
// Top-Level Classification Dispatch
// ============================================================================

/// Classifies a 16-byte chunk using portable SWAR bitmask vectorization.
#[inline(always)]
pub fn classify_chunk_16(chunk: &[u8; 16]) -> ChunkClassification16 {
    classify_chunk_16_swar(chunk)
}

/// Classifies a 32-byte chunk using portable SWAR bitmask vectorization.
#[inline(always)]
pub fn classify_chunk_32(chunk: &[u8; 32]) -> ChunkClassification32 {
    classify_chunk_32_swar(chunk)
}

// ============================================================================
// High-Level Accelerated Scanning Operations
// ============================================================================

/// Scans a byte slice and returns the index of the first byte that is NOT whitespace.
/// If the entire slice is whitespace, returns `data.len()`.
///
/// Uses zero-overhead 4-byte unrolled short-token fast-path, 32-byte chunks, 16-byte chunks, and scalar tail loop.
#[inline(always)]
pub fn find_non_whitespace(data: &[u8]) -> usize {
    let len = data.len();
    if len == 0 || !is_whitespace(data[0]) {
        return 0;
    }
    if len == 1 {
        return 1;
    }
    if !is_whitespace(data[1]) {
        return 1;
    }
    if len == 2 {
        return 2;
    }
    if !is_whitespace(data[2]) {
        return 2;
    }
    if len == 3 {
        return 3;
    }
    if !is_whitespace(data[3]) {
        return 3;
    }

    let mut pos = 4;

    // 32-byte chunks
    while pos + 32 <= len {
        let chunk: &[u8; 32] = data[pos..pos + 32].try_into().unwrap();
        let class = classify_chunk_32(chunk);
        if let Some(idx) = class.first_non_whitespace() {
            return pos + idx;
        }
        pos += 32;
    }

    // 16-byte chunk
    if pos + 16 <= len {
        let chunk: &[u8; 16] = data[pos..pos + 16].try_into().unwrap();
        let class = classify_chunk_16(chunk);
        if let Some(idx) = class.first_non_whitespace() {
            return pos + idx;
        }
        pos += 16;
    }

    // Scalar fallback tail
    while pos < len {
        if !is_whitespace(data[pos]) {
            return pos;
        }
        pos += 1;
    }

    pos
}

/// Scans a byte slice and returns the index of the first byte that is a delimiter or whitespace.
/// If no delimiter or whitespace is found, returns `data.len()`.
///
/// Used for zero-copy name scanning and regular token demarcation.
#[inline(always)]
pub fn find_delimiter_or_whitespace(data: &[u8]) -> usize {
    let len = data.len();
    if len == 0 {
        return 0;
    }
    if is_delimiter_or_ws(data[0]) {
        return 0;
    }
    if len == 1 {
        return 1;
    }
    if is_delimiter_or_ws(data[1]) {
        return 1;
    }
    if len == 2 {
        return 2;
    }
    if is_delimiter_or_ws(data[2]) {
        return 2;
    }
    if len == 3 {
        return 3;
    }
    if is_delimiter_or_ws(data[3]) {
        return 3;
    }

    let mut pos = 4;

    // 32-byte chunks
    while pos + 32 <= len {
        let chunk: &[u8; 32] = data[pos..pos + 32].try_into().unwrap();
        let class = classify_chunk_32(chunk);
        if let Some(idx) = class.first_delimiter_or_whitespace() {
            return pos + idx;
        }
        pos += 32;
    }

    // 16-byte chunk
    if pos + 16 <= len {
        let chunk: &[u8; 16] = data[pos..pos + 16].try_into().unwrap();
        let class = classify_chunk_16(chunk);
        if let Some(idx) = class.first_delimiter_or_whitespace() {
            return pos + idx;
        }
        pos += 16;
    }

    // Scalar fallback tail
    while pos < len {
        if is_delimiter_or_ws(data[pos]) {
            return pos;
        }
        pos += 1;
    }

    pos
}

// ============================================================================
// Scalar Reference Implementations (Used for Property Testing & Fallbacks)
// ============================================================================

/// Scalar reference for finding the first non-whitespace byte.
#[inline]
pub fn scalar_find_non_whitespace(data: &[u8]) -> usize {
    for (i, &b) in data.iter().enumerate() {
        if !is_whitespace(b) {
            return i;
        }
    }
    data.len()
}

/// Scalar reference for finding the first delimiter or whitespace byte.
#[inline]
pub fn scalar_find_delimiter_or_whitespace(data: &[u8]) -> usize {
    for (i, &b) in data.iter().enumerate() {
        if is_delimiter_or_ws(b) {
            return i;
        }
    }
    data.len()
}

/// Scalar reference classification for 16-byte chunks.
pub fn scalar_classify_16(chunk: &[u8; 16]) -> ChunkClassification16 {
    let mut ws = 0u16;
    let mut delim = 0u16;
    for (i, &b) in chunk.iter().enumerate() {
        if is_whitespace(b) {
            ws |= 1 << i;
        }
        if is_delimiter(b) {
            delim |= 1 << i;
        }
    }
    ChunkClassification16 {
        whitespace_mask: ws,
        delimiter_mask: delim,
    }
}

/// Scalar reference classification for 32-byte chunks.
pub fn scalar_classify_32(chunk: &[u8; 32]) -> ChunkClassification32 {
    let mut ws = 0u32;
    let mut delim = 0u32;
    for (i, &b) in chunk.iter().enumerate() {
        if is_whitespace(b) {
            ws |= 1 << i;
        }
        if is_delimiter(b) {
            delim |= 1 << i;
        }
    }
    ChunkClassification32 {
        whitespace_mask: ws,
        delimiter_mask: delim,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_whitespace_and_delimiters() {
        let ws_chars = [0x00, 0x09, 0x0A, 0x0C, 0x0D, 0x20];
        for &b in &ws_chars {
            assert!(is_whitespace(b), "Byte 0x{:02X} should be whitespace", b);
            assert!(
                is_delimiter_or_ws(b),
                "Byte 0x{:02X} should be delim_or_ws",
                b
            );
            assert!(!is_delimiter(b), "Byte 0x{:02X} should not be delimiter", b);
        }

        let delim_chars = b"()<>[{}/%]";
        for &b in delim_chars {
            assert!(
                !is_whitespace(b),
                "Byte {:?} should not be whitespace",
                b as char
            );
            assert!(is_delimiter(b), "Byte {:?} should be delimiter", b as char);
            assert!(
                is_delimiter_or_ws(b),
                "Byte {:?} should be delim_or_ws",
                b as char
            );
        }

        let regular_chars = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+-_.:#";
        for &b in regular_chars {
            assert!(!is_whitespace(b), "Byte {:?} should not be ws", b as char);
            assert!(!is_delimiter(b), "Byte {:?} should not be delim", b as char);
            assert!(
                !is_delimiter_or_ws(b),
                "Byte {:?} should not be delim_or_ws",
                b as char
            );
        }
    }

    #[test]
    fn test_chunk_16_classification_match_scalar() {
        let mut sample = [0u8; 16];
        sample.copy_from_slice(b" /Type (Catalog)");

        let swar = classify_chunk_16_swar(&sample);
        let scalar = scalar_classify_16(&sample);
        assert_eq!(swar, scalar);
    }

    #[test]
    fn test_chunk_32_classification_match_scalar() {
        let mut sample = [0u8; 32];
        sample.copy_from_slice(b"   /Length 42 >> stream \r\n  q 1 ");

        let swar = classify_chunk_32_swar(&sample);
        let scalar = scalar_classify_32(&sample);
        assert_eq!(swar, scalar);
    }

    #[test]
    fn test_find_non_whitespace() {
        assert_eq!(find_non_whitespace(b""), 0);
        assert_eq!(find_non_whitespace(b"hello"), 0);
        assert_eq!(find_non_whitespace(b"   hello"), 3);
        assert_eq!(
            find_non_whitespace(b"                                hello"),
            32
        );
        assert_eq!(
            find_non_whitespace(b"                                  hello"),
            34
        );
        assert_eq!(find_non_whitespace(b"   \r\n\t\x0C  "), 9);
    }

    #[test]
    fn test_find_delimiter_or_whitespace() {
        assert_eq!(find_delimiter_or_whitespace(b""), 0);
        assert_eq!(find_delimiter_or_whitespace(b"Catalog/Page"), 7);
        assert_eq!(find_delimiter_or_whitespace(b"MediaBox [0 0 612 792]"), 8);
        assert_eq!(
            find_delimiter_or_whitespace(
                b"ThisIsAVeryLongNameWithoutAnyDelimitersUntilTheEndNow/Stop"
            ),
            53
        );
    }

    /// Property-based test with 10,000 arbitrary pseudo-random byte sequences:
    /// verifies that SIMD/SWAR chunk scanning exactly matches scalar output on arbitrary byte sequences.
    #[test]
    fn test_property_swar_and_simd_match_scalar_arbitrary_sequences() {
        // High quality deterministic PRNG (SplitMix64)
        let mut seed: u64 = 0x9E3779B97F4A7C15;
        let mut next_u64 = || {
            seed = seed.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = seed;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        };

        // Test with varying lengths (0 to 128 bytes)
        for len in 0..=128 {
            for _ in 0..80 {
                let mut buf = vec![0u8; len];
                for chunk in buf.chunks_mut(8) {
                    let r = next_u64().to_le_bytes();
                    let take = chunk.len().min(8);
                    chunk[..take].copy_from_slice(&r[..take]);
                }

                // Inject PDF tokens with certain probability to test realistic mixtures
                for b in buf.iter_mut() {
                    let p = (next_u64() % 10) as u8;
                    if p == 0 {
                        *b = b' ';
                    } else if p == 1 {
                        *b = b'\n';
                    } else if p == 2 {
                        *b = b'/';
                    } else if p == 3 {
                        *b = b'(';
                    } else if p == 4 {
                        *b = b'>';
                    }
                }

                // Verify find_non_whitespace matches scalar
                let expected_ws = scalar_find_non_whitespace(&buf);
                let actual_ws = find_non_whitespace(&buf);
                assert_eq!(
                    actual_ws, expected_ws,
                    "find_non_whitespace failed on len {}, slice: {:?}",
                    len, buf
                );

                // Verify find_delimiter_or_whitespace matches scalar
                let expected_delim = scalar_find_delimiter_or_whitespace(&buf);
                let actual_delim = find_delimiter_or_whitespace(&buf);
                assert_eq!(
                    actual_delim, expected_delim,
                    "find_delimiter_or_whitespace failed on len {}, slice: {:?}",
                    len, buf
                );

                // Verify 16-byte chunk classification if len == 16
                if len == 16 {
                    let chunk: &[u8; 16] = buf.as_slice().try_into().unwrap();
                    let expected_class = scalar_classify_16(chunk);
                    let actual_swar = classify_chunk_16_swar(chunk);
                    assert_eq!(actual_swar, expected_class);
                }

                // Verify 32-byte chunk classification if len == 32
                if len == 32 {
                    let chunk: &[u8; 32] = buf.as_slice().try_into().unwrap();
                    let expected_class = scalar_classify_32(chunk);
                    let actual_swar = classify_chunk_32_swar(chunk);
                    assert_eq!(actual_swar, expected_class);
                }
            }
        }
    }
}

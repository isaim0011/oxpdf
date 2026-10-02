//! CCITT Fax (Group 3 1D, Group 3 2D, and Group 4 2D ITU-T T.4 / T.6) decompressor.
//!
//! Implements run-length decoding of bi-level facsimile images according to
//! ITU-T Recommendation T.4 and T.6, as used in the PDF `/CCITTFaxDecode` filter.

use crate::error::{Error, Result};
use crate::stream::StreamView;

/// Parameters for CCITTFaxDecode filter per ISO 32000-1 Table 11.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CcittParams {
    /// K parameter:
    /// - `K < 0`: Pure two-dimensional encoding (Group 4, T.6).
    /// - `K = 0`: Pure one-dimensional encoding (Group 3 1D, T.4).
    /// - `K > 0`: Mixed one- and two-dimensional encoding (Group 3 2D, T.4).
    ///
    /// Default: 0.
    pub k: i32,
    /// If true, end-of-line bit patterns must be present.
    /// Default: false.
    pub end_of_line: bool,
    /// If true, 1-bits are interpreted as black pixels, 0-bits as white pixels.
    /// If false (default), 0-bits are white and 1-bits are black, or per PDF spec:
    /// BlackIs1 default is false (0 is white, 1 is black).
    pub black_is1: bool,
    /// Width of the bi-level image in pixels.
    /// Default: 1728.
    pub columns: u32,
    /// Height of the bi-level image in scan lines. If 0 or not specified,
    /// lines are decoded until end-of-block or end of stream.
    /// Default: 0.
    pub rows: u32,
    /// If true, the end-of-block pattern is expected.
    /// Default: true.
    pub end_of_block: bool,
    /// If true, each scan line begins on a byte boundary.
    /// Default: false.
    pub encoded_byte_align: bool,
}

impl Default for CcittParams {
    fn default() -> Self {
        Self {
            k: 0,
            end_of_line: false,
            black_is1: false,
            columns: 1728,
            rows: 0,
            end_of_block: true,
            encoded_byte_align: false,
        }
    }
}

/// A bitstream reader for MSB-first bit order (standard in PDF CCITT).
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8, // 0..8, where 0 is MSB (0x80)
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    #[inline]
    fn is_eof(&self) -> bool {
        self.byte_pos >= self.data.len()
    }

    #[inline]
    fn read_bit(&mut self) -> Option<u8> {
        if self.byte_pos >= self.data.len() {
            return None;
        }
        let byte = self.data[self.byte_pos];
        let bit = (byte >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit)
    }

    /// Peeks up to 16 bits without advancing the reader position.
    #[inline]
    fn peek_bits(&self, count: usize) -> Option<u16> {
        if count == 0 || count > 16 {
            return None;
        }
        let mut cur_byte = self.byte_pos;
        let mut cur_bit = self.bit_pos;
        let mut val = 0u16;

        for _ in 0..count {
            if cur_byte >= self.data.len() {
                return None;
            }
            let bit = (self.data[cur_byte] >> (7 - cur_bit)) & 1;
            val = (val << 1) | (bit as u16);
            cur_bit += 1;
            if cur_bit == 8 {
                cur_bit = 0;
                cur_byte += 1;
            }
        }
        Some(val)
    }

    #[inline]
    fn consume_bits(&mut self, count: usize) {
        let total_bit = self.bit_pos as usize + count;
        self.byte_pos += total_bit / 8;
        self.bit_pos = (total_bit % 8) as u8;
    }

    #[inline]
    fn align_to_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }
}

// -----------------------------------------------------------------------------
// Huffman Codes for ITU-T T.4 / T.6 Modified Huffman
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct HuffmanCode {
    code: u16,
    len: u8,
    run: u16,
}

// Terminating codes for White runs (0..63)
static WHITE_TERMINATING: [HuffmanCode; 64] = [
    HuffmanCode {
        code: 0b00110101,
        len: 8,
        run: 0,
    },
    HuffmanCode {
        code: 0b000111,
        len: 6,
        run: 1,
    },
    HuffmanCode {
        code: 0b0111,
        len: 4,
        run: 2,
    },
    HuffmanCode {
        code: 0b1000,
        len: 4,
        run: 3,
    },
    HuffmanCode {
        code: 0b1011,
        len: 4,
        run: 4,
    },
    HuffmanCode {
        code: 0b1100,
        len: 4,
        run: 5,
    },
    HuffmanCode {
        code: 0b1110,
        len: 4,
        run: 6,
    },
    HuffmanCode {
        code: 0b1111,
        len: 4,
        run: 7,
    },
    HuffmanCode {
        code: 0b10011,
        len: 5,
        run: 8,
    },
    HuffmanCode {
        code: 0b10100,
        len: 5,
        run: 9,
    },
    HuffmanCode {
        code: 0b00111,
        len: 5,
        run: 10,
    },
    HuffmanCode {
        code: 0b01000,
        len: 5,
        run: 11,
    },
    HuffmanCode {
        code: 0b001000,
        len: 6,
        run: 12,
    },
    HuffmanCode {
        code: 0b000011,
        len: 6,
        run: 13,
    },
    HuffmanCode {
        code: 0b110100,
        len: 6,
        run: 14,
    },
    HuffmanCode {
        code: 0b110101,
        len: 6,
        run: 15,
    },
    HuffmanCode {
        code: 0b101010,
        len: 6,
        run: 16,
    },
    HuffmanCode {
        code: 0b101011,
        len: 6,
        run: 17,
    },
    HuffmanCode {
        code: 0b0100111,
        len: 7,
        run: 18,
    },
    HuffmanCode {
        code: 0b0001100,
        len: 7,
        run: 19,
    },
    HuffmanCode {
        code: 0b0001000,
        len: 7,
        run: 20,
    },
    HuffmanCode {
        code: 0b0010111,
        len: 7,
        run: 21,
    },
    HuffmanCode {
        code: 0b0000011,
        len: 7,
        run: 22,
    },
    HuffmanCode {
        code: 0b0000100,
        len: 7,
        run: 23,
    },
    HuffmanCode {
        code: 0b0101000,
        len: 7,
        run: 24,
    },
    HuffmanCode {
        code: 0b0101011,
        len: 7,
        run: 25,
    },
    HuffmanCode {
        code: 0b0010011,
        len: 7,
        run: 26,
    },
    HuffmanCode {
        code: 0b0100100,
        len: 7,
        run: 27,
    },
    HuffmanCode {
        code: 0b0011000,
        len: 7,
        run: 28,
    },
    HuffmanCode {
        code: 0b00000010,
        len: 8,
        run: 29,
    },
    HuffmanCode {
        code: 0b00000011,
        len: 8,
        run: 30,
    },
    HuffmanCode {
        code: 0b00011010,
        len: 8,
        run: 31,
    },
    HuffmanCode {
        code: 0b00011011,
        len: 8,
        run: 32,
    },
    HuffmanCode {
        code: 0b00010010,
        len: 8,
        run: 33,
    },
    HuffmanCode {
        code: 0b00010011,
        len: 8,
        run: 34,
    },
    HuffmanCode {
        code: 0b00010100,
        len: 8,
        run: 35,
    },
    HuffmanCode {
        code: 0b00010101,
        len: 8,
        run: 36,
    },
    HuffmanCode {
        code: 0b00010110,
        len: 8,
        run: 37,
    },
    HuffmanCode {
        code: 0b00010111,
        len: 8,
        run: 38,
    },
    HuffmanCode {
        code: 0b00101000,
        len: 8,
        run: 39,
    },
    HuffmanCode {
        code: 0b00101001,
        len: 8,
        run: 40,
    },
    HuffmanCode {
        code: 0b00101010,
        len: 8,
        run: 41,
    },
    HuffmanCode {
        code: 0b00101011,
        len: 8,
        run: 42,
    },
    HuffmanCode {
        code: 0b01010100,
        len: 8,
        run: 43,
    },
    HuffmanCode {
        code: 0b01010101,
        len: 8,
        run: 44,
    },
    HuffmanCode {
        code: 0b01010110,
        len: 8,
        run: 45,
    },
    HuffmanCode {
        code: 0b01010111,
        len: 8,
        run: 46,
    },
    HuffmanCode {
        code: 0b01101000,
        len: 8,
        run: 47,
    },
    HuffmanCode {
        code: 0b01101001,
        len: 8,
        run: 48,
    },
    HuffmanCode {
        code: 0b01001010,
        len: 8,
        run: 49,
    },
    HuffmanCode {
        code: 0b01001011,
        len: 8,
        run: 50,
    },
    HuffmanCode {
        code: 0b00110010,
        len: 8,
        run: 51,
    },
    HuffmanCode {
        code: 0b00110011,
        len: 8,
        run: 52,
    },
    HuffmanCode {
        code: 0b00110100,
        len: 8,
        run: 53,
    },
    HuffmanCode {
        code: 0b00110110,
        len: 8,
        run: 54,
    },
    HuffmanCode {
        code: 0b00110111,
        len: 8,
        run: 55,
    },
    HuffmanCode {
        code: 0b01101010,
        len: 8,
        run: 56,
    },
    HuffmanCode {
        code: 0b01101011,
        len: 8,
        run: 57,
    },
    HuffmanCode {
        code: 0b01001100,
        len: 8,
        run: 58,
    },
    HuffmanCode {
        code: 0b01001101,
        len: 8,
        run: 59,
    },
    HuffmanCode {
        code: 0b001100010,
        len: 9,
        run: 60,
    },
    HuffmanCode {
        code: 0b001100011,
        len: 9,
        run: 61,
    },
    HuffmanCode {
        code: 0b001100100,
        len: 9,
        run: 62,
    },
    HuffmanCode {
        code: 0b001100101,
        len: 9,
        run: 63,
    },
];

// Make-up codes for White runs (64..1728 in steps of 64)
static WHITE_MAKEUP: [HuffmanCode; 27] = [
    HuffmanCode {
        code: 0b11011,
        len: 5,
        run: 64,
    },
    HuffmanCode {
        code: 0b10010,
        len: 5,
        run: 128,
    },
    HuffmanCode {
        code: 0b010111,
        len: 6,
        run: 192,
    },
    HuffmanCode {
        code: 0b0110111,
        len: 7,
        run: 256,
    },
    HuffmanCode {
        code: 0b00110110,
        len: 8,
        run: 320,
    },
    HuffmanCode {
        code: 0b00110111,
        len: 8,
        run: 384,
    },
    HuffmanCode {
        code: 0b01100100,
        len: 8,
        run: 448,
    },
    HuffmanCode {
        code: 0b01100101,
        len: 8,
        run: 512,
    },
    HuffmanCode {
        code: 0b01101000,
        len: 8,
        run: 576,
    },
    HuffmanCode {
        code: 0b01100111,
        len: 8,
        run: 640,
    },
    HuffmanCode {
        code: 0b011001100,
        len: 9,
        run: 704,
    },
    HuffmanCode {
        code: 0b011001101,
        len: 9,
        run: 768,
    },
    HuffmanCode {
        code: 0b011010010,
        len: 9,
        run: 832,
    },
    HuffmanCode {
        code: 0b011010011,
        len: 9,
        run: 896,
    },
    HuffmanCode {
        code: 0b011010100,
        len: 9,
        run: 960,
    },
    HuffmanCode {
        code: 0b011010101,
        len: 9,
        run: 1024,
    },
    HuffmanCode {
        code: 0b011010110,
        len: 9,
        run: 1088,
    },
    HuffmanCode {
        code: 0b011010111,
        len: 9,
        run: 1152,
    },
    HuffmanCode {
        code: 0b011001000,
        len: 9,
        run: 1216,
    },
    HuffmanCode {
        code: 0b011001001,
        len: 9,
        run: 1280,
    },
    HuffmanCode {
        code: 0b011001010,
        len: 9,
        run: 1344,
    },
    HuffmanCode {
        code: 0b011001011,
        len: 9,
        run: 1408,
    },
    HuffmanCode {
        code: 0b010011000,
        len: 9,
        run: 1472,
    },
    HuffmanCode {
        code: 0b010011001,
        len: 9,
        run: 1536,
    },
    HuffmanCode {
        code: 0b010011010,
        len: 9,
        run: 1600,
    },
    HuffmanCode {
        code: 0b011000,
        len: 6,
        run: 1664,
    },
    HuffmanCode {
        code: 0b010011011,
        len: 9,
        run: 1728,
    },
];

// Terminating codes for Black runs (0..63)
static BLACK_TERMINATING: [HuffmanCode; 64] = [
    HuffmanCode {
        code: 0b0000110111,
        len: 10,
        run: 0,
    },
    HuffmanCode {
        code: 0b010,
        len: 3,
        run: 1,
    },
    HuffmanCode {
        code: 0b11,
        len: 2,
        run: 2,
    },
    HuffmanCode {
        code: 0b10,
        len: 2,
        run: 3,
    },
    HuffmanCode {
        code: 0b011,
        len: 3,
        run: 4,
    },
    HuffmanCode {
        code: 0b0011,
        len: 4,
        run: 5,
    },
    HuffmanCode {
        code: 0b0010,
        len: 4,
        run: 6,
    },
    HuffmanCode {
        code: 0b00011,
        len: 5,
        run: 7,
    },
    HuffmanCode {
        code: 0b000101,
        len: 6,
        run: 8,
    },
    HuffmanCode {
        code: 0b000100,
        len: 6,
        run: 9,
    },
    HuffmanCode {
        code: 0b0000100,
        len: 7,
        run: 10,
    },
    HuffmanCode {
        code: 0b0000101,
        len: 7,
        run: 11,
    },
    HuffmanCode {
        code: 0b0000111,
        len: 7,
        run: 12,
    },
    HuffmanCode {
        code: 0b00000100,
        len: 8,
        run: 13,
    },
    HuffmanCode {
        code: 0b00000111,
        len: 8,
        run: 14,
    },
    HuffmanCode {
        code: 0b000011000,
        len: 9,
        run: 15,
    },
    HuffmanCode {
        code: 0b0000010111,
        len: 10,
        run: 16,
    },
    HuffmanCode {
        code: 0b0000011000,
        len: 10,
        run: 17,
    },
    HuffmanCode {
        code: 0b0000001000,
        len: 10,
        run: 18,
    },
    HuffmanCode {
        code: 0b00001100111,
        len: 11,
        run: 19,
    },
    HuffmanCode {
        code: 0b00001101000,
        len: 11,
        run: 20,
    },
    HuffmanCode {
        code: 0b00001101100,
        len: 11,
        run: 21,
    },
    HuffmanCode {
        code: 0b00000110111,
        len: 11,
        run: 22,
    },
    HuffmanCode {
        code: 0b00000101000,
        len: 11,
        run: 23,
    },
    HuffmanCode {
        code: 0b00000010111,
        len: 11,
        run: 24,
    },
    HuffmanCode {
        code: 0b00000011000,
        len: 11,
        run: 25,
    },
    HuffmanCode {
        code: 0b000011001010,
        len: 12,
        run: 26,
    },
    HuffmanCode {
        code: 0b000011001011,
        len: 12,
        run: 27,
    },
    HuffmanCode {
        code: 0b000011001100,
        len: 12,
        run: 28,
    },
    HuffmanCode {
        code: 0b000011001101,
        len: 12,
        run: 29,
    },
    HuffmanCode {
        code: 0b000001101000,
        len: 12,
        run: 30,
    },
    HuffmanCode {
        code: 0b000001101001,
        len: 12,
        run: 31,
    },
    HuffmanCode {
        code: 0b000001101010,
        len: 12,
        run: 32,
    },
    HuffmanCode {
        code: 0b000001101011,
        len: 12,
        run: 33,
    },
    HuffmanCode {
        code: 0b000011010010,
        len: 12,
        run: 34,
    },
    HuffmanCode {
        code: 0b000011010011,
        len: 12,
        run: 35,
    },
    HuffmanCode {
        code: 0b000011010100,
        len: 12,
        run: 36,
    },
    HuffmanCode {
        code: 0b000011010101,
        len: 12,
        run: 37,
    },
    HuffmanCode {
        code: 0b000011010110,
        len: 12,
        run: 38,
    },
    HuffmanCode {
        code: 0b000011010111,
        len: 12,
        run: 39,
    },
    HuffmanCode {
        code: 0b000001101100,
        len: 12,
        run: 40,
    },
    HuffmanCode {
        code: 0b000001101101,
        len: 12,
        run: 41,
    },
    HuffmanCode {
        code: 0b000011011010,
        len: 12,
        run: 42,
    },
    HuffmanCode {
        code: 0b000011011011,
        len: 12,
        run: 43,
    },
    HuffmanCode {
        code: 0b000001010100,
        len: 12,
        run: 44,
    },
    HuffmanCode {
        code: 0b000001010101,
        len: 12,
        run: 45,
    },
    HuffmanCode {
        code: 0b000001010110,
        len: 12,
        run: 46,
    },
    HuffmanCode {
        code: 0b000001010111,
        len: 12,
        run: 47,
    },
    HuffmanCode {
        code: 0b000001100100,
        len: 12,
        run: 48,
    },
    HuffmanCode {
        code: 0b000001100101,
        len: 12,
        run: 49,
    },
    HuffmanCode {
        code: 0b000001010010,
        len: 12,
        run: 50,
    },
    HuffmanCode {
        code: 0b000001010011,
        len: 12,
        run: 51,
    },
    HuffmanCode {
        code: 0b000000100100,
        len: 12,
        run: 52,
    },
    HuffmanCode {
        code: 0b000000110111,
        len: 12,
        run: 53,
    },
    HuffmanCode {
        code: 0b000000111000,
        len: 12,
        run: 54,
    },
    HuffmanCode {
        code: 0b000000100111,
        len: 12,
        run: 55,
    },
    HuffmanCode {
        code: 0b000000101000,
        len: 12,
        run: 56,
    },
    HuffmanCode {
        code: 0b000001011000,
        len: 12,
        run: 57,
    },
    HuffmanCode {
        code: 0b000001011001,
        len: 12,
        run: 58,
    },
    HuffmanCode {
        code: 0b000000101011,
        len: 12,
        run: 59,
    },
    HuffmanCode {
        code: 0b000000101100,
        len: 12,
        run: 60,
    },
    HuffmanCode {
        code: 0b000001011010,
        len: 12,
        run: 61,
    },
    HuffmanCode {
        code: 0b000001100110,
        len: 12,
        run: 62,
    },
    HuffmanCode {
        code: 0b000001100111,
        len: 12,
        run: 63,
    },
];

// Make-up codes for Black runs (64..1728 in steps of 64)
static BLACK_MAKEUP: [HuffmanCode; 27] = [
    HuffmanCode {
        code: 0b0000001111,
        len: 10,
        run: 64,
    },
    HuffmanCode {
        code: 0b000011001000,
        len: 12,
        run: 128,
    },
    HuffmanCode {
        code: 0b000011001001,
        len: 12,
        run: 192,
    },
    HuffmanCode {
        code: 0b000001011011,
        len: 12,
        run: 256,
    },
    HuffmanCode {
        code: 0b000000110011,
        len: 12,
        run: 320,
    },
    HuffmanCode {
        code: 0b000000110100,
        len: 12,
        run: 384,
    },
    HuffmanCode {
        code: 0b000000110101,
        len: 12,
        run: 448,
    },
    HuffmanCode {
        code: 0b0000001101100,
        len: 13,
        run: 512,
    },
    HuffmanCode {
        code: 0b0000001101101,
        len: 13,
        run: 576,
    },
    HuffmanCode {
        code: 0b0000001001010,
        len: 13,
        run: 640,
    },
    HuffmanCode {
        code: 0b0000001001011,
        len: 13,
        run: 704,
    },
    HuffmanCode {
        code: 0b0000001001100,
        len: 13,
        run: 768,
    },
    HuffmanCode {
        code: 0b0000001001101,
        len: 13,
        run: 832,
    },
    HuffmanCode {
        code: 0b0000001110010,
        len: 13,
        run: 896,
    },
    HuffmanCode {
        code: 0b0000001110011,
        len: 13,
        run: 960,
    },
    HuffmanCode {
        code: 0b0000001110100,
        len: 13,
        run: 1024,
    },
    HuffmanCode {
        code: 0b0000001110101,
        len: 13,
        run: 1088,
    },
    HuffmanCode {
        code: 0b0000001110110,
        len: 13,
        run: 1152,
    },
    HuffmanCode {
        code: 0b0000001110111,
        len: 13,
        run: 1216,
    },
    HuffmanCode {
        code: 0b0000001010010,
        len: 13,
        run: 1280,
    },
    HuffmanCode {
        code: 0b0000001010011,
        len: 13,
        run: 1344,
    },
    HuffmanCode {
        code: 0b0000001010100,
        len: 13,
        run: 1408,
    },
    HuffmanCode {
        code: 0b0000001010101,
        len: 13,
        run: 1472,
    },
    HuffmanCode {
        code: 0b0000001011010,
        len: 13,
        run: 1536,
    },
    HuffmanCode {
        code: 0b0000001011011,
        len: 13,
        run: 1600,
    },
    HuffmanCode {
        code: 0b0000011000,
        len: 10,
        run: 1664,
    },
    HuffmanCode {
        code: 0b0000001100101,
        len: 13,
        run: 1728,
    },
];

// Additional shared make-up codes (1792..2560 in steps of 64)
static SHARED_MAKEUP: [HuffmanCode; 13] = [
    HuffmanCode {
        code: 0b00000001000,
        len: 11,
        run: 1792,
    },
    HuffmanCode {
        code: 0b00000001100,
        len: 11,
        run: 1856,
    },
    HuffmanCode {
        code: 0b00000001101,
        len: 11,
        run: 1920,
    },
    HuffmanCode {
        code: 0b000000010010,
        len: 12,
        run: 1984,
    },
    HuffmanCode {
        code: 0b000000010011,
        len: 12,
        run: 2048,
    },
    HuffmanCode {
        code: 0b000000010100,
        len: 12,
        run: 2112,
    },
    HuffmanCode {
        code: 0b000000010101,
        len: 12,
        run: 2176,
    },
    HuffmanCode {
        code: 0b000000010110,
        len: 12,
        run: 2240,
    },
    HuffmanCode {
        code: 0b000000010111,
        len: 12,
        run: 2304,
    },
    HuffmanCode {
        code: 0b000000011100,
        len: 12,
        run: 2368,
    },
    HuffmanCode {
        code: 0b000000011101,
        len: 12,
        run: 2432,
    },
    HuffmanCode {
        code: 0b000000011110,
        len: 12,
        run: 2496,
    },
    HuffmanCode {
        code: 0b000000011111,
        len: 12,
        run: 2560,
    },
];

/// Decodes one complete white run-length (including make-up codes if any).
fn decode_white_run(reader: &mut BitReader<'_>) -> Result<u32> {
    let mut total_run = 0u32;
    loop {
        let mut matched = false;

        // Try terminating codes (len 4..9)
        for entry in &WHITE_TERMINATING {
            if let Some(peek) = reader.peek_bits(entry.len as usize) {
                if peek == entry.code {
                    reader.consume_bits(entry.len as usize);
                    total_run = total_run.saturating_add(entry.run as u32);
                    return Ok(total_run);
                }
            }
        }

        // Try white make-up codes (len 5..9)
        for entry in &WHITE_MAKEUP {
            if let Some(peek) = reader.peek_bits(entry.len as usize) {
                if peek == entry.code {
                    reader.consume_bits(entry.len as usize);
                    total_run = total_run.saturating_add(entry.run as u32);
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        // Try shared makeups (len 11..12)
        for entry in &SHARED_MAKEUP {
            if let Some(peek) = reader.peek_bits(entry.len as usize) {
                if peek == entry.code {
                    reader.consume_bits(entry.len as usize);
                    total_run = total_run.saturating_add(entry.run as u32);
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        return Err(Error::Unsupported("invalid CCITT white run code"));
    }
}

/// Decodes one complete black run-length (including make-up codes if any).
fn decode_black_run(reader: &mut BitReader<'_>) -> Result<u32> {
    let mut total_run = 0u32;
    loop {
        let mut matched = false;

        // Try terminating codes (len 2..12)
        for entry in &BLACK_TERMINATING {
            if let Some(peek) = reader.peek_bits(entry.len as usize) {
                if peek == entry.code {
                    reader.consume_bits(entry.len as usize);
                    total_run = total_run.saturating_add(entry.run as u32);
                    return Ok(total_run);
                }
            }
        }

        // Try black make-up codes (len 10..13)
        for entry in &BLACK_MAKEUP {
            if let Some(peek) = reader.peek_bits(entry.len as usize) {
                if peek == entry.code {
                    reader.consume_bits(entry.len as usize);
                    total_run = total_run.saturating_add(entry.run as u32);
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        // Try shared makeups (len 11..12)
        for entry in &SHARED_MAKEUP {
            if let Some(peek) = reader.peek_bits(entry.len as usize) {
                if peek == entry.code {
                    reader.consume_bits(entry.len as usize);
                    total_run = total_run.saturating_add(entry.run as u32);
                    matched = true;
                    break;
                }
            }
        }
        if matched {
            continue;
        }

        return Err(Error::Unsupported("invalid CCITT black run code"));
    }
}

// -----------------------------------------------------------------------------
// 2D Decoding Modes (ITU-T T.4 2D / T.6 Group 4)
// -----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Mode2D {
    Pass,
    Horizontal,
    Vertical0,
    VerticalR1,
    VerticalR2,
    VerticalR3,
    VerticalL1,
    VerticalL2,
    VerticalL3,
    Extension,
    Eofb,
}

/// Decodes next 2D mode from the bitstream.
fn decode_mode_2d(reader: &mut BitReader<'_>) -> Result<Mode2D> {
    // Check for EOFB: 000000000001 000000000001 (two consecutive EOLs = 24 bits, or 12 bits 000000000001)
    if let Some(peek) = reader.peek_bits(12) {
        if peek == 0b000000000001 {
            // EOL or EOFB
            reader.consume_bits(12);
            // Check if there is another EOL following immediately
            if let Some(peek2) = reader.peek_bits(12) {
                if peek2 == 0b000000000001 {
                    reader.consume_bits(12);
                }
            }
            return Ok(Mode2D::Eofb);
        }
    }

    // Pass mode: 0001 (4 bits)
    if let Some(peek) = reader.peek_bits(4) {
        if peek == 0b0001 {
            reader.consume_bits(4);
            return Ok(Mode2D::Pass);
        }
    }

    // Horizontal mode: 001 (3 bits)
    if let Some(peek) = reader.peek_bits(3) {
        if peek == 0b001 {
            reader.consume_bits(3);
            return Ok(Mode2D::Horizontal);
        }
    }

    // Vertical modes
    // V(0): 1 (1 bit)
    if let Some(peek) = reader.peek_bits(1) {
        if peek == 1 {
            reader.consume_bits(1);
            return Ok(Mode2D::Vertical0);
        }
    }

    // VR(1): 011 (3 bits)
    // VL(1): 010 (3 bits)
    if let Some(peek) = reader.peek_bits(3) {
        if peek == 0b011 {
            reader.consume_bits(3);
            return Ok(Mode2D::VerticalR1);
        }
        if peek == 0b010 {
            reader.consume_bits(3);
            return Ok(Mode2D::VerticalL1);
        }
    }

    // VR(2): 000011 (6 bits)
    // VL(2): 000010 (6 bits)
    if let Some(peek) = reader.peek_bits(6) {
        if peek == 0b000011 {
            reader.consume_bits(6);
            return Ok(Mode2D::VerticalR2);
        }
        if peek == 0b000010 {
            reader.consume_bits(6);
            return Ok(Mode2D::VerticalL2);
        }
    }

    // VR(3): 0000011 (7 bits)
    // VL(3): 0000010 (7 bits)
    if let Some(peek) = reader.peek_bits(7) {
        if peek == 0b0000011 {
            reader.consume_bits(7);
            return Ok(Mode2D::VerticalR3);
        }
        if peek == 0b0000010 {
            reader.consume_bits(7);
            return Ok(Mode2D::VerticalL3);
        }
    }

    // Extension: 0000001xxx (7 bits prefix 0000001)
    if let Some(peek) = reader.peek_bits(7) {
        if peek == 0b0000001 {
            reader.consume_bits(7);
            // Read 3 extension bits
            let _ext = reader.peek_bits(3).unwrap_or(0);
            reader.consume_bits(3);
            return Ok(Mode2D::Extension);
        }
    }

    Err(Error::Unsupported("invalid CCITT 2D mode code"))
}

/// Finds the next changing element on the given line after `start_idx`.
/// Returns the index in pixels, or `columns` if none.
fn find_next_changing_element(line: &[u8], start_idx: usize, columns: usize) -> usize {
    if start_idx >= columns {
        return columns;
    }
    let current_val = line[start_idx];
    for (i, &val) in line
        .iter()
        .enumerate()
        .skip(start_idx + 1)
        .take(columns - (start_idx + 1))
    {
        if val != current_val {
            return i;
        }
    }
    columns
}

/// Finds the next changing element of a specific color after `start_idx`.
fn find_next_changing_color(
    line: &[u8],
    start_idx: usize,
    columns: usize,
    target_color: u8,
) -> usize {
    for (i, &val) in line
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(columns - start_idx)
    {
        if val == target_color {
            return i;
        }
    }
    columns
}

// -----------------------------------------------------------------------------
// Line decoding & Group 4 / Group 3 drivers
// -----------------------------------------------------------------------------

/// Decodes one 1D line (Modified Huffman).
fn decode_line_1d(
    reader: &mut BitReader<'_>,
    current_line: &mut [u8],
    columns: usize,
) -> Result<()> {
    let mut a0 = 0usize;
    let mut is_white = true;

    while a0 < columns {
        let run = if is_white {
            decode_white_run(reader)? as usize
        } else {
            decode_black_run(reader)? as usize
        };

        let end = (a0 + run).min(columns);
        let color = if is_white { 0u8 } else { 1u8 };
        current_line[a0..end].fill(color);
        a0 = end;
        is_white = !is_white;
    }
    Ok(())
}

/// Decodes one 2D line given the reference line above it.
fn decode_line_2d(
    reader: &mut BitReader<'_>,
    ref_line: &[u8],
    cur_line: &mut [u8],
    columns: usize,
) -> Result<bool> {
    // a0 starts at -1 imaginary element; represented here as:
    // a0_pos: position in line, initial 0
    // a0_color: color of a0 (initially 0 = white)
    let mut a0 = 0usize;
    let mut a0_color = 0u8; // white
    let mut is_first = true;

    while a0 < columns {
        // b1: first changing element on reference line to the right of a0 and of OPPOSITE color to a0
        // Exception: when a0 is at beginning (imaginary white pixel before line), b1 is first black pixel.
        let b1 = if is_first {
            find_next_changing_color(ref_line, 0, columns, 1)
        } else {
            let next_change = find_next_changing_element(ref_line, a0, columns);
            if next_change < columns && ref_line[next_change] != a0_color {
                next_change
            } else {
                find_next_changing_element(ref_line, next_change, columns)
            }
        };

        // b2: next changing element to the right of b1 on reference line
        let b2 = find_next_changing_element(ref_line, b1, columns);

        let mode = decode_mode_2d(reader)?;
        match mode {
            Mode2D::Pass => {
                // Pass mode: a0 moves to b2
                let next_a0 = b2.min(columns);
                cur_line[a0..next_a0].fill(a0_color);
                a0 = next_a0;
                is_first = false;
            }
            Mode2D::Horizontal => {
                // Horizontal mode: a0a1 run, then a1a2 run
                // a0a1 has color a0_color
                let run1 = if a0_color == 0 {
                    decode_white_run(reader)? as usize
                } else {
                    decode_black_run(reader)? as usize
                };
                let a1 = (a0 + run1).min(columns);
                cur_line[a0..a1].fill(a0_color);

                // a1a2 has opposite color
                let a1_color = 1 - a0_color;
                let run2 = if a1_color == 0 {
                    decode_white_run(reader)? as usize
                } else {
                    decode_black_run(reader)? as usize
                };
                let a2 = (a1 + run2).min(columns);
                cur_line[a1..a2].fill(a1_color);

                a0 = a2;
                a0_color = a1_color;
                is_first = false;
            }
            Mode2D::Vertical0 => {
                let a1 = b1.min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::VerticalR1 => {
                let a1 = (b1 + 1).min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::VerticalR2 => {
                let a1 = (b1 + 2).min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::VerticalR3 => {
                let a1 = (b1 + 3).min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::VerticalL1 => {
                let a1 = if b1 > 0 { b1 - 1 } else { 0 };
                let a1 = a1.min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::VerticalL2 => {
                let a1 = if b1 > 1 { b1 - 2 } else { 0 };
                let a1 = a1.min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::VerticalL3 => {
                let a1 = if b1 > 2 { b1 - 3 } else { 0 };
                let a1 = a1.min(columns);
                cur_line[a0..a1].fill(a0_color);
                a0 = a1;
                a0_color = 1 - a0_color;
                is_first = false;
            }
            Mode2D::Extension => {
                // Skip extension, continue
            }
            Mode2D::Eofb => {
                // End of facsimile block reached
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Packs an array of pixel values (0 or 1) into packed bytes: 8 pixels per byte, MSB first.
fn pack_pixels(line: &[u8], black_is1: bool) -> Vec<u8> {
    let row_bytes = line.len().div_ceil(8);
    let mut out = vec![0u8; row_bytes];

    for (i, &pixel) in line.iter().enumerate() {
        // By default: pixel == 1 is black, pixel == 0 is white.
        // If black_is1 is true: black is 1, white is 0.
        // If black_is1 is false: PDF spec standard: 0 is white, 1 is black (or inverted if BlackIs1 is 0).
        // In PDF CCITTFaxDecode:
        // /BlackIs1 boolean:
        // "A flag indicating whether 1-bits are to be interpreted as black pixels and 0-bits as white pixels.
        // Default value: false."
        // That means by default: 0-bit = black, 1-bit = white!
        // When BlackIs1 is true: 1-bit = black, 0-bit = white.
        let bit_val = if black_is1 {
            pixel // 1 for black, 0 for white
        } else {
            1 - pixel // 0 for black, 1 for white
        };

        if bit_val == 1 {
            let byte_idx = i / 8;
            let bit_idx = 7 - (i % 8);
            out[byte_idx] |= 1 << bit_idx;
        }
    }
    out
}

/// Decodes CCITTFax encoded bytes with safety boundaries.
pub fn decode_ccitt_fax(input: &[u8], params: &CcittParams) -> Result<Vec<u8>> {
    let columns = params.columns as usize;
    if columns == 0 {
        return Err(Error::Unsupported("CCITT Columns cannot be 0"));
    }

    let bytes_per_row = columns.div_ceil(8);

    // Pre-calculate safety bounds
    let max_allowed_rows = StreamView::MAX_DECOMPRESS_BYTES / bytes_per_row;
    let expected_rows = if params.rows > 0 {
        if (params.rows as usize) > max_allowed_rows {
            return Err(Error::Unsupported(
                "decompressed CCITT stream exceeds 256 MB safety limit",
            ));
        }
        params.rows as usize
    } else {
        0
    };

    let mut output = Vec::with_capacity(if expected_rows > 0 {
        expected_rows * bytes_per_row
    } else {
        bytes_per_row * 64
    });

    let mut reader = BitReader::new(input);
    let mut ref_line = vec![0u8; columns]; // All white initially
    let mut cur_line = vec![0u8; columns];
    let mut row_count = 0usize;

    // Decoding loop
    loop {
        if expected_rows > 0 && row_count >= expected_rows {
            break;
        }

        if reader.is_eof() {
            break;
        }

        // Check 256MB safety limit
        if output.len() + bytes_per_row > StreamView::MAX_DECOMPRESS_BYTES {
            return Err(Error::Unsupported(
                "decompressed CCITT stream exceeds 256 MB safety limit",
            ));
        }

        if params.k < 0 {
            // Group 4 (2D T.6)
            let is_eofb = decode_line_2d(&mut reader, &ref_line, &mut cur_line, columns)?;
            if is_eofb {
                break;
            }
        } else if params.k == 0 {
            // Group 3 1D
            if params.end_of_line {
                // Consume EOL if present
                if let Some(peek) = reader.peek_bits(12) {
                    if peek == 0b000000000001 {
                        reader.consume_bits(12);
                    }
                }
            }
            decode_line_1d(&mut reader, &mut cur_line, columns)?;
        } else {
            // Group 3 2D: first bit indicates 1D (1) or 2D (0)
            if params.end_of_line {
                if let Some(peek) = reader.peek_bits(12) {
                    if peek == 0b000000000001 {
                        reader.consume_bits(12);
                    }
                }
            }
            let is_1d = reader.read_bit().unwrap_or(1) == 1;
            if is_1d {
                decode_line_1d(&mut reader, &mut cur_line, columns)?;
            } else {
                let is_eofb = decode_line_2d(&mut reader, &ref_line, &mut cur_line, columns)?;
                if is_eofb {
                    break;
                }
            }
        }

        // Pack current row pixels into bytes
        let packed_row = pack_pixels(&cur_line, params.black_is1);
        output.extend_from_slice(&packed_row);

        ref_line.copy_from_slice(&cur_line);
        row_count += 1;

        if params.encoded_byte_align {
            reader.align_to_byte();
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_reader() {
        let data = [0b10110010, 0b11001100];
        let mut reader = BitReader::new(&data);
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(0));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.read_bit(), Some(1));
        assert_eq!(reader.peek_bits(4), Some(0b0010));
        reader.consume_bits(4);
        assert_eq!(reader.peek_bits(8), Some(0b11001100));
    }

    #[test]
    fn test_white_black_run_huffman() {
        // Test white terminating code for run = 2 is 0111 (4 bits)
        // Test black terminating code for run = 2 is 11 (2 bits)
        // Combined bits: 0111 1100 (0x7C)
        let data = [0b0111_1100];
        let mut reader = BitReader::new(&data);
        let w = decode_white_run(&mut reader).unwrap();
        assert_eq!(w, 2);
        let b = decode_black_run(&mut reader).unwrap();
        assert_eq!(b, 2);
    }

    #[test]
    fn test_ccitt_group4_all_white_and_vertical_mode() {
        // Group 4: line 1 all white: V(0) = 1 (since reference line is all white, V(0) makes it all white)
        // Then line 2 all white: V(0) = 1
        // Then EOFB: 000000000001 000000000001 (two 12-bit words)
        // Line 1: V(0) (1 bit: '1')
        // Line 2: V(0) (1 bit: '1')
        // EOFB: 12 bits 000000000001, 12 bits 000000000001
        // Bit stream: 1 1 000000000001 000000000001 000000 (pad to byte boundary)
        let mut bits = Vec::new();
        // bit 0: 1, bit 1: 1
        // next 12 bits: 000000000001
        // next 12 bits: 000000000001
        // In bytes:
        // Byte 0: 1100 0000 (0xC0)
        // Byte 1: 0000 0100 (0x04)
        // Byte 2: 0000 0000 (0x00)
        // Byte 3: 0100 0000 (0x40)
        bits.extend_from_slice(&[0xC0, 0x04, 0x00, 0x40]);

        let params = CcittParams {
            k: -1, // Group 4
            columns: 8,
            rows: 2,
            black_is1: true,
            ..Default::default()
        };

        let decoded = decode_ccitt_fax(&bits, &params).unwrap();
        assert_eq!(decoded.len(), 2); // 2 rows, 1 byte per row (8 columns)
        assert_eq!(decoded[0], 0x00); // All white (0 when black_is1 = true)
        assert_eq!(decoded[1], 0x00);
    }

    #[test]
    fn test_ccitt_bomb_prevention() {
        // Asking for excessive rows should trigger error immediately
        let params = CcittParams {
            k: -1,
            columns: 1728,
            rows: 2_000_000, // 2M rows * 216 bytes = 432 MB > 256 MB
            ..Default::default()
        };

        let res = decode_ccitt_fax(&[0u8; 10], &params);
        assert!(res.is_err());
        match res.unwrap_err() {
            Error::Unsupported(msg) => {
                assert!(msg.contains("256 MB"));
            }
            other => panic!("Expected Unsupported error, got {other:?}"),
        }
    }

    #[test]
    fn test_ccitt_group4_horizontal_and_alternating_runs() {
        // Line with 8 pixels: 4 white pixels, then 4 black pixels
        // Reference line: all 8 white pixels (0, 0, 0, 0, 0, 0, 0, 0)
        // At start: a0 = 0 (white).
        // Since ref line has no black pixels, b1 = 8, b2 = 8.
        // We use Horizontal mode: H code = '001' (3 bits)
        // Then white run = 4: white terminating code 4 is '1011' (4 bits)
        // Then black run = 4: black terminating code 4 is '011' (3 bits)
        // After this, a0 = 8 = columns (line finished).
        // Bits: 001 1011 011 = 10 bits:
        // 0011 0110 1100 0000 -> 0x36, 0xC0
        let data = [0b0011_0110, 0b1100_0000];

        let params = CcittParams {
            k: -1,
            columns: 8,
            rows: 1,
            black_is1: true,
            ..Default::default()
        };

        let decoded = decode_ccitt_fax(&data, &params).unwrap();
        assert_eq!(decoded.len(), 1);
        // 4 white (0) then 4 black (1) -> 0000 1111 = 0x0F
        assert_eq!(decoded[0], 0x0F);

        // Test with black_is1: false (PDF spec default: 0 is black, 1 is white)
        let params_inverted = CcittParams {
            k: -1,
            columns: 8,
            rows: 1,
            black_is1: false,
            ..Default::default()
        };
        let decoded_inv = decode_ccitt_fax(&data, &params_inverted).unwrap();
        assert_eq!(decoded_inv.len(), 1);
        // 4 white (1) then 4 black (0) -> 1111 0000 = 0xF0
        assert_eq!(decoded_inv[0], 0xF0);
    }

    #[test]
    fn test_ccitt_group4_vertical_offsets_and_pass_mode() {
        // Line 1: H mode: 4 white, 4 black -> pixels: 0 0 0 0 1 1 1 1
        // Line 2: VR(1) mode: code '011' (3 bits)
        // In line 1, first black pixel is at idx 4.
        // VR(1) sets a1 = b1 + 1 = 4 + 1 = 5. So 5 white pixels, then remaining 3 black.
        // Line 2 pixels: 0 0 0 0 0 1 1 1. Then V(0) = '1' to finish the line (a1 = b1 = 8).
        // Line 1 bits: H(001) + W4(1011) + B4(011) = 001 1011 011 (10 bits)
        // Line 2 bits: VR(1) (011) + V(0) (1) = 011 1 (4 bits)
        // Total 14 bits:
        // 0011 0110 1101 1100 -> 0x36, 0xDC
        let data = [0b0011_0110, 0b1101_1100];

        let params = CcittParams {
            k: -1,
            columns: 8,
            rows: 2,
            black_is1: true,
            ..Default::default()
        };

        let decoded = decode_ccitt_fax(&data, &params).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0], 0b0000_1111); // 4 white, 4 black
        assert_eq!(decoded[1], 0b0000_0111); // 5 white, 3 black
    }

    #[test]
    fn test_ccitt_group3_1d_decoding() {
        // Group 3 1D: k = 0
        // Line of 8 pixels: 8 white pixels
        // Terminating white code for 8 is '10011' (5 bits)
        // Bits: 1001 1000 = 0x98
        let data = [0b1001_1000];

        let params = CcittParams {
            k: 0,
            columns: 8,
            rows: 1,
            black_is1: true,
            ..Default::default()
        };

        let decoded = decode_ccitt_fax(&data, &params).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0], 0x00); // 8 white pixels
    }
}

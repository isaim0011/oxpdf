//! Filter implementations for PDF stream decompression.
//!
//! Provides pure-Rust implementations of standard PDF stream decompression filters:
//! - [`decode_ccitt_fax`]: CCITT Group 3 1D/2D and Group 4 2D facsimile decompression (ITU-T T.4 / T.6).
//! - [`decode_jbig2`]: JBIG2 bi-level image stream decoding (ISO/IEC 14492).

pub mod ccitt;
pub mod jbig2;

pub use ccitt::{decode_ccitt_fax, CcittParams};
pub use jbig2::{decode_jbig2, Jbig2Params, Jbig2SegmentHeader};

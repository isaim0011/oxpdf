pub mod agl;
pub mod cff;
pub mod truetype;

pub use agl::AdobeGlyphList;
pub use cff::CffFont;
pub use truetype::TrueTypeFont;

use crate::cmap::CMap;
use crate::error::Result;

/// Unified representation of an embedded font parsed from a PDF stream (`/FontFile2` or `/FontFile3`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddedFont {
    TrueType(TrueTypeFont),
    Cff(CffFont),
}

impl EmbeddedFont {
    /// Parses an embedded TrueType font from a `/FontFile2` byte stream.
    pub fn parse_truetype(data: &[u8]) -> Result<Self> {
        TrueTypeFont::parse(data).map(EmbeddedFont::TrueType)
    }

    /// Parses an embedded CFF font from a `/FontFile3` byte stream.
    pub fn parse_cff(data: &[u8]) -> Result<Self> {
        CffFont::parse(data).map(EmbeddedFont::Cff)
    }

    /// Maps a glyph ID to its Unicode character representation.
    #[inline]
    pub fn map_glyph_to_unicode(&self, gid: u16) -> Option<char> {
        match self {
            EmbeddedFont::TrueType(f) => f.map_glyph_to_unicode(gid),
            EmbeddedFont::Cff(f) => f.map_glyph_to_unicode(gid),
        }
    }

    /// Converts the embedded font glyph mappings into a `CMap` for text extraction.
    pub fn to_cmap(&self) -> CMap {
        match self {
            EmbeddedFont::TrueType(f) => f.to_cmap(),
            EmbeddedFont::Cff(f) => f.to_cmap(),
        }
    }
}

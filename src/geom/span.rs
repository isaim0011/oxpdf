use crate::geom::{Matrix, Rect};
use std::borrow::Cow;
use std::fmt;

/// Represents a positioned span of text extracted from a PDF page,
/// including its calculated device-space bounding box and transformation matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan<'a> {
    pub text: Cow<'a, str>,
    pub bbox: Rect,
    pub font_name: Cow<'a, str>,
    pub font_size: f32,
    pub is_bold: bool,
    pub is_italic: bool,
    pub transform: Matrix,
}

impl<'a> TextSpan<'a> {
    pub fn new(
        text: impl Into<Cow<'a, str>>,
        bbox: Rect,
        font_name: impl Into<Cow<'a, str>>,
        font_size: f32,
        is_bold: bool,
        is_italic: bool,
        transform: Matrix,
    ) -> Self {
        Self {
            text: text.into(),
            bbox,
            font_name: font_name.into(),
            font_size,
            is_bold,
            is_italic,
            transform,
        }
    }

    /// Converts this span into a fully-owned `'static` lifespan span.
    pub fn into_static(self) -> TextSpan<'static> {
        TextSpan {
            text: Cow::Owned(self.text.into_owned()),
            bbox: self.bbox,
            font_name: Cow::Owned(self.font_name.into_owned()),
            font_size: self.font_size,
            is_bold: self.is_bold,
            is_italic: self.is_italic,
            transform: self.transform,
        }
    }
}

impl<'a> fmt::Display for TextSpan<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TextSpan(\"{}\", bbox: {}, font: \"{}\" {:.1}pt, bold: {}, italic: {})",
            self.text, self.bbox, self.font_name, self.font_size, self.is_bold, self.is_italic
        )
    }
}

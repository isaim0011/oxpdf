use crate::cmap::CMap;
use crate::content::{Operation, Operator};
use crate::geom::{Matrix, Rect, TextSpan};
use crate::text::{decode_text, FontEncoding};
use crate::types::Object;
use std::borrow::Cow;
use std::collections::HashMap;

/// Metadata about a font's visual style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FontInfo {
    pub is_bold: bool,
    pub is_italic: bool,
}

/// Saved graphics state pushed/popped by `q` and `Q` operators.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsState {
    pub ctm: Matrix,
    pub font_name: String,
    pub font_size: f32,
    pub leading: f32,
    pub char_spacing: f32,
    pub word_spacing: f32,
    pub horiz_scaling: f32,
    pub rise: f32,
}

/// Tracks the PDF graphics and text state during content stream parsing,
/// maintaining CTM, text matrices (Tm, Tlm), text parameters, and calculating
/// precise device-space bounding boxes for text spans.
#[derive(Debug, Clone)]
pub struct GraphicsStateTracker {
    // Graphics state
    pub ctm: Matrix,
    pub stack: Vec<GraphicsState>,

    // Text state
    pub tm: Matrix,
    pub tlm: Matrix,
    pub font_name: String,
    pub font_size: f32,
    pub leading: f32,
    pub char_spacing: f32,
    pub word_spacing: f32,
    pub horiz_scaling: f32,
    pub rise: f32,

    // Resources & Metrics
    pub font_encodings: HashMap<String, FontEncoding>,
    pub font_cmaps: HashMap<String, CMap>,
    pub font_widths: HashMap<String, HashMap<u32, f32>>,
    pub font_missing_widths: HashMap<String, f32>,
    pub font_info: HashMap<String, FontInfo>,
    pub kerning_threshold: f32,
}

impl Default for GraphicsStateTracker {
    fn default() -> Self {
        Self {
            ctm: Matrix::identity(),
            stack: Vec::new(),
            tm: Matrix::identity(),
            tlm: Matrix::identity(),
            font_name: String::new(),
            font_size: 0.0,
            leading: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horiz_scaling: 100.0, // PDF default is 100%
            rise: 0.0,
            font_encodings: HashMap::new(),
            font_cmaps: HashMap::new(),
            font_widths: HashMap::new(),
            font_missing_widths: HashMap::new(),
            font_info: HashMap::new(),
            kerning_threshold: -100.0,
        }
    }
}

impl GraphicsStateTracker {
    /// Maximum graphics state stack nesting depth to prevent unbounded memory exhaustion.
    pub const MAX_GRAPHICS_STATE_DEPTH: usize = 256;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_font_encodings(mut self, encodings: HashMap<String, FontEncoding>) -> Self {
        self.font_encodings = encodings;
        self
    }

    pub fn with_font_cmaps(mut self, cmaps: HashMap<String, CMap>) -> Self {
        self.font_cmaps = cmaps;
        self
    }

    pub fn with_font_widths(mut self, widths: HashMap<String, HashMap<u32, f32>>) -> Self {
        self.font_widths = widths;
        self
    }

    pub fn with_font_missing_widths(mut self, missing_widths: HashMap<String, f32>) -> Self {
        self.font_missing_widths = missing_widths;
        self
    }

    pub fn with_font_info(mut self, info: HashMap<String, FontInfo>) -> Self {
        self.font_info = info;
        self
    }

    pub fn with_kerning_threshold(mut self, threshold: f32) -> Self {
        self.kerning_threshold = threshold;
        self
    }

    fn save_graphics_state(&self) -> GraphicsState {
        GraphicsState {
            ctm: self.ctm,
            font_name: self.font_name.clone(),
            font_size: self.font_size,
            leading: self.leading,
            char_spacing: self.char_spacing,
            word_spacing: self.word_spacing,
            horiz_scaling: self.horiz_scaling,
            rise: self.rise,
        }
    }

    fn restore_graphics_state(&mut self, state: GraphicsState) {
        self.ctm = state.ctm;
        self.font_name = state.font_name;
        self.font_size = state.font_size;
        self.leading = state.leading;
        self.char_spacing = state.char_spacing;
        self.word_spacing = state.word_spacing;
        self.horiz_scaling = state.horiz_scaling;
        self.rise = state.rise;
    }

    /// Decodes raw string bytes using active font CMap or Encoding.
    fn decode_bytes(&self, bytes: &[u8]) -> String {
        let clean = self.font_name.trim_start_matches('/');
        let enc = self
            .font_encodings
            .get(clean)
            .or_else(|| self.font_encodings.get(&self.font_name))
            .copied()
            .unwrap_or(FontEncoding::WinAnsiEncoding);

        if let Some(cmap) = self
            .font_cmaps
            .get(clean)
            .or_else(|| self.font_cmaps.get(&self.font_name))
        {
            cmap.decode_string_with_fallback(bytes, enc)
        } else {
            decode_text(bytes, enc)
        }
    }

    /// Resolves bold and italic status for current font.
    fn resolve_font_style(&self) -> (bool, bool) {
        let clean = self.font_name.trim_start_matches('/');
        let meta = self
            .font_info
            .get(clean)
            .or_else(|| self.font_info.get(&self.font_name))
            .copied();

        let lower = self.font_name.to_lowercase();
        let name_bold = lower.contains("bold")
            || lower.contains("black")
            || lower.contains("heavy")
            || lower.contains("demi");
        let name_italic =
            lower.contains("italic") || lower.contains("oblique") || lower.contains("slanted");

        let is_bold = meta.map(|m| m.is_bold).unwrap_or(false) || name_bold;
        let is_italic = meta.map(|m| m.is_italic).unwrap_or(false) || name_italic;

        (is_bold, is_italic)
    }

    /// Resolves glyph advance width in 1/1000 font units.
    fn get_glyph_width(&self, ch: char) -> f32 {
        let clean = self.font_name.trim_start_matches('/');
        let widths_map = self
            .font_widths
            .get(clean)
            .or_else(|| self.font_widths.get(&self.font_name));

        if let Some(map) = widths_map {
            if let Some(&w) = map.get(&(ch as u32)) {
                return w;
            }
        }

        if let Some(&mw) = self
            .font_missing_widths
            .get(clean)
            .or_else(|| self.font_missing_widths.get(&self.font_name))
        {
            return mw;
        }

        // Standard PDF font fallback width heuristics
        match ch {
            ' ' => 278.0,
            '.' | ',' | ':' | ';' | '!' | '\'' | '|' | 'i' | 'l' | 'j' | 't' | 'f' | 'r' | 'I'
            | '1' => 278.0,
            'm' | 'w' | 'M' | 'W' | '@' | '%' | '—' => 750.0,
            _ => 500.0,
        }
    }

    /// Processes a slice of parsed content stream operations, updating state and returning
    /// all generated `TextSpan`s.
    pub fn process_operations(&mut self, operations: &[Operation<'_>]) -> Vec<TextSpan<'static>> {
        let mut spans = Vec::new();
        for op in operations {
            self.process_operation(op, &mut spans);
        }
        spans
    }

    /// Processes an individual content stream operation.
    pub fn process_operation(&mut self, op: &Operation<'_>, spans: &mut Vec<TextSpan<'static>>) {
        match op.operator() {
            // Graphics state stack
            Operator::q => {
                if self.stack.len() < Self::MAX_GRAPHICS_STATE_DEPTH {
                    self.stack.push(self.save_graphics_state());
                }
            }
            Operator::Q => {
                if let Some(saved) = self.stack.pop() {
                    self.restore_graphics_state(saved);
                }
            }

            // Current Transformation Matrix
            Operator::cm => {
                if op.operands().len() >= 6 {
                    if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) = (
                        to_f32(&op.operands()[0]),
                        to_f32(&op.operands()[1]),
                        to_f32(&op.operands()[2]),
                        to_f32(&op.operands()[3]),
                        to_f32(&op.operands()[4]),
                        to_f32(&op.operands()[5]),
                    ) {
                        let cm_matrix = Matrix::new(a, b, c, d, e, f);
                        self.ctm = cm_matrix.multiply(&self.ctm);
                    }
                }
            }

            // Text object begin / end
            Operator::BT => {
                self.tm = Matrix::identity();
                self.tlm = Matrix::identity();
            }
            Operator::ET => {
                // Outside text object, text matrices undefined, but keep clean identity
                self.tm = Matrix::identity();
                self.tlm = Matrix::identity();
            }

            // Text state settings
            Operator::Tf => {
                if let Some(name_obj) = op.operands().first() {
                    let name = match name_obj {
                        Object::Name(n) => n.as_ref(),
                        Object::String(s) => std::str::from_utf8(s).unwrap_or(""),
                        _ => "",
                    };
                    self.font_name = name.trim_start_matches('/').to_string();
                }
                if let Some(size_obj) = op.operands().get(1) {
                    if let Some(size) = to_f32(size_obj) {
                        self.font_size = size;
                    }
                }
            }
            Operator::Tc => {
                if let Some(val) = op.operands().first().and_then(to_f32) {
                    self.char_spacing = val;
                }
            }
            Operator::Tw => {
                if let Some(val) = op.operands().first().and_then(to_f32) {
                    self.word_spacing = val;
                }
            }
            Operator::Tz => {
                if let Some(val) = op.operands().first().and_then(to_f32) {
                    self.horiz_scaling = val;
                }
            }
            Operator::TL => {
                if let Some(val) = op.operands().first().and_then(to_f32) {
                    self.leading = val;
                }
            }
            Operator::Ts => {
                if let Some(val) = op.operands().first().and_then(to_f32) {
                    self.rise = val;
                }
            }

            // Text positioning
            Operator::Tm => {
                if op.operands().len() >= 6 {
                    if let (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) = (
                        to_f32(&op.operands()[0]),
                        to_f32(&op.operands()[1]),
                        to_f32(&op.operands()[2]),
                        to_f32(&op.operands()[3]),
                        to_f32(&op.operands()[4]),
                        to_f32(&op.operands()[5]),
                    ) {
                        self.tm = Matrix::new(a, b, c, d, e, f);
                        self.tlm = self.tm;
                    }
                }
            }
            Operator::Td => {
                let tx = op.operands().first().and_then(to_f32).unwrap_or(0.0);
                let ty = op.operands().get(1).and_then(to_f32).unwrap_or(0.0);
                let offset = Matrix::translation(tx, ty);
                self.tlm = offset.multiply(&self.tlm);
                self.tm = self.tlm;
            }
            Operator::TD => {
                let tx = op.operands().first().and_then(to_f32).unwrap_or(0.0);
                let ty = op.operands().get(1).and_then(to_f32).unwrap_or(0.0);
                self.leading = -ty;
                let offset = Matrix::translation(tx, ty);
                self.tlm = offset.multiply(&self.tlm);
                self.tm = self.tlm;
            }
            Operator::TStar => {
                let offset = Matrix::translation(0.0, -self.leading);
                self.tlm = offset.multiply(&self.tlm);
                self.tm = self.tlm;
            }

            // Text showing: Tj
            Operator::Tj => {
                if let Some(Object::String(bytes)) = op.operands().first() {
                    self.show_text_span(bytes, spans);
                }
            }

            // Move to next line and show: '
            Operator::Quote => {
                let offset = Matrix::translation(0.0, -self.leading);
                self.tlm = offset.multiply(&self.tlm);
                self.tm = self.tlm;
                if let Some(Object::String(bytes)) = op.operands().first() {
                    self.show_text_span(bytes, spans);
                }
            }

            // Set spacing, move to next line, and show: "
            Operator::DoubleQuote => {
                if let Some(ws) = op.operands().first().and_then(to_f32) {
                    self.word_spacing = ws;
                }
                if let Some(cs) = op.operands().get(1).and_then(to_f32) {
                    self.char_spacing = cs;
                }
                let offset = Matrix::translation(0.0, -self.leading);
                self.tlm = offset.multiply(&self.tlm);
                self.tm = self.tlm;
                if let Some(Object::String(bytes)) = op.operands().get(2) {
                    self.show_text_span(bytes, spans);
                }
            }

            // Text showing with kerning: TJ
            Operator::TJ => {
                if let Some(Object::Array(items)) = op.operands().first() {
                    self.show_tj_span(items, spans);
                }
            }

            _ => {}
        }
    }

    /// Renders text from `Tj`, computes device coordinates, bounding box, advances `tm`,
    /// and emits `TextSpan`.
    fn show_text_span(&mut self, bytes: &[u8], spans: &mut Vec<TextSpan<'static>>) {
        let text = self.decode_bytes(bytes);
        if text.is_empty() {
            return;
        }

        let tdevice = self.tm.multiply(&self.ctm);
        let h_scale = self.horiz_scaling / 100.0;
        let mut total_advance = 0.0f32;

        for ch in text.chars() {
            let w0 = self.get_glyph_width(ch);
            let mut char_adv = (w0 / 1000.0) * self.font_size + self.char_spacing;
            if ch == ' ' {
                char_adv += self.word_spacing;
            }
            char_adv *= h_scale;
            total_advance += char_adv;
        }

        let eff_font_size = if self.font_size > 0.0 {
            self.font_size
        } else {
            12.0
        };
        let span_rect = Rect::new(
            0.0,
            self.rise,
            total_advance.max(0.0),
            self.rise + eff_font_size,
        );
        let bbox = tdevice.transform_rect(&span_rect);

        // Advance tm horizontal displacement
        let advance_matrix = Matrix::translation(total_advance, 0.0);
        self.tm = advance_matrix.multiply(&self.tm);

        let (is_bold, is_italic) = self.resolve_font_style();

        spans.push(TextSpan {
            text: Cow::Owned(text),
            bbox,
            font_name: Cow::Owned(self.font_name.clone()),
            font_size: self.font_size,
            is_bold,
            is_italic,
            transform: tdevice,
        });
    }

    /// Renders text array from `TJ`, computes device coordinates, bounding box, advances `tm`,
    /// and emits `TextSpan`.
    fn show_tj_span(&mut self, items: &[Object<'_>], spans: &mut Vec<TextSpan<'static>>) {
        let tdevice = self.tm.multiply(&self.ctm);
        let h_scale = self.horiz_scaling / 100.0;

        let mut full_text = String::new();
        let mut total_advance = 0.0f32;

        for item in items {
            match item {
                Object::String(bytes) => {
                    let s = self.decode_bytes(bytes);
                    for ch in s.chars() {
                        let w0 = self.get_glyph_width(ch);
                        let mut char_adv = (w0 / 1000.0) * self.font_size + self.char_spacing;
                        if ch == ' ' {
                            char_adv += self.word_spacing;
                        }
                        char_adv *= h_scale;
                        total_advance += char_adv;
                    }
                    if full_text.ends_with(' ') && s.starts_with(' ') {
                        full_text.push_str(s.trim_start());
                    } else {
                        full_text.push_str(&s);
                    }
                }
                Object::Integer(k) => {
                    let k_f32 = *k as f32;
                    let adj = -(k_f32 / 1000.0) * self.font_size * h_scale;
                    total_advance += adj;
                    if k_f32 <= self.kerning_threshold
                        && !full_text.is_empty()
                        && !full_text.ends_with(' ')
                    {
                        full_text.push(' ');
                    }
                }
                Object::Real(k) => {
                    let k_f32 = *k as f32;
                    let adj = -(k_f32 / 1000.0) * self.font_size * h_scale;
                    total_advance += adj;
                    if k_f32 <= self.kerning_threshold
                        && !full_text.is_empty()
                        && !full_text.ends_with(' ')
                    {
                        full_text.push(' ');
                    }
                }
                _ => {}
            }
        }

        if full_text.is_empty() {
            return;
        }

        let eff_font_size = if self.font_size > 0.0 {
            self.font_size
        } else {
            12.0
        };
        let span_rect = Rect::new(
            0.0,
            self.rise,
            total_advance.max(0.0),
            self.rise + eff_font_size,
        );
        let bbox = tdevice.transform_rect(&span_rect);

        // Advance tm horizontal displacement
        let advance_matrix = Matrix::translation(total_advance, 0.0);
        self.tm = advance_matrix.multiply(&self.tm);

        let (is_bold, is_italic) = self.resolve_font_style();

        spans.push(TextSpan {
            text: Cow::Owned(full_text),
            bbox,
            font_name: Cow::Owned(self.font_name.clone()),
            font_size: self.font_size,
            is_bold,
            is_italic,
            transform: tdevice,
        });
    }
}

#[inline]
fn to_f32(obj: &Object<'_>) -> Option<f32> {
    match obj {
        Object::Real(r) => Some(*r as f32),
        Object::Integer(i) => Some(*i as f32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentParser;

    #[test]
    fn test_ctm_concatenation_and_q_stack() {
        let mut tracker = GraphicsStateTracker::new();
        assert_eq!(tracker.ctm, Matrix::identity());

        // cm 2 0 0 2 10 20
        let op_cm = Operation::new(
            Operator::cm,
            vec![
                Object::Integer(2),
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(2),
                Object::Integer(10),
                Object::Integer(20),
            ],
        );
        let mut spans = Vec::new();
        tracker.process_operation(&op_cm, &mut spans);

        assert_eq!(tracker.ctm, Matrix::new(2.0, 0.0, 0.0, 2.0, 10.0, 20.0));

        // q pushes state
        tracker.process_operation(&Operation::new(Operator::q, vec![]), &mut spans);
        assert_eq!(tracker.stack.len(), 1);

        // inner cm: translate 5 5
        let op_cm2 = Operation::new(
            Operator::cm,
            vec![
                Object::Integer(1),
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(1),
                Object::Integer(5),
                Object::Integer(5),
            ],
        );
        tracker.process_operation(&op_cm2, &mut spans);
        // ctm = cm2.multiply(ctm):
        // [1 0 0 1 5 5] * [2 0 0 2 10 20]
        // row 3: 5 * 2 + 10 = 20, 5 * 2 + 20 = 30
        assert_eq!(tracker.ctm, Matrix::new(2.0, 0.0, 0.0, 2.0, 20.0, 30.0));

        // Q restores state
        tracker.process_operation(&Operation::new(Operator::Q, vec![]), &mut spans);
        assert_eq!(tracker.stack.len(), 0);
        assert_eq!(tracker.ctm, Matrix::new(2.0, 0.0, 0.0, 2.0, 10.0, 20.0));
    }

    #[test]
    fn test_text_positioning_and_spans() {
        let content = b"BT /Helvetica-Bold 12 Tf 100 200 Td (Hello) Tj 50 0 Td (World) Tj ET";
        let mut parser = ContentParser::new(content);
        let ops = parser.parse().unwrap();

        let mut tracker = GraphicsStateTracker::new();
        let spans = tracker.process_operations(&ops);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "Hello");
        assert_eq!(spans[0].font_name, "Helvetica-Bold");
        assert_eq!(spans[0].font_size, 12.0);
        assert!(spans[0].is_bold);
        assert!(!spans[0].is_italic);
        assert_eq!(spans[0].bbox.min_x, 100.0);
        assert_eq!(spans[0].bbox.min_y, 200.0);

        assert_eq!(spans[1].text, "World");
        assert!(spans[1].bbox.min_x > spans[0].bbox.max_x);
    }

    #[test]
    fn test_multiline_td_and_tstar() {
        let content = b"BT /F1 10 Tf 50 100 Td 15 TL (Line 1) Tj T* (Line 2) Tj ET";
        let mut parser = ContentParser::new(content);
        let ops = parser.parse().unwrap();

        let mut tracker = GraphicsStateTracker::new();
        let spans = tracker.process_operations(&ops);

        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "Line 1");
        assert_eq!(spans[0].bbox.min_x, 50.0);
        assert_eq!(spans[0].bbox.min_y, 100.0);

        assert_eq!(spans[1].text, "Line 2");
        assert_eq!(spans[1].bbox.min_x, 50.0);
        assert_eq!(spans[1].bbox.min_y, 85.0); // 100 - 15 leading
    }

    #[test]
    fn test_tj_kerning_and_scaling() {
        let content = b"BT /F1 12 Tf 150 Tz [(Kerning) -250 (Test)] TJ ET";
        let mut parser = ContentParser::new(content);
        let ops = parser.parse().unwrap();

        let mut tracker = GraphicsStateTracker::new();
        let spans = tracker.process_operations(&ops);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Kerning Test");
        assert!(spans[0].bbox.width() > 0.0);
    }

    #[test]
    fn test_quote_and_double_quote() {
        let content = b"BT /F1 10 Tf 12 TL (Initial) Tj (Second line) ' 2 1 (Third line) \" ET";
        let mut parser = ContentParser::new(content);
        let ops = parser.parse().unwrap();

        let mut tracker = GraphicsStateTracker::new();
        let spans = tracker.process_operations(&ops);

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].text, "Initial");
        assert_eq!(spans[1].text, "Second line");
        assert_eq!(spans[2].text, "Third line");
        assert_eq!(tracker.word_spacing, 2.0);
        assert_eq!(tracker.char_spacing, 1.0);
    }

    #[test]
    fn test_graphics_state_stack_empty_pop() {
        let mut tracker = GraphicsStateTracker::new();
        let mut spans = Vec::new();
        // Popping empty stack shouldn't panic
        tracker.process_operation(&Operation::new(Operator::Q, vec![]), &mut spans);
        assert_eq!(tracker.ctm, Matrix::identity());
    }
}

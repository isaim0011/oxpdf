use crate::error::{Error, Result};
use crate::lexer::Token;
use smallvec::SmallVec;

/// Zero-copy streaming Lexer specialized for PDF content stream operator and operand syntax.
///
/// Tuned according to oxpdf v1.0 §4.3: content streams are operator-heavy and number-dense
/// (coordinates, matrices, color values, operators like BT, ET, Tj, TJ, cm, re, etc.),
/// so this lexer utilizes fast-path numeric parsing, zero-copy string and name slicing,
/// and inline image data handling.
#[derive(Clone)]
pub struct ContentLexer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ContentLexer<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    #[inline]
    pub fn cursor(&self) -> usize {
        self.pos
    }

    #[inline]
    pub fn seek(&mut self, pos: usize) {
        self.pos = pos.min(self.data.len());
    }

    #[inline]
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    #[inline]
    pub fn raw_slice(&self) -> &'a [u8] {
        self.data
    }

    /// Fast-skips whitespace characters according to PDF specification ISO 32000-1 §7.2.2.
    #[inline]
    pub fn skip_whitespace(&mut self) {
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Fetches next token while skipping comments and whitespaces.
    pub fn next_token(&mut self) -> Result<Option<Token<'a>>> {
        loop {
            self.skip_whitespace();
            if self.pos >= self.data.len() {
                return Ok(None);
            }

            let b = self.data[self.pos];

            // Comments
            if b == b'%' {
                self.skip_comment();
                continue;
            }

            // Two-character delimiters: << and >>
            if b == b'<' && self.peek_byte(1) == Some(b'<') {
                self.pos += 2;
                return Ok(Some(Token::DictOpen));
            }
            if b == b'>' && self.peek_byte(1) == Some(b'>') {
                self.pos += 2;
                return Ok(Some(Token::DictClose));
            }

            // Single character delimiters
            match b {
                b'[' => {
                    self.pos += 1;
                    return Ok(Some(Token::ArrayOpen));
                }
                b']' => {
                    self.pos += 1;
                    return Ok(Some(Token::ArrayClose));
                }
                b'/' => return self.read_name().map(Some),
                b'(' => return self.read_literal_string().map(Some),
                b'<' => return self.read_hex_string().map(Some),
                _ => {}
            }

            // Fast path for numbers (integers, floating point numbers, coordinates)
            if b.is_ascii_digit()
                || ((b == b'+' || b == b'-') && self.peek_is_num_start())
                || (b == b'.' && self.peek_is_digit())
            {
                return self.read_number().map(Some);
            }

            // Regular operator or keyword
            return self.read_operator_or_keyword().map(Some);
        }
    }

    #[inline]
    fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.pos
            .checked_add(offset)
            .and_then(|idx| self.data.get(idx).copied())
    }

    #[inline]
    fn peek_is_num_start(&self) -> bool {
        match self.data.get(self.pos + 1) {
            Some(&b) if b.is_ascii_digit() => true,
            Some(&b'.') => match self.data.get(self.pos + 2) {
                Some(&b) => b.is_ascii_digit(),
                None => false,
            },
            _ => false,
        }
    }

    #[inline]
    fn peek_is_digit(&self) -> bool {
        match self.data.get(self.pos + 1) {
            Some(&b) => b.is_ascii_digit(),
            None => false,
        }
    }

    #[inline]
    fn peek_is_exponent_digit(&self) -> bool {
        let mut offset = 1;
        if let Some(&b) = self.data.get(self.pos + offset) {
            if b == b'+' || b == b'-' {
                offset += 1;
            }
        }
        match self.data.get(self.pos + offset) {
            Some(&b) => b.is_ascii_digit(),
            None => false,
        }
    }

    fn skip_comment(&mut self) {
        self.pos += 1; // skip '%'
        if let Some(idx) = memchr::memchr2(b'\r', b'\n', &self.data[self.pos..]) {
            self.pos += idx + 1;
            if self.pos < self.data.len()
                && self.data[self.pos - 1] == b'\r'
                && self.data[self.pos] == b'\n'
            {
                self.pos += 1;
            }
        } else {
            self.pos = self.data.len();
        }
    }

    fn read_name(&mut self) -> Result<Token<'a>> {
        self.pos += 1; // skip '/'
        let start = self.pos;
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if Self::is_delimiter_or_ws(b) {
                break;
            }
            self.pos += 1;
        }
        let raw = &self.data[start..self.pos];
        let name_str = std::str::from_utf8(raw).map_err(|_| Error::SyntaxError {
            offset: start,
            message: "Invalid UTF-8 name token",
        })?;
        Ok(Token::Name(name_str))
    }

    fn read_literal_string(&mut self) -> Result<Token<'a>> {
        let start = self.pos;
        self.pos += 1; // skip '('
        let mut depth = 1usize;
        let mut out: SmallVec<[u8; 32]> = SmallVec::new();

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;

            if b == b'(' {
                depth += 1;
                out.push(b);
            } else if b == b')' {
                depth -= 1;
                if depth == 0 {
                    return Ok(Token::String(out));
                }
                out.push(b);
            } else if b == b'\\' {
                if self.pos >= self.data.len() {
                    return Err(Error::UnterminatedString(start));
                }
                let esc = self.data[self.pos];
                self.pos += 1;
                match esc {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(0x08),
                    b'f' => out.push(0x0C),
                    b'(' => out.push(b'('),
                    b')' => out.push(b')'),
                    b'\\' => out.push(b'\\'),
                    b'\r' | b'\n' => {
                        // Line continuation: ignore newline
                        if esc == b'\r' && self.peek_byte(0) == Some(b'\n') {
                            self.pos += 1;
                        }
                    }
                    b'0'..=b'7' => {
                        // Octal escape up to 3 digits (PDF spec §7.3.4.2: high-order overflow ignored)
                        let mut oct_val = (esc - b'0') as u16;
                        for _ in 0..2 {
                            if let Some(next) = self.peek_byte(0) {
                                if (b'0'..=b'7').contains(&next) {
                                    self.pos += 1;
                                    oct_val = (oct_val << 3) + ((next - b'0') as u16);
                                } else {
                                    break;
                                }
                            }
                        }
                        out.push((oct_val & 0xFF) as u8);
                    }
                    other => out.push(other),
                }
            } else {
                out.push(b);
            }
        }

        Err(Error::UnterminatedString(start))
    }

    fn read_hex_string(&mut self) -> Result<Token<'a>> {
        let start = self.pos;
        self.pos += 1; // skip '<'
        let mut out: SmallVec<[u8; 32]> = SmallVec::new();
        let mut high_nibble: Option<u8> = None;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;

            if b == b'>' {
                if let Some(h) = high_nibble {
                    out.push(h << 4);
                }
                return Ok(Token::HexString(out));
            }

            if matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20) {
                continue;
            }

            let nibble = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return Err(Error::InvalidHexString(start)),
            };

            match high_nibble {
                None => high_nibble = Some(nibble),
                Some(h) => {
                    out.push((h << 4) | nibble);
                    high_nibble = None;
                }
            }
        }

        Err(Error::InvalidHexString(start))
    }

    fn read_number(&mut self) -> Result<Token<'a>> {
        let start = self.pos;
        let mut is_float = false;
        let mut has_exp = false;

        // Optional leading sign
        if self.pos < self.data.len()
            && (self.data[self.pos] == b'+' || self.data[self.pos] == b'-')
        {
            self.pos += 1;
        }

        // Scan digits and decimal point
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if b.is_ascii_digit() {
                self.pos += 1;
            } else if b == b'.' && !is_float {
                is_float = true;
                self.pos += 1;
            } else if (b == b'e' || b == b'E') && !has_exp && self.peek_is_exponent_digit() {
                is_float = true;
                has_exp = true;
                self.pos += 1;
                // Optional exponent sign
                if self.pos < self.data.len()
                    && (self.data[self.pos] == b'+' || self.data[self.pos] == b'-')
                {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }

        let slice = &self.data[start..self.pos];
        if slice.is_empty()
            || slice == b"+"
            || slice == b"-"
            || slice == b"."
            || slice == b"+."
            || slice == b"-."
        {
            return Err(Error::InvalidNumber(start));
        }

        if !is_float {
            if let Ok(s) = std::str::from_utf8(slice) {
                if let Ok(i) = s.parse::<i64>() {
                    return Ok(Token::Integer(i));
                }
                if let Ok(f) = s.parse::<f64>() {
                    return Ok(Token::Real(f));
                }
            }
        } else if let Ok(s) = std::str::from_utf8(slice) {
            if let Ok(f) = s.parse::<f64>() {
                return Ok(Token::Real(f));
            }
        }

        Err(Error::InvalidNumber(start))
    }

    fn read_operator_or_keyword(&mut self) -> Result<Token<'a>> {
        let start = self.pos;
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if Self::is_delimiter_or_ws(b) {
                break;
            }
            self.pos += 1;
        }

        if self.pos == start {
            self.pos += 1;
            return Err(Error::SyntaxError {
                offset: start,
                message: "Unexpected delimiter character",
            });
        }

        let slice = &self.data[start..self.pos];
        if slice == b"true" {
            return Ok(Token::Boolean(true));
        }
        if slice == b"false" {
            return Ok(Token::Boolean(false));
        }
        if slice == b"null" {
            return Ok(Token::Null);
        }

        let kw = std::str::from_utf8(slice).map_err(|_| Error::SyntaxError {
            offset: start,
            message: "Invalid UTF-8 operator keyword",
        })?;
        Ok(Token::Keyword(kw))
    }

    /// Reads raw inline image data immediately following an `ID` operator up to `EI`.
    pub fn read_inline_image_data(&mut self) -> Result<&'a [u8]> {
        // Skip single whitespace after ID operator if present
        if self.pos < self.data.len() && Self::is_whitespace(self.data[self.pos]) {
            self.pos += 1;
        }

        let start = self.pos;
        let mut idx = self.pos;
        while idx < self.data.len() {
            if let Some(pos) = memchr::memchr(b'E', &self.data[idx..]) {
                let e_pos = idx + pos;
                if e_pos + 1 < self.data.len() && self.data[e_pos + 1] == b'I' {
                    // Preceded by whitespace
                    let preceded_by_ws =
                        e_pos == start || Self::is_whitespace(self.data[e_pos - 1]);
                    // Followed by whitespace, delimiter, or EOF
                    let followed_by_delim_or_ws = e_pos + 2 >= self.data.len()
                        || Self::is_delimiter_or_ws(self.data[e_pos + 2]);

                    if preceded_by_ws && followed_by_delim_or_ws {
                        let mut data_end = e_pos;
                        if data_end > start && Self::is_whitespace(self.data[data_end - 1]) {
                            data_end -= 1;
                            if data_end > start
                                && self.data[data_end] == b'\n'
                                && self.data[data_end - 1] == b'\r'
                            {
                                data_end -= 1;
                            }
                        }
                        self.pos = e_pos + 2; // skip past "EI"
                        return Ok(&self.data[start..data_end]);
                    }
                }
                idx = e_pos + 1;
            } else {
                break;
            }
        }

        self.pos = self.data.len();
        Ok(&self.data[start..])
    }

    #[inline]
    pub fn is_delimiter(b: u8) -> bool {
        matches!(
            b,
            b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
        )
    }

    #[inline]
    pub fn is_whitespace(b: u8) -> bool {
        matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20)
    }

    #[inline]
    pub fn is_delimiter_or_ws(b: u8) -> bool {
        Self::is_whitespace(b) || Self::is_delimiter(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_lexer_numbers_and_coordinates() {
        let input = b"0 100 -250 12.345 -.75 +42 .5 1.5e-3 2E+4 100-200 0ET";
        let mut lexer = ContentLexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(0)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(100)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(-250)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Real(12.345)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Real(-0.75)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(42)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Real(0.5)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Real(0.0015)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Real(20000.0)));
        // 100-200 without spaces -> 100 then -200
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(100)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(-200)));
        // 0ET -> 0 then ET
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(0)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("ET")));
        assert_eq!(lexer.next_token().unwrap(), None);
    }

    #[test]
    fn test_content_lexer_operators_and_delimiters() {
        let input = b"BT ET Tf Tj TJ cm re m l c v y h f F f* S s B B* b b* W W* q Q rg RG k K cs CS gs ' \" T*";
        let mut lexer = ContentLexer::new(input);

        let expected = [
            "BT", "ET", "Tf", "Tj", "TJ", "cm", "re", "m", "l", "c", "v", "y", "h", "f", "F", "f*",
            "S", "s", "B", "B*", "b", "b*", "W", "W*", "q", "Q", "rg", "RG", "k", "K", "cs", "CS",
            "gs", "'", "\"", "T*",
        ];

        for op in expected {
            assert_eq!(
                lexer.next_token().unwrap(),
                Some(Token::Keyword(op)),
                "failed for operator {}",
                op
            );
        }
        assert_eq!(lexer.next_token().unwrap(), None);
    }

    #[test]
    fn test_content_lexer_strings_and_names() {
        let input = b"/F1 (Hello (nested) World) <48656C6C6F>";
        let mut lexer = ContentLexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("F1")));
        assert_eq!(
            lexer.next_token().unwrap(),
            Some(Token::String(SmallVec::from_slice(b"Hello (nested) World")))
        );
        assert_eq!(
            lexer.next_token().unwrap(),
            Some(Token::HexString(SmallVec::from_slice(b"Hello")))
        );
        assert_eq!(lexer.next_token().unwrap(), None);
    }

    #[test]
    fn test_content_lexer_inline_image() {
        let input = b"BI /W 2 /H 2 ID \x01\x02\x03\x04 EI Q";
        let mut lexer = ContentLexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("BI")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("W")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(2)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("H")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(2)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("ID")));

        let raw = lexer.read_inline_image_data().unwrap();
        assert_eq!(raw, &[1, 2, 3, 4]);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("Q")));
        assert_eq!(lexer.next_token().unwrap(), None);
    }

    #[test]
    fn test_content_lexer_comments_and_whitespace() {
        let input = b"% Start of stream\r\nBT % Text block\n (Text) Tj \n ET % End\n";
        let mut lexer = ContentLexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("BT")));
        assert_eq!(
            lexer.next_token().unwrap(),
            Some(Token::String(SmallVec::from_slice(b"Text")))
        );
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("Tj")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Keyword("ET")));
        assert_eq!(lexer.next_token().unwrap(), None);
    }
}

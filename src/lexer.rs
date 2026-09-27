use crate::error::{Error, Result};
use smallvec::SmallVec;

/// A borrowed token emitted by the zero-copy streaming lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Keyword(&'a str),
    Name(&'a str),
    Integer(i64),
    Real(f64),
    Boolean(bool),
    Null,
    String(SmallVec<[u8; 32]>),
    HexString(SmallVec<[u8; 32]>),
    DictOpen,   // <<
    DictClose,  // >>
    ArrayOpen,  // [
    ArrayClose, // ]
    Comment(&'a [u8]),
}

/// Zero-copy streaming Lexer over byte slices.
#[derive(Clone)]
pub struct Lexer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
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
                _ => return self.read_regular_token().map(Some),
            }
        }
    }

    #[inline]
    fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.data.get(self.pos + offset).copied()
    }

    fn skip_comment(&mut self) {
        self.pos += 1; // skip '%'
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            if b == b'\r' || b == b'\n' {
                break;
            }
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
                        // Octal escape up to 3 digits
                        let mut oct_val = (esc - b'0') as u8;
                        for _ in 0..2 {
                            if let Some(next) = self.peek_byte(0) {
                                if (b'0'..=b'7').contains(&next) {
                                    self.pos += 1;
                                    oct_val = (oct_val << 3) + (next - b'0');
                                } else {
                                    break;
                                }
                            }
                        }
                        out.push(oct_val);
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

    fn read_regular_token(&mut self) -> Result<Token<'a>> {
        let start = self.pos;
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if Self::is_delimiter_or_ws(b) {
                break;
            }
            self.pos += 1;
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

        // Try integer parse
        if let Ok(text) = std::str::from_utf8(slice) {
            if let Ok(i) = text.parse::<i64>() {
                return Ok(Token::Integer(i));
            }
            if let Ok(f) = text.parse::<f64>() {
                return Ok(Token::Real(f));
            }
            return Ok(Token::Keyword(text));
        }

        Err(Error::SyntaxError {
            offset: start,
            message: "Malformed regular token",
        })
    }

    #[inline]
    fn is_delimiter_or_ws(b: u8) -> bool {
        matches!(
            b,
            0x00 | 0x09
                | 0x0A
                | 0x0C
                | 0x0D
                | 0x20
                | b'('
                | b')'
                | b'<'
                | b'>'
                | b'['
                | b']'
                | b'{'
                | b'}'
                | b'/'
                | b'%'
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_primitives() {
        let input = b"true false null 123 -456 3.14159 /Catalog /Page";
        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::Boolean(true)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Boolean(false)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Null));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(123)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(-456)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Real(3.14159)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("Catalog")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("Page")));
        assert_eq!(lexer.next_token().unwrap(), None);
    }

    #[test]
    fn test_lexer_delimiters_and_strings() {
        let input = b"<< /Type /Pages /Count 1 >> [(Hello \\(world\\)) <48656c6c6f>]";
        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Some(Token::DictOpen));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("Type")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("Pages")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Name("Count")));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::Integer(1)));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::DictClose));
        assert_eq!(lexer.next_token().unwrap(), Some(Token::ArrayOpen));
        assert_eq!(
            lexer.next_token().unwrap(),
            Some(Token::String(SmallVec::from_slice(b"Hello (world)")))
        );
        assert_eq!(
            lexer.next_token().unwrap(),
            Some(Token::HexString(SmallVec::from_slice(b"Hello")))
        );
        assert_eq!(lexer.next_token().unwrap(), Some(Token::ArrayClose));
        assert_eq!(lexer.next_token().unwrap(), None);
    }
}

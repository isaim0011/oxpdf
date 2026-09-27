use crate::error::{Error, Result};
use crate::lexer::{Lexer, Token};
use crate::types::Object;
use std::collections::BTreeMap;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    max_depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            lexer: Lexer::new(data),
            max_depth: 256,
        }
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Parses the next PDF object safely with bounded recursion depth.
    pub fn parse_object(&mut self) -> Result<Option<Object<'a>>> {
        self.parse_object_depth(0)
    }

    fn parse_object_depth(&mut self, depth: usize) -> Result<Option<Object<'a>>> {
        if depth > self.max_depth {
            return Err(Error::RecursionLimitExceeded(self.max_depth));
        }

        let token = match self.lexer.next_token()? {
            Some(t) => t,
            None => return Ok(None),
        };

        match token {
            Token::Null => Ok(Some(Object::Null)),
            Token::Boolean(b) => Ok(Some(Object::Boolean(b))),
            Token::Integer(i) => {
                // Check if this is the start of an indirect reference: `id gen R`
                let checkpoint = self.lexer.cursor();
                if let Ok(Some(Token::Integer(gen))) = self.lexer.next_token() {
                    if let Ok(Some(Token::Keyword("R"))) = self.lexer.next_token() {
                        return Ok(Some(Object::Reference {
                            id: i as u32,
                            gen: gen as u16,
                        }));
                    }
                }
                // Rewind if not reference
                self.lexer.seek(checkpoint);
                Ok(Some(Object::Integer(i)))
            }
            Token::Real(f) => Ok(Some(Object::Real(f))),
            Token::Name(n) => Ok(Some(Object::Name(n))),
            Token::String(s) | Token::HexString(s) => Ok(Some(Object::String(s))),
            Token::ArrayOpen => {
                let mut arr = Vec::new();
                loop {
                    let checkpoint = self.lexer.cursor();
                    match self.lexer.next_token()? {
                        Some(Token::ArrayClose) => break,
                        Some(_) => {
                            self.lexer.seek(checkpoint);
                            if let Some(obj) = self.parse_object_depth(depth + 1)? {
                                arr.push(obj);
                            } else {
                                return Err(Error::SyntaxError {
                                    offset: self.lexer.cursor(),
                                    message: "Unclosed array",
                                });
                            }
                        }
                        None => {
                            return Err(Error::UnexpectedEof(self.lexer.cursor()));
                        }
                    }
                }
                Ok(Some(Object::Array(arr)))
            }
            Token::DictOpen => {
                let mut dict = BTreeMap::new();
                loop {
                    let checkpoint = self.lexer.cursor();
                    match self.lexer.next_token()? {
                        Some(Token::DictClose) => break,
                        Some(Token::Name(key)) => {
                            let val =
                                self.parse_object_depth(depth + 1)?
                                    .ok_or(Error::SyntaxError {
                                        offset: self.lexer.cursor(),
                                        message: "Missing value for dictionary key",
                                    })?;
                            dict.insert(key, val);
                        }
                        Some(_) => {
                            self.lexer.seek(checkpoint);
                            return Err(Error::SyntaxError {
                                offset: self.lexer.cursor(),
                                message: "Expected dictionary key (Name)",
                            });
                        }
                        None => return Err(Error::UnexpectedEof(self.lexer.cursor())),
                    }
                }
                Ok(Some(Object::Dictionary(dict)))
            }
            _ => Err(Error::SyntaxError {
                offset: self.lexer.cursor(),
                message: "Unexpected token in object position",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nested_dict_and_ref() {
        let input = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>";
        let mut parser = Parser::new(input);
        let obj = parser.parse_object().unwrap().unwrap();

        match obj {
            Object::Dictionary(dict) => {
                assert_eq!(dict.get("Type"), Some(&Object::Name("Page")));
                assert_eq!(
                    dict.get("Parent"),
                    Some(&Object::Reference { id: 2, gen: 0 })
                );
                if let Some(Object::Array(arr)) = dict.get("MediaBox") {
                    assert_eq!(arr.len(), 4);
                    assert_eq!(arr[0], Object::Integer(0));
                    assert_eq!(arr[3], Object::Integer(792));
                } else {
                    panic!("MediaBox should be Array");
                }
            }
            _ => panic!("Expected dictionary"),
        }
    }

    #[test]
    fn test_recursion_limit() {
        // Construct deeply nested array: [[[[...
        let input = vec![b'['; 300];
        let mut parser = Parser::new(&input).with_max_depth(50);
        let result = parser.parse_object();
        assert!(matches!(result, Err(Error::RecursionLimitExceeded(50))));
    }
}

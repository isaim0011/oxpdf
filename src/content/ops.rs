use crate::content::lexer::ContentLexer;
use crate::error::{Error, Result};
use crate::lexer::Token;
use crate::types::Object;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

/// Standard PDF Content Stream Operators according to ISO 32000-1 §9 & §8.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum Operator {
    // Text object
    BT,
    ET,

    // Text state
    Tf,
    Tc,
    Tw,
    Tz,
    TL,
    Tr,
    Ts,

    // Text positioning
    Td,
    TD,
    Tm,
    TStar, // T*

    // Text showing
    Tj,
    TJ,
    Quote,       // '
    DoubleQuote, // "

    // Graphics state
    q,
    Q,
    cm,
    w,
    J,
    j,
    M,
    d,
    ri,
    i,
    gs,

    // Path construction
    m,
    l,
    c,
    v,
    y,
    h,
    re,

    // Path painting
    S,
    s,
    f,
    F,
    fStar, // f*
    B,
    BStar, // B*
    b,
    bStar, // b*
    n,

    // Clipping paths
    W,
    WStar, // W*

    // Color
    CS,
    cs,
    SC,
    SCN,
    sc,
    scn,
    G,
    g,
    RG,
    rg,
    K,
    k,

    // Shading
    sh,

    // Inlined image & XObject
    Do,
    BI,
    ID,
    EI,

    // Marked content
    MP,
    DP,
    BMC,
    BDC,
    EMC,

    // Compatibility
    BX,
    EX,

    // Type 3 fonts
    d0,
    d1,

    // Fallback for custom or unrecognized operators
    Unknown(String),
}

impl Operator {
    pub fn from_str_name(s: &str) -> Self {
        match s {
            "BT" => Operator::BT,
            "ET" => Operator::ET,
            "Tf" => Operator::Tf,
            "Tc" => Operator::Tc,
            "Tw" => Operator::Tw,
            "Tz" => Operator::Tz,
            "TL" => Operator::TL,
            "Tr" => Operator::Tr,
            "Ts" => Operator::Ts,
            "Td" => Operator::Td,
            "TD" => Operator::TD,
            "Tm" => Operator::Tm,
            "T*" => Operator::TStar,
            "Tj" => Operator::Tj,
            "TJ" => Operator::TJ,
            "'" => Operator::Quote,
            "\"" => Operator::DoubleQuote,
            "q" => Operator::q,
            "Q" => Operator::Q,
            "cm" => Operator::cm,
            "w" => Operator::w,
            "J" => Operator::J,
            "j" => Operator::j,
            "M" => Operator::M,
            "d" => Operator::d,
            "ri" => Operator::ri,
            "i" => Operator::i,
            "gs" => Operator::gs,
            "m" => Operator::m,
            "l" => Operator::l,
            "c" => Operator::c,
            "v" => Operator::v,
            "y" => Operator::y,
            "h" => Operator::h,
            "re" => Operator::re,
            "S" => Operator::S,
            "s" => Operator::s,
            "f" => Operator::f,
            "F" => Operator::F,
            "f*" => Operator::fStar,
            "B" => Operator::B,
            "B*" => Operator::BStar,
            "b" => Operator::b,
            "b*" => Operator::bStar,
            "n" => Operator::n,
            "W" => Operator::W,
            "W*" => Operator::WStar,
            "CS" => Operator::CS,
            "cs" => Operator::cs,
            "SC" => Operator::SC,
            "SCN" => Operator::SCN,
            "sc" => Operator::sc,
            "scn" => Operator::scn,
            "G" => Operator::G,
            "g" => Operator::g,
            "RG" => Operator::RG,
            "rg" => Operator::rg,
            "K" => Operator::K,
            "k" => Operator::k,
            "sh" => Operator::sh,
            "Do" => Operator::Do,
            "BI" => Operator::BI,
            "ID" => Operator::ID,
            "EI" => Operator::EI,
            "MP" => Operator::MP,
            "DP" => Operator::DP,
            "BMC" => Operator::BMC,
            "BDC" => Operator::BDC,
            "EMC" => Operator::EMC,
            "BX" => Operator::BX,
            "EX" => Operator::EX,
            "d0" => Operator::d0,
            "d1" => Operator::d1,
            other => Operator::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Operator::BT => "BT",
            Operator::ET => "ET",
            Operator::Tf => "Tf",
            Operator::Tc => "Tc",
            Operator::Tw => "Tw",
            Operator::Tz => "Tz",
            Operator::TL => "TL",
            Operator::Tr => "Tr",
            Operator::Ts => "Ts",
            Operator::Td => "Td",
            Operator::TD => "TD",
            Operator::Tm => "Tm",
            Operator::TStar => "T*",
            Operator::Tj => "Tj",
            Operator::TJ => "TJ",
            Operator::Quote => "'",
            Operator::DoubleQuote => "\"",
            Operator::q => "q",
            Operator::Q => "Q",
            Operator::cm => "cm",
            Operator::w => "w",
            Operator::J => "J",
            Operator::j => "j",
            Operator::M => "M",
            Operator::d => "d",
            Operator::ri => "ri",
            Operator::i => "i",
            Operator::gs => "gs",
            Operator::m => "m",
            Operator::l => "l",
            Operator::c => "c",
            Operator::v => "v",
            Operator::y => "y",
            Operator::h => "h",
            Operator::re => "re",
            Operator::S => "S",
            Operator::s => "s",
            Operator::f => "f",
            Operator::F => "F",
            Operator::fStar => "f*",
            Operator::B => "B",
            Operator::BStar => "B*",
            Operator::b => "b",
            Operator::bStar => "b*",
            Operator::n => "n",
            Operator::W => "W",
            Operator::WStar => "W*",
            Operator::CS => "CS",
            Operator::cs => "cs",
            Operator::SC => "SC",
            Operator::SCN => "SCN",
            Operator::sc => "sc",
            Operator::scn => "scn",
            Operator::G => "G",
            Operator::g => "g",
            Operator::RG => "RG",
            Operator::rg => "rg",
            Operator::K => "K",
            Operator::k => "k",
            Operator::sh => "sh",
            Operator::Do => "Do",
            Operator::BI => "BI",
            Operator::ID => "ID",
            Operator::EI => "EI",
            Operator::MP => "MP",
            Operator::DP => "DP",
            Operator::BMC => "BMC",
            Operator::BDC => "BDC",
            Operator::EMC => "EMC",
            Operator::BX => "BX",
            Operator::EX => "EX",
            Operator::d0 => "d0",
            Operator::d1 => "d1",
            Operator::Unknown(s) => s.as_str(),
        }
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for Operator {
    fn from(s: &str) -> Self {
        Self::from_str_name(s)
    }
}

/// A parsed operation in a PDF content stream consisting of an operator and its operands.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation<'a> {
    pub operator: Operator,
    pub operands: Vec<Object<'a>>,
}

impl<'a> Operation<'a> {
    #[inline]
    pub fn new(operator: Operator, operands: Vec<Object<'a>>) -> Self {
        Self { operator, operands }
    }

    #[inline]
    pub fn operator(&self) -> &Operator {
        &self.operator
    }

    #[inline]
    pub fn operands(&self) -> &[Object<'a>] {
        &self.operands
    }

    pub fn into_owned(self) -> Operation<'static> {
        Operation {
            operator: self.operator,
            operands: self
                .operands
                .into_iter()
                .map(Object::into_owned)
                .collect(),
        }
    }
}

/// Streaming and batch parser for operations in decompressed PDF content streams.
pub struct ContentParser<'a> {
    lexer: ContentLexer<'a>,
    has_errored: bool,
}

impl<'a> ContentParser<'a> {
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            lexer: ContentLexer::new(data),
            has_errored: false,
        }
    }

    /// Parses all operations in the content stream into a vector.
    pub fn parse(&mut self) -> Result<Vec<Operation<'a>>> {
        let mut ops = Vec::new();
        while let Some(op) = self.parse_next_operation()? {
            ops.push(op);
        }
        Ok(ops)
    }

    /// Parses the next operation from the content stream, or returns None at EOF.
    pub fn parse_next_operation(&mut self) -> Result<Option<Operation<'a>>> {
        let mut operands = Vec::new();

        while let Some(token) = self.lexer.next_token()? {
            match token {
                Token::Keyword(kw) => {
                    let operator = Operator::from_str_name(kw);
                    if operator == Operator::ID {
                        let raw = self.lexer.read_inline_image_data()?;
                        operands.push(Object::Stream {
                            dict: BTreeMap::new(),
                            data: Cow::Borrowed(raw),
                        });
                    }
                    return Ok(Some(Operation::new(operator, operands)));
                }
                Token::Comment(_) => {}
                other => {
                    let obj = self.parse_object_from_token(other)?;
                    operands.push(obj);
                }
            }
        }

        if operands.is_empty() {
            Ok(None)
        } else {
            // Trailing operands at EOF without an operator (e.g. truncated content stream)
            Ok(None)
        }
    }

    fn parse_object_from_token(&mut self, token: Token<'a>) -> Result<Object<'a>> {
        self.parse_object_from_token_depth(token, 0)
    }

    fn parse_object_from_token_depth(&mut self, token: Token<'a>, depth: usize) -> Result<Object<'a>> {
        if depth > 64 {
            return Err(Error::RecursionLimitExceeded(64));
        }
        match token {
            Token::Integer(i) => Ok(Object::Integer(i)),
            Token::Real(f) => Ok(Object::Real(f)),
            Token::Boolean(b) => Ok(Object::Boolean(b)),
            Token::Null => Ok(Object::Null),
            Token::Name(n) => Ok(Object::Name(Cow::Borrowed(n))),
            Token::String(s) => Ok(Object::String(s)),
            Token::HexString(h) => Ok(Object::String(h)),
            Token::ArrayOpen => {
                let items = self.parse_array_depth(depth + 1)?;
                Ok(Object::Array(items))
            }
            Token::DictOpen => {
                let dict = self.parse_dictionary_depth(depth + 1)?;
                Ok(Object::Dictionary(dict))
            }
            Token::Keyword(kw) => Ok(Object::Name(Cow::Borrowed(kw))),
            _ => Err(Error::SyntaxError {
                offset: self.lexer.cursor(),
                message: "Unexpected token while parsing content stream operand",
            }),
        }
    }

    fn parse_array_depth(&mut self, depth: usize) -> Result<Vec<Object<'a>>> {
        let mut items = Vec::new();
        while let Some(token) = self.lexer.next_token()? {
            match token {
                Token::ArrayClose => return Ok(items),
                Token::Comment(_) => {}
                other => {
                    let obj = self.parse_object_from_token_depth(other, depth)?;
                    items.push(obj);
                }
            }
        }
        Ok(items)
    }

    fn parse_dictionary_depth(&mut self, depth: usize) -> Result<BTreeMap<Cow<'a, str>, Object<'a>>> {
        let mut dict = BTreeMap::new();
        while let Some(token) = self.lexer.next_token()? {
            match token {
                Token::DictClose => return Ok(dict),
                Token::Name(key) => {
                    if let Some(val_tok) = self.lexer.next_token()? {
                        let val = self.parse_object_from_token_depth(val_tok, depth)?;
                        dict.insert(Cow::Borrowed(key), val);
                    }
                }
                Token::Comment(_) => {}
                _ => {}
            }
        }
        Ok(dict)
    }
}

impl<'a> Iterator for ContentParser<'a> {
    type Item = Result<Operation<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_errored {
            return None;
        }
        match self.parse_next_operation() {
            Ok(Some(op)) => Some(Ok(op)),
            Ok(None) => None,
            Err(e) => {
                self.has_errored = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::SmallVec;

    #[test]
    fn test_parse_tj_single_string() {
        let stream = b"BT /F1 12 Tf 72 712 Td (Hello World) Tj ET";
        let mut parser = ContentParser::new(stream);
        let ops = parser.parse().unwrap();

        assert_eq!(ops.len(), 5);

        // 1. BT
        assert_eq!(ops[0].operator, Operator::BT);
        assert!(ops[0].operands.is_empty());

        // 2. /F1 12 Tf
        assert_eq!(ops[1].operator, Operator::Tf);
        assert_eq!(ops[1].operands.len(), 2);
        assert_eq!(ops[1].operands[0], Object::Name(Cow::Borrowed("F1")));
        assert_eq!(ops[1].operands[1], Object::Integer(12));

        // 3. 72 712 Td
        assert_eq!(ops[2].operator, Operator::Td);
        assert_eq!(ops[2].operands.len(), 2);
        assert_eq!(ops[2].operands[0], Object::Integer(72));
        assert_eq!(ops[2].operands[1], Object::Integer(712));

        // 4. (Hello World) Tj
        assert_eq!(ops[3].operator, Operator::Tj);
        assert_eq!(ops[3].operands.len(), 1);
        assert_eq!(
            ops[3].operands[0],
            Object::String(SmallVec::from_slice(b"Hello World"))
        );

        // 5. ET
        assert_eq!(ops[4].operator, Operator::ET);
        assert!(ops[4].operands.is_empty());
    }

    #[test]
    fn test_parse_tj_array_with_kerning() {
        let stream = b"BT /F2 24 Tf 100 200 Td [(W) 120 (A) -80 (V) 50 (E)] TJ ET";
        let mut parser = ContentParser::new(stream);
        let ops = parser.parse().unwrap();

        assert_eq!(ops.len(), 5);

        // Verify TJ operation
        let tj_op = &ops[3];
        assert_eq!(tj_op.operator, Operator::TJ);
        assert_eq!(tj_op.operands.len(), 1);

        match &tj_op.operands[0] {
            Object::Array(elements) => {
                assert_eq!(elements.len(), 7);
                assert_eq!(elements[0], Object::String(SmallVec::from_slice(b"W")));
                assert_eq!(elements[1], Object::Integer(120));
                assert_eq!(elements[2], Object::String(SmallVec::from_slice(b"A")));
                assert_eq!(elements[3], Object::Integer(-80));
                assert_eq!(elements[4], Object::String(SmallVec::from_slice(b"V")));
                assert_eq!(elements[5], Object::Integer(50));
                assert_eq!(elements[6], Object::String(SmallVec::from_slice(b"E")));
            }
            other => panic!("Expected Object::Array, found {:?}", other),
        }
    }

    #[test]
    fn test_parse_graphics_and_path_stream() {
        let stream = b"q 1 0 0 1 50 100 cm 0.5 0.2 0.8 rg 10 20 200 150 re f 0.1 0.1 0.1 RG 2 w 10 20 m 210 20 l 210 170 l 10 170 l h S Q";
        let mut parser = ContentParser::new(stream);
        let ops = parser.parse().unwrap();

        let op_names: Vec<&str> = ops.iter().map(|o| o.operator.as_str()).collect();
        assert_eq!(
            op_names,
            vec![
                "q", "cm", "rg", "re", "f", "RG", "w", "m", "l", "l", "l", "h", "S", "Q"
            ]
        );

        // Verify cm operands
        assert_eq!(ops[1].operands.len(), 6);
        assert_eq!(ops[1].operands[0], Object::Integer(1));
        assert_eq!(ops[1].operands[4], Object::Integer(50));
        assert_eq!(ops[1].operands[5], Object::Integer(100));

        // Verify rg operands
        assert_eq!(ops[2].operands.len(), 3);
        assert_eq!(ops[2].operands[0], Object::Real(0.5));
        assert_eq!(ops[2].operands[1], Object::Real(0.2));
        assert_eq!(ops[2].operands[2], Object::Real(0.8));

        // Verify re operands
        assert_eq!(ops[3].operands.len(), 4);
        assert_eq!(ops[3].operands[0], Object::Integer(10));
        assert_eq!(ops[3].operands[1], Object::Integer(20));
        assert_eq!(ops[3].operands[2], Object::Integer(200));
        assert_eq!(ops[3].operands[3], Object::Integer(150));
    }

    #[test]
    fn test_parse_text_positioning_and_matrix() {
        let stream = b"BT 1 0 0 1 100 200 Tm (Line 1) Tj T* (Line 2) ' 10 5 (Line 3) \" ET";
        let mut parser = ContentParser::new(stream);
        let ops = parser.parse().unwrap();

        let op_names: Vec<&str> = ops.iter().map(|o| o.operator.as_str()).collect();
        assert_eq!(
            op_names,
            vec!["BT", "Tm", "Tj", "T*", "'", "\"", "ET"]
        );

        // Verify quote (') operation: 1 string operand
        assert_eq!(ops[4].operator, Operator::Quote);
        assert_eq!(
            ops[4].operands,
            vec![Object::String(SmallVec::from_slice(b"Line 2"))]
        );

        // Verify double quote (") operation: aw, ac, string
        assert_eq!(ops[5].operator, Operator::DoubleQuote);
        assert_eq!(ops[5].operands.len(), 3);
        assert_eq!(ops[5].operands[0], Object::Integer(10));
        assert_eq!(ops[5].operands[1], Object::Integer(5));
        assert_eq!(
            ops[5].operands[2],
            Object::String(SmallVec::from_slice(b"Line 3"))
        );
    }

    #[test]
    fn test_parse_marked_content_and_dict() {
        let stream = b"/Span << /MCID 0 /Lang (en-US) >> BDC (Tagged text) Tj EMC";
        let mut parser = ContentParser::new(stream);
        let ops = parser.parse().unwrap();

        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].operator, Operator::BDC);
        assert_eq!(ops[0].operands.len(), 2);
        assert_eq!(ops[0].operands[0], Object::Name(Cow::Borrowed("Span")));

        if let Object::Dictionary(dict) = &ops[0].operands[1] {
            assert_eq!(dict.get("MCID"), Some(&Object::Integer(0)));
            assert_eq!(
                dict.get("Lang"),
                Some(&Object::String(SmallVec::from_slice(b"en-US")))
            );
        } else {
            panic!("Expected dictionary operand in BDC");
        }

        assert_eq!(ops[1].operator, Operator::Tj);
        assert_eq!(ops[2].operator, Operator::EMC);
    }

    #[test]
    fn test_content_parser_iterator() {
        let stream = b"BT 100 200 Td (Streamed Text) Tj ET";
        let parser = ContentParser::new(stream);

        let ops: Vec<Operation> = parser.map(|res| res.unwrap()).collect();
        assert_eq!(ops.len(), 4);
        assert_eq!(ops[0].operator, Operator::BT);
        assert_eq!(ops[1].operator, Operator::Td);
        assert_eq!(ops[2].operator, Operator::Tj);
        assert_eq!(ops[3].operator, Operator::ET);
    }

    #[test]
    fn test_operator_display_and_roundtrip() {
        let names = ["BT", "ET", "Tj", "TJ", "cm", "re", "m", "l", "c", "v", "y", "h", "f", "F", "f*", "S", "s", "B", "B*", "b", "b*", "W", "W*", "q", "Q", "rg", "RG", "k", "K", "cs", "CS", "gs", "'", "\"", "T*", "customOp"];
        for name in names {
            let op = Operator::from_str_name(name);
            assert_eq!(op.as_str(), name);
            assert_eq!(format!("{}", op), name);
        }
    }

    #[test]
    fn test_operation_into_owned() {
        let stream = b"/Span << /ActualText (Hello) >> BDC";
        let mut parser = ContentParser::new(stream);
        let op = parser.parse_next_operation().unwrap().unwrap();
        let owned_op = op.into_owned();
        assert_eq!(owned_op.operator, Operator::BDC);
        assert_eq!(owned_op.operands.len(), 2);
    }
}

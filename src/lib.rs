pub mod cmap;
pub mod content;
pub mod document;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod recover;
pub mod simd;
pub mod source;
pub mod stream;
pub mod text;
pub mod types;
pub mod writer;
pub mod xref;

pub use cmap::CMap;
pub use content::{ContentLexer, ContentParser, Operation, Operator};
pub use document::Document;
pub use error::{Error, Result};
pub use lexer::{Lexer, Token};
#[cfg(not(target_arch = "wasm32"))]
pub use source::MmapSource;
pub use source::{BufferSource, PdfSource};
pub use stream::{FilterKind, StreamView};
pub use text::{decode_text, FontEncoding, TextExtractor};
pub use types::Object;
pub use writer::Serializer;
pub use xref::{XRefEntry, XRefTable};

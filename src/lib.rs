pub mod document;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod recover;
pub mod source;
pub mod stream;
pub mod types;
pub mod writer;
pub mod xref;

pub use document::Document;
pub use error::{Error, Result};
pub use lexer::{Lexer, Token};
#[cfg(not(target_arch = "wasm32"))]
pub use source::MmapSource;
pub use source::{BufferSource, PdfSource};
pub use stream::{FilterKind, StreamView};
pub use types::Object;
pub use writer::Serializer;
pub use xref::{XRefEntry, XRefTable};

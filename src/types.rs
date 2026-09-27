use smallvec::SmallVec;
use std::collections::BTreeMap;

/// Borrowed or lightweight PDF Object model optimized for zero-copy inspection.
#[derive(Debug, Clone, PartialEq)]
pub enum Object<'a> {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    Name(&'a str),
    String(SmallVec<[u8; 32]>),
    Array(Vec<Object<'a>>),
    Dictionary(BTreeMap<&'a str, Object<'a>>),
    Reference { id: u32, gen: u16 },
}

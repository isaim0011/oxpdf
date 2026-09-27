use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// Borrowed or lightweight PDF Object model optimized for zero-copy inspection.
#[derive(Debug, Clone, PartialEq)]
pub enum Object<'a> {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    Name(Cow<'a, str>),
    String(SmallVec<[u8; 32]>),
    Array(Vec<Object<'a>>),
    Dictionary(BTreeMap<Cow<'a, str>, Object<'a>>),
    Stream {
        dict: BTreeMap<Cow<'a, str>, Object<'a>>,
        data: Cow<'a, [u8]>,
    },
    Reference {
        id: u32,
        gen: u16,
    },
}

impl<'a> Object<'a> {
    pub fn name<S: Into<Cow<'a, str>>>(name: S) -> Self {
        Object::Name(name.into())
    }

    pub fn as_name(&self) -> Option<&str> {
        match self {
            Object::Name(n) => Some(n.as_ref()),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&BTreeMap<Cow<'a, str>, Object<'a>>> {
        match self {
            Object::Dictionary(d) => Some(d),
            Object::Stream { dict, .. } => Some(dict),
            _ => None,
        }
    }

    pub fn into_owned(self) -> Object<'static> {
        match self {
            Object::Null => Object::Null,
            Object::Boolean(b) => Object::Boolean(b),
            Object::Integer(i) => Object::Integer(i),
            Object::Real(f) => Object::Real(f),
            Object::Name(n) => Object::Name(Cow::Owned(n.into_owned())),
            Object::String(s) => Object::String(s),
            Object::Array(arr) => Object::Array(arr.into_iter().map(Object::into_owned).collect()),
            Object::Dictionary(dict) => {
                let mut owned = BTreeMap::new();
                for (k, v) in dict {
                    owned.insert(Cow::Owned(k.into_owned()), v.into_owned());
                }
                Object::Dictionary(owned)
            }
            Object::Stream { dict, data } => {
                let mut owned_dict = BTreeMap::new();
                for (k, v) in dict {
                    owned_dict.insert(Cow::Owned(k.into_owned()), v.into_owned());
                }
                Object::Stream {
                    dict: owned_dict,
                    data: Cow::Owned(data.into_owned()),
                }
            }
            Object::Reference { id, gen } => Object::Reference { id, gen },
        }
    }
}

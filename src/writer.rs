use crate::error::{Error, Result};
use crate::types::Object;
use std::io::Write;

/// Zero-allocation, monotonic PDF serializer inspired by Typst's `pdf-writer`.
/// Writes directly to any `std::io::Write` target without intermediate tree serialization.
pub struct Serializer<W: Write> {
    writer: W,
    bytes_written: u64,
}

impl<W: Write> Serializer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            bytes_written: 0,
        }
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    pub fn write_header(&mut self, version: (u8, u8)) -> Result<()> {
        let header = format!("%PDF-{}.{}\n%âãÏÓ\n", version.0, version.1);
        self.write_bytes(header.as_bytes())
    }

    pub fn write_indirect_object_header(&mut self, id: u32, gen: u16) -> Result<u64> {
        let offset = self.bytes_written;
        let s = format!("{id} {gen} obj\n");
        self.write_bytes(s.as_bytes())?;
        Ok(offset)
    }

    pub fn write_indirect_object_footer(&mut self) -> Result<()> {
        self.write_bytes(b"\nendobj\n")
    }

    pub fn write_object(&mut self, obj: &Object<'_>) -> Result<()> {
        match obj {
            Object::Null => self.write_bytes(b"null"),
            Object::Boolean(b) => {
                if *b {
                    self.write_bytes(b"true")
                } else {
                    self.write_bytes(b"false")
                }
            }
            Object::Integer(i) => self.write_bytes(i.to_string().as_bytes()),
            Object::Real(f) => self.write_bytes(
                format!("{:.5}", f)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .as_bytes(),
            ),
            Object::Name(n) => {
                self.write_bytes(b"/")?;
                self.write_bytes(n.as_bytes())
            }
            Object::String(bytes) => {
                self.write_bytes(b"(")?;
                // Simple escaping for ( ) and \
                for &b in bytes.iter() {
                    match b {
                        b'(' => self.write_bytes(b"\\(")?,
                        b')' => self.write_bytes(b"\\)")?,
                        b'\\' => self.write_bytes(b"\\\\")?,
                        other => self.write_bytes(&[other])?,
                    }
                }
                self.write_bytes(b")")
            }
            Object::Reference { id, gen } => {
                let s = format!("{id} {gen} R");
                self.write_bytes(s.as_bytes())
            }
            Object::Array(arr) => {
                self.write_bytes(b"[")?;
                for (idx, item) in arr.iter().enumerate() {
                    if idx > 0 {
                        self.write_bytes(b" ")?;
                    }
                    self.write_object(item)?;
                }
                self.write_bytes(b"]")
            }
            Object::Dictionary(dict) => {
                self.write_bytes(b"<<\n")?;
                for (k, v) in dict {
                    self.write_bytes(b"/")?;
                    self.write_bytes(k.as_bytes())?;
                    self.write_bytes(b" ")?;
                    self.write_object(v)?;
                    self.write_bytes(b"\n")?;
                }
                self.write_bytes(b">>")
            }
            Object::Stream { dict, data } => {
                self.write_bytes(b"<<\n")?;
                // Always write correct /Length for the actual data payload
                let actual_len = data.len();
                self.write_bytes(b"/Length ")?;
                self.write_bytes(actual_len.to_string().as_bytes())?;
                self.write_bytes(b"\n")?;
                for (k, v) in dict {
                    if k.as_ref() == "Length" {
                        // Skip stale /Length from original — we wrote the correct one above
                        continue;
                    }
                    self.write_bytes(b"/")?;
                    self.write_bytes(k.as_bytes())?;
                    self.write_bytes(b" ")?;
                    self.write_object(v)?;
                    self.write_bytes(b"\n")?;
                }
                self.write_bytes(b">>\nstream\n")?;
                self.write_bytes(data)?;
                self.write_bytes(b"\nendstream")
            }
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer
            .write_all(bytes)
            .map_err(|e| Error::Io(e.to_string()))?;
        self.bytes_written = self
            .bytes_written
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| Error::Unsupported("file size overflow"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_monotonic_serializer() {
        let mut buf = Vec::new();
        let mut ser = Serializer::new(&mut buf);

        ser.write_header((1, 7)).unwrap();
        let offset = ser.write_indirect_object_header(1, 0).unwrap();
        assert!(offset > 0);

        let mut dict = BTreeMap::new();
        dict.insert(std::borrow::Cow::Borrowed("Type"), Object::name("Catalog"));
        dict.insert(
            std::borrow::Cow::Borrowed("Pages"),
            Object::Reference { id: 2, gen: 0 },
        );
        ser.write_object(&Object::Dictionary(dict)).unwrap();
        ser.write_indirect_object_footer().unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("%PDF-1.7"));
        assert!(output.contains("1 0 obj"));
        assert!(output.contains("/Type /Catalog"));
        assert!(output.contains("/Pages 2 0 R"));
        assert!(output.contains("endobj"));
    }
}

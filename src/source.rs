use crate::error::{Error, Result};
use std::fs::File;

/// Abstract source trait for reading PDF byte content.
///
/// Implements §5 of the specification:
/// Supports zero-copy in-memory buffering (`BufferSource`) and OS memory-mapped
/// file access (`MmapSource`) to guarantee bounded RAM usage for multi-gigabyte files.
pub trait PdfSource {
    /// Reads a subslice of `len` bytes starting at `offset`.
    fn read_at(&self, offset: usize, len: usize) -> Result<&[u8]>;

    /// Total byte length of the source.
    fn len(&self) -> usize;

    /// Whether the source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Access the entire backing slice if contiguous in memory.
    fn as_slice(&self) -> Result<&[u8]>;
}

/// In-memory slice backed source. Ideal for WASM32, testing, and callers with in-memory buffers.
pub struct BufferSource<'a> {
    data: &'a [u8],
}

impl<'a> BufferSource<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl<'a> PdfSource for BufferSource<'a> {
    fn read_at(&self, offset: usize, len: usize) -> Result<&[u8]> {
        if offset + len > self.data.len() {
            return Err(Error::TruncatedFile {
                expected_offset: offset + len,
                file_len: self.data.len(),
            });
        }
        Ok(&self.data[offset..offset + len])
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn as_slice(&self) -> Result<&[u8]> {
        Ok(self.data)
    }
}

impl<'a> From<&'a [u8]> for BufferSource<'a> {
    fn from(data: &'a [u8]) -> Self {
        Self::new(data)
    }
}

/// Memory-mapped file backed source for native OS environments.
///
/// Implements §5: The OS page cache manages memory pressure, enabling 10GB+ files
/// to be inspected without allocating large byte vectors in process memory.
#[cfg(not(target_arch = "wasm32"))]
pub struct MmapSource {
    mmap: memmap2::Mmap,
}

#[cfg(not(target_arch = "wasm32"))]
impl MmapSource {
    /// Memory maps the provided open file for zero-copy random access.
    pub fn open(file: &File) -> Result<Self> {
        let mmap =
            unsafe { memmap2::Mmap::map(file).map_err(|e| Error::Io(format!("mmap error: {e}")))? };
        Ok(Self { mmap })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PdfSource for MmapSource {
    fn read_at(&self, offset: usize, len: usize) -> Result<&[u8]> {
        if offset + len > self.mmap.len() {
            return Err(Error::TruncatedFile {
                expected_offset: offset + len,
                file_len: self.mmap.len(),
            });
        }
        Ok(&self.mmap[offset..offset + len])
    }

    fn len(&self) -> usize {
        self.mmap.len()
    }

    fn as_slice(&self) -> Result<&[u8]> {
        Ok(&self.mmap[..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_source() {
        let sample = b"hello pdf world";
        let src = BufferSource::new(sample);

        assert_eq!(src.len(), sample.len());
        assert!(!src.is_empty());
        assert_eq!(src.read_at(6, 3).unwrap(), b"pdf");
        assert_eq!(src.as_slice().unwrap(), sample);

        // Out of bounds check returns TruncatedFile
        assert!(src.read_at(10, 10).is_err());
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_mmap_source() {
        let tmp = tempfile_stub::create_temp(b"%PDF-1.4\n1 0 obj\n<<>>\nendobj\n");
        let src = MmapSource::open(&tmp).unwrap();
        assert_eq!(src.read_at(0, 8).unwrap(), b"%PDF-1.4");
        assert_eq!(src.len(), 29);
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tempfile_stub {
    use std::fs::File;
    use std::io::Write;

    pub fn create_temp(content: &[u8]) -> File {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("oxpdf_test_{}.pdf", std::process::id()));
        let mut file = File::create(&path).unwrap();
        file.write_all(content).unwrap();
        drop(file);
        File::open(&path).unwrap()
    }
}

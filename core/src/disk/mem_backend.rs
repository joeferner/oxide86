use anyhow::{Result, anyhow};

use crate::disk::DiskBackend;

/// In-memory DiskBackend backed by a Vec<u8>.
pub struct MemBackend {
    data: Vec<u8>,
}

impl MemBackend {
    /// Create a zeroed buffer of the given size.
    #[cfg(test)]
    pub(crate) fn zeroed(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
        }
    }
}

impl DiskBackend for MemBackend {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let offset = offset as usize;
        if offset >= self.data.len() {
            return Err(anyhow!(
                "read_at: offset {} out of range (size {})",
                offset,
                self.data.len()
            ));
        }
        let end = (offset + buf.len()).min(self.data.len());
        let n = end - offset;
        buf[..n].copy_from_slice(&self.data[offset..end]);
        Ok(n)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        let offset = offset as usize;
        if offset >= self.data.len() {
            return Err(anyhow!(
                "write_at: offset {} out of range (size {})",
                offset,
                self.data.len()
            ));
        }
        let end = (offset + buf.len()).min(self.data.len());
        let n = end - offset;
        self.data[offset..end].copy_from_slice(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }

    fn size(&self) -> u64 {
        self.data.len() as u64
    }
}

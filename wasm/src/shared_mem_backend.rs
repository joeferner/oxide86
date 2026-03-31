use std::sync::{Arc, RwLock};

use oxide86_core::disk::DiskBackend;

/// An in-memory DiskBackend backed by an Arc<RwLock<Vec<u8>>>, so that writes
/// made by the emulator are immediately visible through the shared reference.
pub(crate) struct SharedMemBackend {
    pub(crate) data: Arc<RwLock<Vec<u8>>>,
}

impl DiskBackend for SharedMemBackend {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> anyhow::Result<usize> {
        let data = self.data.read().unwrap();
        let offset = offset as usize;
        if offset >= data.len() {
            return Err(anyhow::anyhow!(
                "read_at: offset {} out of range (size {})",
                offset,
                data.len()
            ));
        }
        let end = (offset + buf.len()).min(data.len());
        let n = end - offset;
        buf[..n].copy_from_slice(&data[offset..end]);
        Ok(n)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> anyhow::Result<usize> {
        let mut data = self.data.write().unwrap();
        let offset = offset as usize;
        if offset >= data.len() {
            return Err(anyhow::anyhow!(
                "write_at: offset {} out of range (size {})",
                offset,
                data.len()
            ));
        }
        let end = (offset + buf.len()).min(data.len());
        let n = end - offset;
        data[offset..end].copy_from_slice(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn size(&self) -> u64 {
        self.data.read().unwrap().len() as u64
    }
}

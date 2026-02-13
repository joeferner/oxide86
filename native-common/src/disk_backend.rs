use anyhow::{Context, Result};
use emu86_core::DiskBackend;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

/// File-backed disk storage for native platform.
/// Reads and writes go directly to the disk image file.
pub struct FileDiskBackend {
    file: File,
    size: u64,
}

impl FileDiskBackend {
    /// Open a disk image file for read/write access
    pub fn open(path: &str, read_only: bool) -> Result<Self> {
        let file = if read_only {
            File::open(path).with_context(|| format!("Failed to open disk image: {}", path))?
        } else {
            File::options()
                .read(true)
                .write(true)
                .open(path)
                .with_context(|| format!("Failed to open disk image for read/write: {}", path))?
        };
        let size = file
            .metadata()
            .with_context(|| format!("Failed to get metadata for: {}", path))?
            .len();
        Ok(Self { file, size })
    }
}

impl DiskBackend for FileDiskBackend {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.file.seek(SeekFrom::Start(offset))?;
        let bytes_read = self.file.read(buf)?;
        Ok(bytes_read)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize> {
        self.file.seek(SeekFrom::Start(offset))?;
        let bytes_written = self.file.write(buf)?;
        // Flush immediately to ensure data is persisted
        self.file.flush()?;
        Ok(bytes_written)
    }

    fn flush(&mut self) -> Result<()> {
        self.file.flush()?;
        Ok(())
    }

    fn size(&self) -> u64 {
        self.size
    }
}

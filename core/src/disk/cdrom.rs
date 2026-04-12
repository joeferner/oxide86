use super::DiskBackend;

pub enum CdromError {
    ReadError,
    OutOfRange,
}

pub trait CdromBackend {
    fn read_sector(&mut self, lba: u32, buf: &mut [u8; 2048]) -> Result<(), CdromError>;
    fn total_sectors(&self) -> u32;
}

pub struct BackedCdrom<B: DiskBackend> {
    backend: B,
}

impl<B: DiskBackend> BackedCdrom<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B: DiskBackend> CdromBackend for BackedCdrom<B> {
    fn read_sector(&mut self, lba: u32, buf: &mut [u8; 2048]) -> Result<(), CdromError> {
        let offset = lba as u64 * 2048;
        self.backend
            .read_at(offset, buf)
            .map_err(|_| CdromError::ReadError)?;
        Ok(())
    }

    fn total_sectors(&self) -> u32 {
        (self.backend.size() / 2048) as u32
    }
}

/// Standard I/O implementation of Bios for native platform
use emu86_core::{Bios, DiskController, SECTOR_SIZE};
use emu86_core::cpu::bios::{DriveParams, disk_errors};
use std::io::{self, Read, Write};

pub struct StdioBios<D: DiskController> {
    disk: D,
}

impl<D: DiskController> StdioBios<D> {
    pub fn new(disk: D) -> Self {
        Self { disk }
    }
}

impl<D: DiskController> Bios for StdioBios<D> {
    fn read_char(&mut self) -> Option<u8> {
        let mut buffer = [0u8; 1];
        match io::stdin().read_exact(&mut buffer) {
            Ok(_) => Some(buffer[0]),
            Err(_) => None,
        }
    }

    fn write_char(&mut self, ch: u8) {
        print!("{}", ch as char);
        let _ = io::stdout().flush();
    }

    fn write_str(&mut self, s: &str) {
        print!("{}", s);
        let _ = io::stdout().flush();
    }

    fn disk_reset(&mut self, _drive: u8) -> bool {
        // Always succeed for reset
        true
    }

    fn disk_read_sectors(
        &mut self,
        _drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, u8> {
        let mut result = Vec::with_capacity(count as usize * SECTOR_SIZE);

        for i in 0..count {
            // Calculate CHS for each sector
            let current_sector = sector + i;

            match self.disk.read_sector_chs(cylinder as u16, head as u16, current_sector as u16) {
                Ok(sector_data) => {
                    result.extend_from_slice(&sector_data);
                }
                Err(_) => {
                    return Err(disk_errors::SECTOR_NOT_FOUND);
                }
            }
        }

        Ok(result)
    }

    fn disk_write_sectors(
        &mut self,
        _drive: u8,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
        data: &[u8],
    ) -> Result<u8, u8> {
        if self.disk.is_read_only() {
            return Err(disk_errors::WRITE_PROTECTED);
        }

        let mut sectors_written = 0;

        for i in 0..count {
            let offset = i as usize * SECTOR_SIZE;
            if offset + SECTOR_SIZE > data.len() {
                break;
            }

            let current_sector = sector + i;
            let mut sector_data = [0u8; SECTOR_SIZE];
            sector_data.copy_from_slice(&data[offset..offset + SECTOR_SIZE]);

            match self.disk.write_sector_chs(cylinder as u16, head as u16, current_sector as u16, &sector_data) {
                Ok(_) => {
                    sectors_written += 1;
                }
                Err(_) => {
                    if sectors_written == 0 {
                        return Err(disk_errors::SECTOR_NOT_FOUND);
                    } else {
                        return Ok(sectors_written);
                    }
                }
            }
        }

        Ok(sectors_written)
    }

    fn disk_get_params(&self, _drive: u8) -> Result<DriveParams, u8> {
        let geom = self.disk.geometry();
        Ok(DriveParams {
            max_cylinder: (geom.cylinders - 1).min(255) as u8,
            max_head: (geom.heads - 1).min(255) as u8,
            max_sector: geom.sectors_per_track.min(63) as u8,
            drive_count: 1,
        })
    }
}

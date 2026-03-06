use std::sync::{Arc, RwLock};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crate::{
    Device,
    bus::Bus,
    byte_to_printable_char,
    cpu::{Cpu, CpuType, bios::int09_keyboard_hardware_interrupt::scan_code_to_ascii},
    devices::rtc::{CMOS_REG_FLOPPY_TYPES, Clock, RTC_IO_PORT_DATA, RTC_IO_PORT_REGISTER_SELECT},
    disk::{Disk, DiskError, DriveNumber},
    memory::Memory,
    physical_address,
    video::{VideoBuffer, VideoCard, VideoCardType},
};
#[cfg(test)]
use crate::devices::uart::ComPortDevice;
use anyhow::{Result, anyhow};

pub struct ComputerConfig {
    pub cpu_type: CpuType,
    pub clock_speed: u32,
    pub memory_size: usize,
    pub clock: Box<dyn Clock>,
    pub hard_disks: Vec<Box<dyn Disk>>,
    pub video_card_type: VideoCardType,
    pub video_buffer: Arc<RwLock<VideoBuffer>>,
}

pub struct Computer {
    cpu: Cpu,
    bus: Bus,
    key_presses: VecDeque<u8>,
    boot_drive: Option<DriveNumber>,
}

impl Computer {
    pub fn new(config: ComputerConfig) -> Self {
        let cpu = Cpu::new(config.cpu_type, config.clock_speed);
        log::info!("Memory {}kb", config.memory_size / 1024);
        let memory = Memory::new(config.memory_size);
        let clock_speed = cpu.clock_speed();
        let clock = if config.cpu_type == CpuType::I8086 {
            None
        } else {
            Some(config.clock)
        };
        let video_card = Rc::new(RefCell::new(VideoCard::new(
            config.video_card_type,
            config.video_buffer,
        )));
        let mut computer = Self {
            cpu,
            bus: Bus::new(memory, clock_speed, clock, config.hard_disks, video_card),
            key_presses: VecDeque::new(),
            boot_drive: None,
        };
        computer.reset();
        computer
    }

    pub fn add_device<T: Device + 'static>(&mut self, device: T) {
        self.bus.add_device(device);
    }

    /// Insert or eject the floppy disk for the given drive. Returns the previous disk if any.
    /// Pass `None` to eject; the CMOS drive-type register is only updated on insert.
    pub fn set_floppy_disk(
        &mut self,
        drive: DriveNumber,
        disk: Option<Box<dyn Disk>>,
    ) -> Option<Box<dyn Disk>> {
        if let Some(ref d) = disk {
            // Derive CMOS type code from geometry.
            // Values: 0=none, 1=360KB 5.25", 2=1.2MB 5.25", 3=720KB 3.5", 4=1.44MB 3.5", 5=2.88MB 3.5"
            let cmos_type: u8 = match d.disk_geometry().total_size {
                1_474_560 => 0x04,
                737_280 => 0x03,
                368_640 | 163_840 => 0x01,
                _ => 0x00,
            };

            // Update CMOS register 0x10 (floppy drive types).
            // Read current value first to preserve the other drive's nibble.
            self.bus
                .io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
            let current = self.bus.io_read_u8(RTC_IO_PORT_DATA);
            let current = if current == 0xFF { 0x00 } else { current };
            let floppy_types = if drive.as_floppy_index() == 0 {
                (cmos_type << 4) | (current & 0x0F) // drive A in bits 7:4
            } else {
                (current & 0xF0) | (cmos_type & 0x0F) // drive B in bits 3:0
            };
            self.bus
                .io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
            self.bus.io_write_u8(RTC_IO_PORT_DATA, floppy_types);
        }

        self.bus.floppy_controller_mut().set_drive_disk(drive, disk)
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = physical_address(segment, offset);
        self.bus.load_at(physical_addr, program_data)?;
        self.cpu.reset(segment, offset, None);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn run(&mut self) {
        while self.get_exit_code().is_none() && !self.cpu.wait_for_key_press() {
            self.step();
        }
    }

    pub fn step(&mut self) {
        self.process_key_presses();
        self.cpu.step(&mut self.bus);
        if self.cpu.at_reset_vector() {
            if let Some(drive) = self.boot_drive {
                log::info!("Rebooting from drive {}", drive.to_letter());
                if let Err(e) = self.boot(drive) {
                    log::error!("Reboot failed: {e}");
                }
            } else {
                log::warn!("Reset vector reached but no boot drive set; resetting CPU");
                self.reset();
            }
        }
    }

    fn reset(&mut self) {
        self.bus.reset();
        self.cpu.reset(0xffff, 0x0000, None);
    }

    /// Boot from disk by loading boot sector to 0x0000:0x7C00
    /// This simulates the BIOS boot process:
    /// 1. Read sector 0 (cylinder 0, head 0, sector 1) from the specified drive
    /// 2. Load it to physical address 0x7C00
    /// 3. Set CS:IP to 0x0000:0x7C00
    /// 4. Set DL to boot drive number
    pub fn boot(&mut self, drive: DriveNumber) -> Result<()> {
        // If floppy boot fails, fall back to hard drive C — same behavior as a real BIOS.
        if drive.is_floppy() {
            let hdd = DriveNumber::hard_drive_c();
            return match self.boot_from(drive) {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::warn!(
                        "Boot from {} failed ({}), falling back to hard drive C:",
                        drive.to_letter(),
                        e
                    );
                    self.boot_from(hdd)
                }
            };
        }
        self.boot_from(drive)
    }

    fn boot_from(&mut self, drive: DriveNumber) -> Result<()> {
        // Reset bus state (IVT, BDA, devices) before booting so that any modifications
        // made by a previous session (e.g. DOS replacing INT 13h at 0x0070) don't persist.
        self.bus.reset();

        // Read boot sector: cylinder 0, head 0, sector 1
        let boot_sector = if drive.is_floppy() {
            self.bus.floppy_controller().read_sectors(drive, 0, 0, 1, 1)
        } else {
            self.bus
                .hard_disk_controller()
                .get_disk(drive)
                .ok_or(DiskError::DriveNotReady)
                .and_then(|disk| disk.read_sectors(0, 0, 1, 1))
        }
        .map_err(|err| anyhow!("failed to read boot sector: {err}"))?;

        if boot_sector.len() != 512 {
            return Err(anyhow::anyhow!(
                "Boot sector must be exactly 512 bytes, got {}",
                boot_sector.len()
            ));
        }

        // Verify boot signature (0x55AA at offset 510-511)
        // Some old "booter" games predate the convention and lack this signature; warn but continue.
        if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
            log::warn!(
                "Boot sector missing 0x55AA signature (got 0x{:02X}{:02X}); proceeding anyway",
                boot_sector[511],
                boot_sector[510]
            );
        }

        // Load boot sector to 0x0000:0x7C00 (physical address 0x7C00)
        const BOOT_SEGMENT: u16 = 0x0000;
        const BOOT_OFFSET: u16 = 0x7C00;
        let boot_addr = physical_address(BOOT_SEGMENT, BOOT_OFFSET);
        self.bus.load_at(boot_addr, &boot_sector)?;
        self.cpu.reset(BOOT_SEGMENT, BOOT_OFFSET, Some(drive));
        self.boot_drive = Some(drive);

        // TODO
        // // Set current drive to match boot drive
        // // Convert BIOS drive number to DOS drive number: 0x00->0, 0x01->1, 0x80->2, 0x81->3
        // self.bios.set_default_drive(drive);
        //
        // // Pre-allocate memory for DOS kernel
        // // In a real system, DOS would already be loaded in memory before
        // // the memory allocator starts. We simulate this by pre-allocating
        // // a block for DOS, reducing the amount of "free" memory available.
        // // Typically DOS + COMMAND.COM takes about 64-128KB.
        // // We'll allocate 4096 paragraphs (64KB) for DOS.
        // const DOS_PARAGRAPHS: u16 = 4096; // 64KB for DOS kernel and COMMAND.COM
        // match self.bios.memory_allocate(DOS_PARAGRAPHS) {
        //     Ok(seg) => {
        //         log::info!(
        //             "Pre-allocated {} KB at segment 0x{:04X} for DOS kernel",
        //             (DOS_PARAGRAPHS as u32 * 16) / 1024,
        //             seg
        //         );
        //     }
        //     Err((error_code, available)) => {
        //         log::warn!(
        //             "Failed to pre-allocate DOS memory: error {}, available {} paragraphs",
        //             error_code,
        //             available
        //         );
        //     }
        // }

        // // Store boot drive for reset/reboot operations
        // self.boot_drive = Some(drive);

        // // Clear loaded_program since we're booting, not loading
        // self.loaded_program = None;

        Ok(())
    }

    pub fn push_key_press(&mut self, scan_code: u8) {
        log::debug!(
            "pushing key 0x{scan_code:02X} '{}'",
            byte_to_printable_char(scan_code_to_ascii(scan_code, false, false, false))
        );
        self.key_presses.push_back(scan_code);
        self.process_key_presses();
    }

    fn process_key_presses(&mut self) {
        // Gate on output_buffer_full (obf) rather than pending_key.
        // pending_key is cleared by the PIC as soon as it dispatches the IRQ,
        // but the BIOS INT 09h handler hasn't read port 0x60 yet at that point.
        // If we load the next scan code then, we overwrite the previous one
        // before INT 09h can read it, so it never reaches the BDA buffer.
        // obf stays true until port 0x60 is actually read, which is the right gate.
        let obf = self.bus.keyboard_controller().output_buffer_full();
        if !obf && let Some(scan_code) = self.key_presses.pop_front() {
            {
                let mut keyboard_controller = self.bus.keyboard_controller_mut();
                keyboard_controller.key_press(scan_code);
            }
            self.cpu.key_press(&mut self.bus);
        }
    }

    #[cfg(test)]
    pub(crate) fn read_hard_disk_sectors(
        &self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        self.bus
            .hard_disk_controller()
            .get_disk(drive)
            .ok_or(DiskError::DriveNotReady)?
            .read_sectors(cylinder, head, sector, count)
    }

    pub fn get_exit_code(&self) -> Option<u8> {
        self.cpu.get_exit_code()
    }

    pub fn wait_for_key_press(&self) -> bool {
        self.cpu.wait_for_key_press()
    }

    #[cfg(test)]
    pub fn set_com_port_device(
        &mut self,
        port: u8,
        device: Option<Arc<RwLock<dyn ComPortDevice>>>,
    ) {
        self.bus.uart_mut().set_com_port_device(port, device)
    }

    pub fn set_exec_logging_enabled(&mut self, enabled: bool) {
        self.cpu.exec_logging_enabled = enabled;
    }

    pub fn exec_logging_enabled(&self) -> bool {
        self.cpu.exec_logging_enabled
    }
}

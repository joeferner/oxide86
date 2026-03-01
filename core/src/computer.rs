use crate::{
    Device, KeyPress,
    bus::Bus,
    cpu::Cpu,
    disk::{DriveNumber, disk_read_sectors},
    memory::Memory,
    physical_address,
};
use anyhow::{Result, anyhow};

pub struct Computer {
    cpu: Cpu,
    bus: Bus,
}

impl Computer {
    pub fn new(cpu: Cpu, memory: Memory) -> Self {
        let mut computer = Self {
            cpu,
            bus: Bus::new(memory),
        };
        computer.reset();
        computer
    }

    pub fn add_device<T: Device + 'static>(&mut self, device: T) {
        self.bus.add_device(device);
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = physical_address(segment, offset);
        self.bus.load_at(physical_addr, program_data)?;
        self.cpu.reset(segment, offset, None);
        Ok(())
    }

    pub fn run(&mut self) {
        while !self.cpu.is_halted() {
            self.step();
        }
    }

    pub fn step(&mut self) {
        self.cpu.step(&mut self.bus);
    }

    pub fn is_halted(&self) -> bool {
        self.cpu.is_halted()
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
        // Read boot sector using BIOS disk services
        // Boot sector is at cylinder 0, head 0, sector 1
        let boot_sector = disk_read_sectors(&self.bus, drive, 0, 0, 1, 1)
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

    pub fn push_keyboard_key(&mut self, key: KeyPress) {
        self.bus.keyboard_controller_mut().push_key_press(key);
    }
}

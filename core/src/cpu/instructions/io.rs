use crate::Bus;
use crate::cpu::Cpu;
use crate::io::IoDevice;

impl Cpu {
    /// IN AL, imm8 (0xE4)
    /// Read byte from immediate 8-bit port address to AL
    pub(in crate::cpu) fn in_al_imm8(
        &mut self,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.fetch_byte(bus) as u16;
        let value = match port {
            // Primary ATA channel registers (0x1F0-0x1F7)
            0x1F0 => bios.ata_read_u8(0),
            0x1F1..=0x1F7 => bios.ata_read_u8((port - 0x1F0) as u8),
            // Primary ATA alternate status (0x3F6-0x3F7)
            0x3F6 | 0x3F7 => bios.ata_read_alt_status(),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => bios.serial_io_read(0, port - 0x3F8),
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => bios.serial_io_read(1, port - 0x2F8),
            // Secondary ATA channel (0x170-0x177, 0x376) — not implemented, return 0x7F (not busy)
            0x170..=0x177 | 0x376 => 0x7F,
            // Other ports use existing io_device
            _ => io_device.read_byte(port),
        };
        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// IN AX, imm8 (0xE5)
    /// Read word from immediate 8-bit port address to AX
    pub(in crate::cpu) fn in_ax_imm8(
        &mut self,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.fetch_byte(bus) as u16;
        let value = match port {
            // Primary ATA data port word read (0x1F0)
            0x1F0 => bios.ata_read_u16(),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => {
                let low = bios.serial_io_read(0, port - 0x3F8);
                let high = bios.serial_io_read(0, port - 0x3F8 + 1);
                u16::from_le_bytes([low, high])
            }
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => {
                let low = bios.serial_io_read(1, port - 0x2F8);
                let high = bios.serial_io_read(1, port - 0x2F8 + 1);
                u16::from_le_bytes([low, high])
            }
            // Other ports use existing io_device
            _ => io_device.read_word(port),
        };
        self.ax = value;
    }

    /// OUT imm8, AL (0xE6)
    /// Write byte from AL to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_al(
        &mut self,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.fetch_byte(bus) as u16;
        let value = (self.ax & 0xFF) as u8;
        match port {
            // Primary ATA channel registers (0x1F0-0x1F7)
            0x1F0 => bios.ata_write_u8(0, value),
            0x1F1..=0x1F7 => bios.ata_write_u8((port - 0x1F0) as u8, value),
            // Primary ATA device control (0x3F6)
            0x3F6 => bios.ata_write_control(value),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => bios.serial_io_write(0, port - 0x3F8, value),
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => bios.serial_io_write(1, port - 0x2F8, value),
            // Secondary ATA (0x170-0x177, 0x376) — silently ignore
            0x170..=0x177 | 0x376 => {}
            // Other ports use existing io_device
            _ => io_device.write_byte(port, value, bus.video_mut()),
        }
    }

    /// OUT imm8, AX (0xE7)
    /// Write word from AX to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_ax(
        &mut self,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.fetch_byte(bus) as u16;
        match port {
            // Primary ATA data port word write (0x1F0)
            0x1F0 => bios.ata_write_u16(self.ax),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => {
                let bytes = self.ax.to_le_bytes();
                bios.serial_io_write(0, port - 0x3F8, bytes[0]);
                bios.serial_io_write(0, port - 0x3F8 + 1, bytes[1]);
            }
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => {
                let bytes = self.ax.to_le_bytes();
                bios.serial_io_write(1, port - 0x2F8, bytes[0]);
                bios.serial_io_write(1, port - 0x2F8 + 1, bytes[1]);
            }
            // Other ports use existing io_device
            _ => io_device.write_word(port, self.ax, bus.video_mut()),
        }
    }

    /// IN AL, DX (0xEC)
    /// Read byte from port address in DX to AL
    pub(in crate::cpu) fn in_al_dx(
        &mut self,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.dx;
        let value = match port {
            // Primary ATA channel registers (0x1F0-0x1F7)
            0x1F0 => bios.ata_read_u8(0),
            0x1F1..=0x1F7 => bios.ata_read_u8((port - 0x1F0) as u8),
            // Primary ATA alternate status (0x3F6-0x3F7)
            0x3F6 | 0x3F7 => bios.ata_read_alt_status(),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => bios.serial_io_read(0, port - 0x3F8),
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => bios.serial_io_read(1, port - 0x2F8),
            // Secondary ATA channel (0x170-0x177, 0x376) — not implemented, return 0x7F (not busy)
            0x170..=0x177 | 0x376 => 0x7F,
            // Other ports use existing io_device
            _ => io_device.read_byte(port),
        };
        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// IN AX, DX (0xED)
    /// Read word from port address in DX to AX
    pub(in crate::cpu) fn in_ax_dx(
        &mut self,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.dx;
        let value = match port {
            // Primary ATA data port word read (0x1F0)
            0x1F0 => bios.ata_read_u16(),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => {
                let low = bios.serial_io_read(0, port - 0x3F8);
                let high = bios.serial_io_read(0, port - 0x3F8 + 1);
                u16::from_le_bytes([low, high])
            }
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => {
                let low = bios.serial_io_read(1, port - 0x2F8);
                let high = bios.serial_io_read(1, port - 0x2F8 + 1);
                u16::from_le_bytes([low, high])
            }
            // Other ports use existing io_device
            _ => io_device.read_word(port),
        };
        self.ax = value;
    }

    /// OUT DX, AL (0xEE)
    /// Write byte from AL to port address in DX
    pub(in crate::cpu) fn out_dx_al(
        &mut self,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.dx;
        let value = (self.ax & 0xFF) as u8;
        match port {
            // Primary ATA channel registers (0x1F0-0x1F7)
            0x1F0 => bios.ata_write_u8(0, value),
            0x1F1..=0x1F7 => bios.ata_write_u8((port - 0x1F0) as u8, value),
            // Primary ATA device control (0x3F6)
            0x3F6 => bios.ata_write_control(value),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => bios.serial_io_write(0, port - 0x3F8, value),
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => bios.serial_io_write(1, port - 0x2F8, value),
            // Secondary ATA (0x170-0x177, 0x376) — silently ignore
            0x170..=0x177 | 0x376 => {}
            // Other ports use existing io_device
            _ => io_device.write_byte(port, value, bus.video_mut()),
        }
    }

    /// OUT DX, AX (0xEF)
    /// Write word from AX to port address in DX
    pub(in crate::cpu) fn out_dx_ax(
        &mut self,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.dx;
        match port {
            // Primary ATA data port word write (0x1F0)
            0x1F0 => bios.ata_write_u16(self.ax),
            // COM1 registers (0x3F8-0x3FF)
            0x3F8..=0x3FF => {
                let bytes = self.ax.to_le_bytes();
                bios.serial_io_write(0, port - 0x3F8, bytes[0]);
                bios.serial_io_write(0, port - 0x3F8 + 1, bytes[1]);
            }
            // COM2 registers (0x2F8-0x2FF)
            0x2F8..=0x2FF => {
                let bytes = self.ax.to_le_bytes();
                bios.serial_io_write(1, port - 0x2F8, bytes[0]);
                bios.serial_io_write(1, port - 0x2F8 + 1, bytes[1]);
            }
            // Other ports use existing io_device
            _ => io_device.write_word(port, self.ax, bus.video_mut()),
        }
    }
}

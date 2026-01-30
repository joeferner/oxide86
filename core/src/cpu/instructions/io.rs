use crate::cpu::Cpu;
use crate::io::IoDevice;
use crate::memory::Memory;

impl Cpu {
    /// IN AL, imm8 (0xE4)
    /// Read byte from immediate 8-bit port address to AL
    pub(in crate::cpu) fn in_al_imm8(&mut self, memory: &Memory, io_device: &mut IoDevice) {
        let port = self.fetch_byte(memory) as u16;
        let value = io_device.read_byte(port);
        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// IN AX, imm8 (0xE5)
    /// Read word from immediate 8-bit port address to AX
    pub(in crate::cpu) fn in_ax_imm8(&mut self, memory: &Memory, io_device: &mut IoDevice) {
        let port = self.fetch_byte(memory) as u16;
        let value = io_device.read_word(port);
        self.ax = value;
    }

    /// OUT imm8, AL (0xE6)
    /// Write byte from AL to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_al(&mut self, memory: &Memory, io_device: &mut IoDevice) {
        let port = self.fetch_byte(memory) as u16;
        let value = (self.ax & 0xFF) as u8;
        io_device.write_byte(port, value);
    }

    /// OUT imm8, AX (0xE7)
    /// Write word from AX to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_ax(&mut self, memory: &Memory, io_device: &mut IoDevice) {
        let port = self.fetch_byte(memory) as u16;
        io_device.write_word(port, self.ax);
    }

    /// IN AL, DX (0xEC)
    /// Read byte from port address in DX to AL
    pub(in crate::cpu) fn in_al_dx(&mut self, io_device: &mut IoDevice) {
        let port = self.dx;
        let value = io_device.read_byte(port);
        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// IN AX, DX (0xED)
    /// Read word from port address in DX to AX
    pub(in crate::cpu) fn in_ax_dx(&mut self, io_device: &mut IoDevice) {
        let port = self.dx;
        let value = io_device.read_word(port);
        self.ax = value;
    }

    /// OUT DX, AL (0xEE)
    /// Write byte from AL to port address in DX
    pub(in crate::cpu) fn out_dx_al(&mut self, io_device: &mut IoDevice) {
        let port = self.dx;
        let value = (self.ax & 0xFF) as u8;
        io_device.write_byte(port, value);
    }

    /// OUT DX, AX (0xEF)
    /// Write word from AX to port address in DX
    pub(in crate::cpu) fn out_dx_ax(&mut self, io_device: &mut IoDevice) {
        let port = self.dx;
        io_device.write_word(port, self.ax);
    }
}

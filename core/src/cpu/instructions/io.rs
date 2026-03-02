use crate::{bus::Bus, cpu::Cpu};

impl Cpu {
    /// OUT imm8, AL (0xE6)
    /// Write byte from AL to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_al(&mut self, bus: &mut Bus) {
        let port = self.fetch_byte(bus) as u16;
        let value = (self.ax & 0xFF) as u8;
        bus.io_write_u8(port, value);
    }

    /// IN AL, imm8 (0xE4)
    /// Read byte from immediate 8-bit port address to AL
    pub(in crate::cpu) fn in_al_imm8(&mut self, bus: &mut Bus) {
        let port = self.fetch_byte(bus) as u16;
        let value = bus.io_read_u8(port);

        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// IN AL, DX (0xEC)
    /// Read byte from port address in DX to AL
    pub(in crate::cpu) fn in_al_dx(&mut self, bus: &mut Bus) {
        let port = self.dx;
        let value = bus.io_read_u8(port);

        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// OUT DX, AL (0xEE)
    /// Write byte from AL to port address in DX
    pub(in crate::cpu) fn out_dx_al(&mut self, bus: &mut Bus) {
        let port = self.dx;
        let value = (self.ax & 0xFF) as u8;
        bus.io_write_u8(port, value);
    }
}

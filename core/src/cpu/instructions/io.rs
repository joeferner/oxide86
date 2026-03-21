use crate::{
    bus::Bus,
    cpu::{Cpu, timing},
};

impl Cpu {
    /// OUT imm8, AL (0xE6)
    /// Write byte from AL to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_al(&mut self, bus: &mut Bus) {
        let port = self.fetch_byte(bus) as u16;
        let value = (self.ax & 0xFF) as u8;
        bus.increment_cycle_count(timing::cycles::OUT_IMM);
        bus.io_write_u8(port, value);
    }

    /// OUT imm8, AX (0xE7)
    /// Write word from AX to immediate 8-bit port address
    pub(in crate::cpu) fn out_imm8_ax(&mut self, bus: &mut Bus) {
        let port = self.fetch_byte(bus) as u16;
        let value = self.ax;
        bus.increment_cycle_count(timing::cycles::OUT_IMM);
        bus.io_write_u16(port, value);
    }

    /// IN AL, imm8 (0xE4)
    /// Read byte from immediate 8-bit port address to AL
    pub(in crate::cpu) fn in_al_imm8(&mut self, bus: &mut Bus) {
        let port = self.fetch_byte(bus) as u16;
        bus.increment_cycle_count(timing::cycles::IN_IMM);
        let value = bus.io_read_u8(port);

        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// IN AX, imm8 (0xE5)
    /// Read word from immediate 8-bit port address to AX
    pub(in crate::cpu) fn in_ax_imm8(&mut self, bus: &mut Bus) {
        let port = self.fetch_byte(bus) as u16;
        bus.increment_cycle_count(timing::cycles::IN_IMM);
        self.ax = bus.io_read_u16(port);
    }

    /// IN AL, DX (0xEC)
    /// Read byte from port address in DX to AL
    pub(in crate::cpu) fn in_al_dx(&mut self, bus: &mut Bus) {
        let port = self.dx;
        bus.increment_cycle_count(timing::cycles::IN_DX);
        let value = bus.io_read_u8(port);

        // Set AL (low byte of AX)
        self.ax = (self.ax & 0xFF00) | (value as u16);
    }

    /// OUT DX, AL (0xEE)
    /// Write byte from AL to port address in DX
    pub(in crate::cpu) fn out_dx_al(&mut self, bus: &mut Bus) {
        let port = self.dx;
        let value = (self.ax & 0xFF) as u8;
        bus.increment_cycle_count(timing::cycles::OUT_DX);
        bus.io_write_u8(port, value);
    }

    /// IN AX, DX (0xED)
    /// Read word from port address in DX to AX
    pub(in crate::cpu) fn in_ax_dx(&mut self, bus: &mut Bus) {
        let port = self.dx;
        bus.increment_cycle_count(timing::cycles::IN_DX);
        self.ax = bus.io_read_u16(port);
    }

    /// OUT DX, AX (0xEF)
    /// Write word from AX to port address in DX
    pub(in crate::cpu) fn out_dx_ax(&mut self, bus: &mut Bus) {
        let port = self.dx;
        let value = self.ax;
        bus.increment_cycle_count(timing::cycles::OUT_DX);
        bus.io_write_u16(port, value);
    }
}

use super::super::{Cpu, RepeatPrefix, timing};
use crate::Bus;
use crate::cpu::bios::Bios;
use crate::cpu::cpu_flag;
use crate::io::IoDevice;

impl Cpu {
    /// OUTS - Output String to Port (opcodes 6E-6F)
    /// 6E: OUTSB - Output byte from DS:SI to port DX
    /// 6F: OUTSW - Output word from DS:SI to port DX
    ///
    /// Writes data from DS:SI to I/O port DX, then increments/decrements SI based on DF.
    pub(in crate::cpu) fn outs(
        &mut self,
        opcode: u8,
        bus: &mut Bus,
        bios: &mut Bios,
        io_device: &mut IoDevice,
    ) {
        let is_word = opcode & 0x01 != 0;

        // Handle repeat prefix
        if self.repeat_prefix.is_some() {
            while self.cx != 0 {
                self.outs_once(is_word, bus, bios, io_device);
                self.cx = self.cx.wrapping_sub(1);
            }
        } else {
            self.outs_once(is_word, bus, bios, io_device);
        }
    }

    fn outs_once(
        &mut self,
        is_word: bool,
        bus: &mut Bus,
        bios: &mut Bios,
        io_device: &mut IoDevice,
    ) {
        let port = self.dx;

        if is_word {
            // OUTSW - Output word; route ATA data port through the ATA handler
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = Self::physical_address(src_seg, self.si);
            let value = bus.read_u16(addr);
            if port == 0x1F0 {
                bios.ata_write_u16(value);
            } else {
                io_device.write_word(port, value, bus.video_mut());
            }

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(2);
            } else {
                self.si = self.si.wrapping_add(2);
            }
        } else {
            // OUTSB - Output byte
            let src_seg = self.segment_override.unwrap_or(self.ds);
            let addr = Self::physical_address(src_seg, self.si);
            let value = bus.read_u8(addr);
            io_device.write_byte(port, value, bus.video_mut());

            // Update SI based on direction flag
            if self.get_flag(cpu_flag::DIRECTION) {
                self.si = self.si.wrapping_sub(1);
            } else {
                self.si = self.si.wrapping_add(1);
            }
        }
    }    
}

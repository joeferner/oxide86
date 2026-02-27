use crate::{cpu::Cpu, memory_bus::MemoryBus, physical_address};

mod control_flow;
mod data_transfer;

impl Cpu {
    // Decode ModR/M byte and calculate effective address
    // Returns (mod, reg, r/m, effective_address, default_segment)
    // mod: 00=no disp (except r/m=110), 01=8-bit disp, 10=16-bit disp, 11=register
    // For mod=11, effective_address is unused
    fn decode_modrm(&mut self, modrm: u8, bus: &MemoryBus) -> (u8, u8, u8, usize, u16) {
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        if mode == 0b11 {
            // Register mode - no memory access
            return (mode, reg, rm, 0, self.ds);
        }

        // Calculate base address from r/m field
        let (base_addr, default_seg) = match rm {
            0b000 => (self.bx.wrapping_add(self.si), self.ds), // [BX + SI]
            0b001 => (self.bx.wrapping_add(self.di), self.ds), // [BX + DI]
            0b010 => (self.bp.wrapping_add(self.si), self.ss), // [BP + SI]
            0b011 => (self.bp.wrapping_add(self.di), self.ss), // [BP + DI]
            0b100 => (self.si, self.ds),                       // [SI]
            0b101 => (self.di, self.ds),                       // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address (16-bit displacement, no base)
                    let disp = self.fetch_word(bus);
                    let seg = self.segment_override.unwrap_or(self.ds);
                    return (mode, reg, rm, physical_address(seg, disp), seg);
                } else {
                    (self.bp, self.ss) // [BP]
                }
            }
            0b111 => (self.bx, self.ds), // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => base_addr, // No displacement
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte(bus) as i8;
                base_addr.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(bus);
                base_addr.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        // Use segment override if present, otherwise use default segment
        let effective_seg = self.segment_override.unwrap_or(default_seg);
        let effective_addr = physical_address(effective_seg, effective_offset);
        (mode, reg, rm, effective_addr, effective_seg)
    }

    // Read 16-bit value from register or memory based on mod field
    fn read_rm16(&self, mode: u8, rm: u8, addr: usize, bus: &MemoryBus) -> u16 {
        if mode == 0b11 {
            // Register mode
            self.get_reg16(rm)
        } else {
            // Memory mode
            bus.read_u16(addr)
        }
    }

    /// Set 8-bit register
    fn set_reg8(&mut self, reg: u8, value: u8) {
        match reg {
            0 => self.ax = (self.ax & 0xFF00) | value as u16, // AL
            1 => self.cx = (self.cx & 0xFF00) | value as u16, // CL
            2 => self.dx = (self.dx & 0xFF00) | value as u16, // DL
            3 => self.bx = (self.bx & 0xFF00) | value as u16, // BL
            4 => self.ax = (self.ax & 0x00FF) | ((value as u16) << 8), // AH
            5 => self.cx = (self.cx & 0x00FF) | ((value as u16) << 8), // CH
            6 => self.dx = (self.dx & 0x00FF) | ((value as u16) << 8), // DH
            7 => self.bx = (self.bx & 0x00FF) | ((value as u16) << 8), // BH
            _ => unreachable!(),
        }
    }

    /// Set 16-bit register
    fn set_reg16(&mut self, reg: u8, value: u16) {
        match reg & 0x07 {
            0 => self.ax = value,
            1 => self.cx = value,
            2 => self.dx = value,
            3 => self.bx = value,
            4 => self.sp = value,
            5 => self.bp = value,
            6 => self.si = value,
            7 => self.di = value,
            _ => unreachable!(),
        }
    }

    /// Get 8-bit register value
    #[allow(dead_code)]
    fn get_reg8(&self, reg: u8) -> u8 {
        match reg {
            0 => (self.ax & 0xFF) as u8, // AL
            1 => (self.cx & 0xFF) as u8, // CL
            2 => (self.dx & 0xFF) as u8, // DL
            3 => (self.bx & 0xFF) as u8, // BL
            4 => (self.ax >> 8) as u8,   // AH
            5 => (self.cx >> 8) as u8,   // CH
            6 => (self.dx >> 8) as u8,   // DH
            7 => (self.bx >> 8) as u8,   // BH
            _ => unreachable!(),
        }
    }

    /// Get 16-bit register value
    fn get_reg16(&self, reg: u8) -> u16 {
        match reg & 0x07 {
            0 => self.ax,
            1 => self.cx,
            2 => self.dx,
            3 => self.bx,
            4 => self.sp,
            5 => self.bp,
            6 => self.si,
            7 => self.di,
            _ => unreachable!(),
        }
    }

    /// Set segment register value
    fn set_segreg(&mut self, reg: u8, value: u16) {
        match reg & 0x03 {
            0 => self.es = value,
            1 => self.cs = value,
            2 => self.ss = value,
            3 => self.ds = value,
            _ => unreachable!(),
        }
    }

    // Push 16-bit value onto stack
    fn push(&mut self, value: u16, bus: &mut MemoryBus) {
        self.sp = self.sp.wrapping_sub(2);
        let addr = physical_address(self.ss, self.sp);
        bus.write_u16(addr, value);
    }

    // Pop 16-bit value from stack
    fn pop(&mut self, bus: &MemoryBus) -> u16 {
        let addr = physical_address(self.ss, self.sp);
        let value = bus.read_u16(addr);
        self.sp = self.sp.wrapping_add(2);
        value
    }
}

use crate::{
    bus::Bus,
    cpu::{Cpu, timing},
};

impl Cpu {
    /// ESC - Escape to coprocessor (opcodes D8-DF)
    /// Passes instruction to 8087 FPU. Without a coprocessor, this is a NOP
    /// that reads the ModR/M byte and any displacement to maintain bus timing.
    /// With a coprocessor, dispatches the subset of 8087 instructions implemented.
    pub(in crate::cpu) fn esc(&mut self, opcode: u8, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if self.math_coprocessor {
            if mode == 0b11 {
                match (opcode, reg, rm) {
                    // FNINIT (DB /4 rm=3 → DB E3)
                    (0xDB, 4, 3) => {
                        self.fpu_status_word = 0x0000;
                        self.fpu_top = 0;
                        self.fpu_stack = [0.0_f64; 8];
                    }
                    // FNSTSW AX (DF /4 rm=0 → DF E0)
                    (0xDF, 4, 0) => self.ax = self.fpu_status_word,
                    // FLD ST(i) (D9 /0 rm=i → D9 C0+i)
                    (0xD9, 0, i) => {
                        let val = self.fpu_stack[self.fpu_top.wrapping_add(i) as usize & 7];
                        self.fpu_top = self.fpu_top.wrapping_sub(1) & 7;
                        self.fpu_stack[self.fpu_top as usize] = val;
                    }
                    _ => log::warn!(
                        "unimplemented FPU register instruction: opcode={:#04X} reg={} rm={}",
                        opcode,
                        reg,
                        rm
                    ),
                }
            } else {
                match (opcode, reg) {
                    // FLD m32 (D9 /0)
                    (0xD9, 0) => {
                        let bits = (bus.memory_read_u16(addr) as u32)
                            | ((bus.memory_read_u16(addr + 2) as u32) << 16);
                        let val = f32::from_bits(bits) as f64;
                        self.fpu_top = self.fpu_top.wrapping_sub(1) & 7;
                        self.fpu_stack[self.fpu_top as usize] = val;
                    }
                    // FLD m64 (DD /0)
                    (0xDD, 0) => {
                        let w0 = bus.memory_read_u16(addr) as u64;
                        let w1 = bus.memory_read_u16(addr + 2) as u64;
                        let w2 = bus.memory_read_u16(addr + 4) as u64;
                        let w3 = bus.memory_read_u16(addr + 6) as u64;
                        let val = f64::from_bits(w0 | (w1 << 16) | (w2 << 32) | (w3 << 48));
                        self.fpu_top = self.fpu_top.wrapping_sub(1) & 7;
                        self.fpu_stack[self.fpu_top as usize] = val;
                    }
                    // FST m32 (D9 /2)
                    (0xD9, 2) => {
                        let bits = (self.fpu_stack[self.fpu_top as usize] as f32).to_bits();
                        bus.memory_write_u32(addr, bits);
                    }
                    // FST m64 (DD /2)
                    (0xDD, 2) => {
                        let bits = self.fpu_stack[self.fpu_top as usize].to_bits();
                        bus.memory_write_u32(addr, bits as u32);
                        bus.memory_write_u32(addr + 4, (bits >> 32) as u32);
                    }
                    // FSTP m32 (D9 /3)
                    (0xD9, 3) => {
                        let bits = (self.fpu_stack[self.fpu_top as usize] as f32).to_bits();
                        bus.memory_write_u32(addr, bits);
                        self.fpu_top = self.fpu_top.wrapping_add(1) & 7;
                    }
                    // FSTP m64 (DD /3)
                    (0xDD, 3) => {
                        let bits = self.fpu_stack[self.fpu_top as usize].to_bits();
                        bus.memory_write_u32(addr, bits as u32);
                        bus.memory_write_u32(addr + 4, (bits >> 32) as u32);
                        self.fpu_top = self.fpu_top.wrapping_add(1) & 7;
                    }
                    _ => log::warn!(
                        "unimplemented FPU memory instruction: opcode={:#04X} reg={}",
                        opcode,
                        reg
                    ),
                }
            }
        }

        bus.increment_cycle_count(timing::cycles::ESC)
    }
}

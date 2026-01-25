use super::super::{Cpu, FLAG_CARRY, FLAG_OVERFLOW, FLAG_ZERO, FLAG_SIGN, FLAG_PARITY};
use crate::memory::Memory;

impl Cpu {
    /// HLT - Halt (opcode F4)
    /// Stops instruction execution until a hardware interrupt occurs
    pub(in crate::cpu) fn hlt(&mut self) {
        self.halted = true;
    }

    /// JMP short relative (opcode EB)
    /// Jump to IP + signed 8-bit displacement
    pub(in crate::cpu) fn jmp_short(&mut self, memory: &Memory) {
        let offset = self.fetch_byte(memory) as i8;
        self.ip = self.ip.wrapping_add(offset as i16 as u16);
    }

    /// JMP near relative (opcode E9)
    /// Jump to IP + signed 16-bit displacement
    pub(in crate::cpu) fn jmp_near(&mut self, memory: &Memory) {
        let offset = self.fetch_word(memory) as i16;
        self.ip = self.ip.wrapping_add(offset as u16);
    }

    /// CALL near relative (opcode E8)
    /// Call procedure at IP + signed 16-bit offset
    pub(in crate::cpu) fn call_near(&mut self, memory: &mut Memory) {
        let offset = self.fetch_word(memory) as i16;
        // Push return address (current IP after reading offset)
        self.push(self.ip, memory);
        // Jump to target
        self.ip = self.ip.wrapping_add(offset as u16);
    }

    /// RET (opcode C3: near return, C2: near return with imm16 pop)
    pub(in crate::cpu) fn ret(&mut self, opcode: u8, memory: &mut Memory) {
        // Pop return address
        self.ip = self.pop(memory);

        // If opcode is C2, also pop additional bytes from stack
        if opcode == 0xC2 {
            let bytes_to_pop = self.fetch_word(memory);
            self.sp = self.sp.wrapping_add(bytes_to_pop);
        }
    }

    /// Conditional jumps - short relative (opcodes 70-7F)
    /// Jump to IP + signed 8-bit displacement if condition is met
    pub(in crate::cpu) fn jmp_conditional(&mut self, opcode: u8, memory: &Memory) {
        let offset = self.fetch_byte(memory) as i8;

        let condition = match opcode {
            0x70 => self.get_flag(FLAG_OVERFLOW),                    // JO - Jump if overflow
            0x71 => !self.get_flag(FLAG_OVERFLOW),                   // JNO - Jump if not overflow
            0x72 => self.get_flag(FLAG_CARRY),                       // JB/JC/JNAE - Jump if below/carry
            0x73 => !self.get_flag(FLAG_CARRY),                      // JAE/JNB/JNC - Jump if above or equal/not below/not carry
            0x74 => self.get_flag(FLAG_ZERO),                        // JE/JZ - Jump if equal/zero
            0x75 => !self.get_flag(FLAG_ZERO),                       // JNE/JNZ - Jump if not equal/not zero
            0x76 => self.get_flag(FLAG_CARRY) || self.get_flag(FLAG_ZERO),  // JBE/JNA - Jump if below or equal/not above
            0x77 => !self.get_flag(FLAG_CARRY) && !self.get_flag(FLAG_ZERO), // JA/JNBE - Jump if above/not below or equal
            0x78 => self.get_flag(FLAG_SIGN),                        // JS - Jump if sign
            0x79 => !self.get_flag(FLAG_SIGN),                       // JNS - Jump if not sign
            0x7A => self.get_flag(FLAG_PARITY),                      // JP/JPE - Jump if parity/parity even
            0x7B => !self.get_flag(FLAG_PARITY),                     // JNP/JPO - Jump if not parity/parity odd
            0x7C => self.get_flag(FLAG_SIGN) != self.get_flag(FLAG_OVERFLOW),  // JL/JNGE - Jump if less/not greater or equal
            0x7D => self.get_flag(FLAG_SIGN) == self.get_flag(FLAG_OVERFLOW),  // JGE/JNL - Jump if greater or equal/not less
            0x7E => self.get_flag(FLAG_ZERO) || (self.get_flag(FLAG_SIGN) != self.get_flag(FLAG_OVERFLOW)),  // JLE/JNG - Jump if less or equal/not greater
            0x7F => !self.get_flag(FLAG_ZERO) && (self.get_flag(FLAG_SIGN) == self.get_flag(FLAG_OVERFLOW)), // JG/JNLE - Jump if greater/not less or equal
            _ => unreachable!(),
        };

        if condition {
            self.ip = self.ip.wrapping_add(offset as i16 as u16);
        }
    }
}

use crate::{
    cpu::{Cpu, timing},
    memory_bus::MemoryBus,
    physical_address,
};

impl Cpu {
    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(in crate::cpu) fn mov_imm_to_reg(&mut self, opcode: u8, memory_bus: &MemoryBus) {
        let reg = opcode & 0x07;
        let is_word = opcode & 0x08 != 0;

        if is_word {
            // 16-bit register
            let value = self.fetch_word(memory_bus);
            self.set_reg16(reg, value);
        } else {
            // 8-bit register
            let value = self.fetch_byte(memory_bus);
            self.set_reg8(reg, value);
        }

        // MOV immediate to register: 4 cycles
        self.last_instruction_cycles = timing::cycles::MOV_IMM_REG;
    }

    /// MOV accumulator to/from direct bus offset (opcodes A0-A3)
    /// A0: MOV AL, [moffs8] - Move byte at direct address to AL
    /// A1: MOV AX, [moffs16] - Move word at direct address to AX
    /// A2: MOV [moffs8], AL - Move AL to byte at direct address
    /// A3: MOV [moffs16], AX - Move AX to word at direct address
    pub(in crate::cpu) fn mov_acc_moffs(&mut self, opcode: u8, bus: &mut MemoryBus) {
        let is_word = opcode & 0x01 != 0;
        let to_acc = opcode & 0x02 == 0; // 0 = to accumulator, 1 = from accumulator

        // Fetch the direct bus offset (16-bit address)
        let offset = self.fetch_word(bus);
        // Use segment override if present, otherwise use DS
        let segment = self.segment_override.unwrap_or(self.ds);
        let addr = physical_address(segment, offset);

        if is_word {
            if to_acc {
                // MOV AX, [offset]
                self.ax = bus.read_u16(addr);
            } else {
                // MOV [offset], AX
                bus.write_u16(addr, self.ax);
            }
        } else if to_acc {
            // MOV AL, [offset]
            let value = bus.read_u8(addr);
            self.ax = (self.ax & 0xFF00) | (value as u16);
        } else {
            // MOV [offset], AL
            let value = (self.ax & 0xFF) as u8;
            bus.write_u8(addr, value);
        }

        // MOV acc, [addr] or [addr], acc: 10 cycles (direct addressing)
        self.last_instruction_cycles = if to_acc {
            timing::cycles::MOV_MEM_ACC
        } else {
            timing::cycles::MOV_ACC_MEM
        };
    }

    /// MOV r/m16 to segment register (opcode 8E)
    /// 8E: MOV segreg, r/m16
    /// Copies a 16-bit register or bus value to a segment register (ES, CS, SS, DS)
    /// Note: MOV to CS is not recommended as it affects instruction fetching
    pub(in crate::cpu) fn mov_rm_to_segreg(&mut self, bus: &mut MemoryBus) {
        let modrm = self.fetch_byte(bus);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.read_rm16(mode, rm, addr, bus);
        self.set_segreg(seg_reg, value);

        // Calculate cycle timing
        self.last_instruction_cycles = if mode == 0b11 {
            // MOV segreg, reg: 2 cycles
            timing::cycles::MOV_RM_SEGREG_REG
        } else {
            // MOV segreg, mem: 8 + EA cycles
            timing::cycles::MOV_RM_SEGREG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        };
    }
}

#[cfg(test)]
mod tests {
    // Assuming these exist in your project structure
    use crate::cpu::tests::create_test_cpu;
    use crate::physical_address;

    #[test]
    #[test_log::test]
    fn test_mov_imm_to_reg_8bit() {
        // 1. Setup: Initialize CPU and Memory
        let (mut cpu, mut memory_bus) = create_test_cpu();

        // 2. Test 8-bit Move: MOV AL, 0x42 (Opcode 0xB0)
        // AL is usually register index 0
        let opcode_al = 0xB0;
        let imm_val_8 = 0x42;

        // Place the immediate value in memory at the current IP
        memory_bus.write_u8(physical_address(0, cpu.ip), imm_val_8);

        cpu.mov_imm_to_reg(opcode_al, &memory_bus);

        assert_eq!(cpu.get_reg8(0), imm_val_8, "AL should contain 0x42");
        assert_eq!(cpu.last_instruction_cycles, 4, "Should take 4 cycles");
        assert_eq!(cpu.ip, 1, "IP should have advanced by 1 bytes");
    }

    #[test]
    #[test_log::test]
    fn test_mov_imm_to_reg_16bit() {
        // 1. Setup: Initialize CPU and Memory
        let (mut cpu, mut memory_bus) = create_test_cpu();

        // 2. Test 16-bit Move: MOV AX, 0x1234 (Opcode 0xB8)
        // AX is usually register index 0 (with the is_word bit set)
        let opcode_ax = 0xB8;
        let imm_val_16 = 0x1234;

        // Place the word in memory (handling little-endian if applicable)
        memory_bus.write_u16(physical_address(0, cpu.ip), imm_val_16);

        cpu.mov_imm_to_reg(opcode_ax, &memory_bus);

        assert_eq!(cpu.get_reg16(0), imm_val_16, "AX should contain 0x1234");
        assert_eq!(cpu.ip, 2, "IP should have advanced by 2 bytes");
    }
}

//! Unit tests for 8086 CPU instructions
//!
//! These tests verify instruction behavior against known 8086 specification,
//! not just testing the implementation.

#[cfg(test)]
mod arithmetic_tests {
    use crate::cpu::{Cpu, cpu_flag};
    use crate::memory::Memory;

    /// Helper to create a CPU and memory for testing
    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_add_8bit_no_carry() {
        let (mut cpu, mut memory) = setup();
        // ADD AL, 0x05 when AL=0x03, result should be 0x08, no carry
        cpu.ax = 0x0003;
        memory.write_u8(0, 0x05); // immediate value
        cpu.add_imm_acc(0x04, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x08, "AL should be 0x08");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
        assert!(!cpu.get_flag(cpu_flag::SIGN), "Sign flag should not be set");
        assert!(
            !cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow flag should not be set"
        );
    }

    #[test]
    fn test_add_8bit_with_carry() {
        let (mut cpu, mut memory) = setup();
        // ADD AL, 0xFF when AL=0x02, result should be 0x01 with carry
        cpu.ax = 0x0002;
        memory.write_u8(0, 0xFF);
        cpu.add_imm_acc(0x04, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x01, "AL should be 0x01");
        assert!(cpu.get_flag(cpu_flag::CARRY), "Carry flag should be set");
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
    }

    #[test]
    fn test_add_8bit_zero_result() {
        let (mut cpu, mut memory) = setup();
        // ADD AL, 0x01 when AL=0xFF, result should be 0x00 with carry and zero
        cpu.ax = 0x00FF;
        memory.write_u8(0, 0x01);
        cpu.add_imm_acc(0x04, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x00, "AL should be 0x00");
        assert!(cpu.get_flag(cpu_flag::CARRY), "Carry flag should be set");
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
        assert!(!cpu.get_flag(cpu_flag::SIGN), "Sign flag should not be set");
    }

    #[test]
    fn test_add_8bit_sign() {
        let (mut cpu, mut memory) = setup();
        // ADD AL, 0x01 when AL=0x7F, result should be 0x80 (negative in signed)
        cpu.ax = 0x007F;
        memory.write_u8(0, 0x01);
        cpu.add_imm_acc(0x04, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x80, "AL should be 0x80");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
        assert!(
            cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow flag should be set (signed overflow)"
        );
    }

    #[test]
    fn test_add_8bit_auxiliary_carry() {
        let (mut cpu, mut memory) = setup();
        // ADD AL, 0x0F when AL=0x0F, should set auxiliary carry (bit 3->4 carry)
        cpu.ax = 0x000F;
        memory.write_u8(0, 0x0F);
        cpu.add_imm_acc(0x04, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x1E, "AL should be 0x1E");
        assert!(
            cpu.get_flag(cpu_flag::AUXILIARY),
            "Auxiliary carry should be set"
        );
    }

    #[test]
    fn test_add_16bit_no_carry() {
        let (mut cpu, mut memory) = setup();
        // ADD AX, 0x1234 when AX=0x0100, result should be 0x1334
        cpu.ax = 0x0100;
        memory.write_u16(0, 0x1234);
        cpu.add_imm_acc(0x05, &memory);

        assert_eq!(cpu.ax, 0x1334, "AX should be 0x1334");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }

    #[test]
    fn test_add_16bit_with_carry() {
        let (mut cpu, mut memory) = setup();
        // ADD AX, 0x0002 when AX=0xFFFF, result should be 0x0001 with carry
        cpu.ax = 0xFFFF;
        memory.write_u16(0, 0x0002);
        cpu.add_imm_acc(0x05, &memory);

        assert_eq!(cpu.ax, 0x0001, "AX should be 0x0001");
        assert!(cpu.get_flag(cpu_flag::CARRY), "Carry flag should be set");
    }

    #[test]
    fn test_sub_8bit_no_borrow() {
        let (mut cpu, mut memory) = setup();
        // SUB AL, 0x03 when AL=0x05, result should be 0x02
        cpu.ax = 0x0005;
        memory.write_u8(0, 0x03);
        cpu.sub_imm_acc(0x2C, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x02, "AL should be 0x02");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
    }

    #[test]
    fn test_sub_8bit_with_borrow() {
        let (mut cpu, mut memory) = setup();
        // SUB AL, 0x05 when AL=0x03, result should be 0xFE with borrow
        cpu.ax = 0x0003;
        memory.write_u8(0, 0x05);
        cpu.sub_imm_acc(0x2C, &memory);

        assert_eq!(cpu.ax & 0xFF, 0xFE, "AL should be 0xFE");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should be set (borrow)"
        );
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
    }

    #[test]
    fn test_sub_8bit_zero_result() {
        let (mut cpu, mut memory) = setup();
        // SUB AL, 0x42 when AL=0x42, result should be 0x00
        cpu.ax = 0x0042;
        memory.write_u8(0, 0x42);
        cpu.sub_imm_acc(0x2C, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x00, "AL should be 0x00");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
    }

    #[test]
    fn test_inc_8bit() {
        let (mut cpu, mut memory) = setup();
        // INC on memory location with value 0x42 should give 0x43
        memory.write_u8(0x1000, 0x42);
        memory.write_u8(0, 0x06); // ModR/M: mode=00, op=000 (INC), r/m=110 (direct address)
        memory.write_u16(1, 0x1000); // Direct address

        cpu.inc_dec_rm(0xFE, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x43, "Memory should be 0x43");
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
        // INC does not affect carry flag
    }

    #[test]
    fn test_inc_16bit_no_overflow() {
        let (mut cpu, _memory) = setup();
        // INC AX when AX=0x0042, result should be 0x0043
        cpu.ax = 0x0042;
        cpu.inc_reg16(0x40); // INC AX (opcode 0x40, reg=0)

        assert_eq!(cpu.ax, 0x0043, "AX should be 0x0043");
        assert!(
            !cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow should not be set"
        );
    }

    #[test]
    fn test_inc_16bit_overflow() {
        let (mut cpu, _memory) = setup();
        // INC AX when AX=0x7FFF (max positive), should overflow to 0x8000
        cpu.ax = 0x7FFF;
        cpu.inc_reg16(0x40); // INC AX

        assert_eq!(cpu.ax, 0x8000, "AX should be 0x8000");
        assert!(
            cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow flag should be set"
        );
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
    }

    #[test]
    fn test_dec_16bit() {
        let (mut cpu, _memory) = setup();
        // DEC AX when AX=0x0043, result should be 0x0042
        cpu.ax = 0x0043;
        cpu.dec_reg16(0x48); // DEC AX (opcode 0x48, reg=0)

        assert_eq!(cpu.ax, 0x0042, "AX should be 0x0042");
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
    }

    #[test]
    fn test_dec_to_zero() {
        let (mut cpu, _memory) = setup();
        // DEC AX when AX=0x0001, result should be 0x0000
        cpu.ax = 0x0001;
        cpu.dec_reg16(0x48); // DEC AX

        assert_eq!(cpu.ax, 0x0000, "AX should be 0x0000");
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
    }

    #[test]
    fn test_dec_16bit_overflow() {
        let (mut cpu, _memory) = setup();
        // DEC AX when AX=0x8000 (min negative), should overflow to 0x7FFF
        cpu.ax = 0x8000;
        cpu.dec_reg16(0x48); // DEC AX

        assert_eq!(cpu.ax, 0x7FFF, "AX should be 0x7FFF");
        assert!(
            cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow flag should be set"
        );
    }

    #[test]
    fn test_adc_8bit_no_carry_in() {
        let (mut cpu, mut memory) = setup();
        // ADC AL, 0x05 when AL=0x03, CF=0, result should be 0x08
        cpu.ax = 0x0003;
        cpu.set_flag(cpu_flag::CARRY, false);
        memory.write_u8(0, 0x05);
        cpu.adc_imm_acc(0x14, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x08, "AL should be 0x08");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }

    #[test]
    fn test_adc_8bit_with_carry_in() {
        let (mut cpu, mut memory) = setup();
        // ADC AL, 0x05 when AL=0x03, CF=1, result should be 0x09
        cpu.ax = 0x0003;
        cpu.set_flag(cpu_flag::CARRY, true);
        memory.write_u8(0, 0x05);
        cpu.adc_imm_acc(0x14, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x09, "AL should be 0x09");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }

    #[test]
    fn test_adc_8bit_double_carry() {
        let (mut cpu, mut memory) = setup();
        // ADC AL, 0xFF when AL=0xFF, CF=1, result should be 0xFF with carry out
        cpu.ax = 0x00FF;
        cpu.set_flag(cpu_flag::CARRY, true);
        memory.write_u8(0, 0xFF);
        cpu.adc_imm_acc(0x14, &memory);

        assert_eq!(cpu.ax & 0xFF, 0xFF, "AL should be 0xFF");
        assert!(cpu.get_flag(cpu_flag::CARRY), "Carry flag should be set");
    }

    #[test]
    fn test_sbb_8bit_no_borrow_in() {
        let (mut cpu, mut memory) = setup();
        // SBB AL, 0x03 when AL=0x05, CF=0, result should be 0x02
        cpu.ax = 0x0005;
        cpu.set_flag(cpu_flag::CARRY, false);
        memory.write_u8(0, 0x03);
        cpu.sbb_imm_acc(0x1C, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x02, "AL should be 0x02");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }

    #[test]
    fn test_sbb_8bit_with_borrow_in() {
        let (mut cpu, mut memory) = setup();
        // SBB AL, 0x03 when AL=0x05, CF=1, result should be 0x01
        cpu.ax = 0x0005;
        cpu.set_flag(cpu_flag::CARRY, true);
        memory.write_u8(0, 0x03);
        cpu.sbb_imm_acc(0x1C, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x01, "AL should be 0x01");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }
}

#[cfg(test)]
mod logical_tests {
    use crate::cpu::{Cpu, cpu_flag};
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_and_8bit() {
        let (mut cpu, mut memory) = setup();
        // AND AL, 0x0F when AL=0x3C, result should be 0x0C
        cpu.ax = 0x003C;
        memory.write_u8(0, 0x0F);
        cpu.and_imm_acc(0x24, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x0C, "AL should be 0x0C");
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should be cleared");
        assert!(
            !cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow should be cleared"
        );
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
    }

    #[test]
    fn test_and_zero_result() {
        let (mut cpu, mut memory) = setup();
        // AND AL, 0x00 when AL=0xFF, result should be 0x00
        cpu.ax = 0x00FF;
        memory.write_u8(0, 0x00);
        cpu.and_imm_acc(0x24, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x00, "AL should be 0x00");
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
        assert!(
            cpu.get_flag(cpu_flag::PARITY),
            "Parity should be even (all zeros)"
        );
    }

    #[test]
    fn test_or_8bit() {
        let (mut cpu, mut memory) = setup();
        // OR AL, 0x0F when AL=0x30, result should be 0x3F
        cpu.ax = 0x0030;
        memory.write_u8(0, 0x0F);
        cpu.or_imm_acc(0x0C, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x3F, "AL should be 0x3F");
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should be cleared");
        assert!(
            !cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow should be cleared"
        );
    }

    #[test]
    fn test_xor_8bit() {
        let (mut cpu, mut memory) = setup();
        // XOR AL, 0x55 when AL=0xAA, result should be 0xFF
        cpu.ax = 0x00AA;
        memory.write_u8(0, 0x55);
        cpu.xor_imm_acc(0x34, &memory);

        assert_eq!(cpu.ax & 0xFF, 0xFF, "AL should be 0xFF");
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should be cleared");
        assert!(
            !cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow should be cleared"
        );
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
    }

    #[test]
    fn test_xor_self_to_zero() {
        let (mut cpu, mut memory) = setup();
        // XOR AL, AL when AL=0x42, result should be 0x00 (common zero idiom)
        cpu.ax = 0x0042;
        memory.write_u8(0, 0x42);
        cpu.xor_imm_acc(0x34, &memory);

        assert_eq!(cpu.ax & 0xFF, 0x00, "AL should be 0x00");
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
        assert!(cpu.get_flag(cpu_flag::PARITY), "Parity should be even");
    }

    #[test]
    fn test_test_8bit() {
        let (mut cpu, mut memory) = setup();
        // TEST AL, 0x80 when AL=0xFF, should set sign flag
        cpu.ax = 0x00FF;
        memory.write_u8(0, 0x80);
        cpu.test_imm_acc(0xA8, &memory);

        // TEST doesn't modify AL
        assert_eq!(cpu.ax & 0xFF, 0xFF, "AL should remain 0xFF");
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
    }

    #[test]
    fn test_test_zero_result() {
        let (mut cpu, mut memory) = setup();
        // TEST AL, 0x00 when AL=0xFF, result is 0x00
        cpu.ax = 0x00FF;
        memory.write_u8(0, 0x00);
        cpu.test_imm_acc(0xA8, &memory);

        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should be cleared");
    }

    #[test]
    fn test_not_8bit() {
        let (mut cpu, mut memory) = setup();
        // NOT on 0x0F should give 0xF0
        memory.write_u8(0x1000, 0x0F);
        memory.write_u8(0, 0x16); // ModR/M: mode=00, op=010 (NOT), r/m=110
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0xF0, "Memory should be 0xF0");
        // NOT doesn't affect flags
    }

    #[test]
    fn test_neg_8bit_positive() {
        let (mut cpu, mut memory) = setup();
        // NEG on 0x01 should give 0xFF (-1 in two's complement)
        memory.write_u8(0x1000, 0x01);
        memory.write_u8(0, 0x1E); // ModR/M: mode=00, op=011 (NEG), r/m=110
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0xFF, "Memory should be 0xFF");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry should be set (value was non-zero)"
        );
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
    }

    #[test]
    fn test_neg_8bit_zero() {
        let (mut cpu, mut memory) = setup();
        // NEG on 0x00 should remain 0x00
        memory.write_u8(0x1000, 0x00);
        memory.write_u8(0, 0x1E); // ModR/M: op=011 (NEG)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x00, "Memory should be 0x00");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry should not be set (value was zero)"
        );
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
    }

    #[test]
    fn test_clc_stc_cmc() {
        let (mut cpu, _memory) = setup();

        // Test CLC - Clear Carry
        cpu.set_flag(cpu_flag::CARRY, true);
        cpu.clc();
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "CLC should clear carry flag"
        );

        // Test STC - Set Carry
        cpu.stc();
        assert!(cpu.get_flag(cpu_flag::CARRY), "STC should set carry flag");

        // Test CMC - Complement Carry
        cpu.cmc();
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "CMC should complement carry flag"
        );
        cpu.cmc();
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "CMC should complement carry flag again"
        );
    }
}

#[cfg(test)]
mod comparison_tests {
    use crate::cpu::{Cpu, cpu_flag};
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_cmp_8bit_equal() {
        let (mut cpu, mut memory) = setup();
        // CMP AL, 0x42 when AL=0x42, should set zero flag
        cpu.ax = 0x0042;
        memory.write_u8(0, 0x42);
        cpu.cmp_imm_acc(0x3C, &memory);

        // CMP doesn't modify AL
        assert_eq!(cpu.ax & 0xFF, 0x42, "AL should remain 0x42");
        assert!(
            cpu.get_flag(cpu_flag::ZERO),
            "Zero flag should be set (equal)"
        );
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }

    #[test]
    fn test_cmp_8bit_less_unsigned() {
        let (mut cpu, mut memory) = setup();
        // CMP AL, 0x50 when AL=0x30, 0x30 < 0x50 (unsigned)
        cpu.ax = 0x0030;
        memory.write_u8(0, 0x50);
        cpu.cmp_imm_acc(0x3C, &memory);

        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should be set (below)"
        );
    }

    #[test]
    fn test_cmp_8bit_greater_unsigned() {
        let (mut cpu, mut memory) = setup();
        // CMP AL, 0x30 when AL=0x50, 0x50 > 0x30 (unsigned)
        cpu.ax = 0x0050;
        memory.write_u8(0, 0x30);
        cpu.cmp_imm_acc(0x3C, &memory);

        assert!(!cpu.get_flag(cpu_flag::ZERO), "Zero flag should not be set");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set (above)"
        );
    }

    #[test]
    fn test_cmp_16bit_equal() {
        let (mut cpu, mut memory) = setup();
        // CMP AX, 0x1234 when AX=0x1234
        cpu.ax = 0x1234;
        memory.write_u16(0, 0x1234);
        cpu.cmp_imm_acc(0x3D, &memory);

        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry flag should not be set"
        );
    }
}

#[cfg(test)]
mod shift_rotate_tests {
    use crate::cpu::{Cpu, cpu_flag};
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_shl_8bit_by_1() {
        let (mut cpu, mut memory) = setup();
        // SHL on 0x42 (0100_0010) by 1 should give 0x84 (1000_0100)
        memory.write_u8(0x1000, 0x42);
        memory.write_u8(0, 0x26); // ModR/M: mode=00, op=100 (SHL), r/m=110
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x84, "Result should be 0x84");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry should be 0 (MSB was 0)"
        );
        assert!(cpu.get_flag(cpu_flag::SIGN), "Sign flag should be set");
        assert!(
            cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow should be set (sign bit changed)"
        );
    }

    #[test]
    fn test_shl_8bit_with_carry() {
        let (mut cpu, mut memory) = setup();
        // SHL on 0x80 (1000_0000) by 1 should give 0x00 with carry
        memory.write_u8(0x1000, 0x80);
        memory.write_u8(0, 0x26); // op=100 (SHL)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x00, "Result should be 0x00");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry should be set (MSB was 1)"
        );
        assert!(cpu.get_flag(cpu_flag::ZERO), "Zero flag should be set");
        assert!(
            cpu.get_flag(cpu_flag::OVERFLOW),
            "Overflow should be set (sign changed)"
        );
    }

    #[test]
    fn test_shr_8bit_by_1() {
        let (mut cpu, mut memory) = setup();
        // SHR on 0x42 (0100_0010) by 1 should give 0x21 (0010_0001)
        memory.write_u8(0x1000, 0x42);
        memory.write_u8(0, 0x2E); // ModR/M: op=101 (SHR)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x21, "Result should be 0x21");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry should be 0 (LSB was 0)"
        );
    }

    #[test]
    fn test_shr_8bit_with_carry() {
        let (mut cpu, mut memory) = setup();
        // SHR on 0x43 (0100_0011) by 1 should give 0x21 with carry
        memory.write_u8(0x1000, 0x43);
        memory.write_u8(0, 0x2E); // op=101 (SHR)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x21, "Result should be 0x21");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry should be set (LSB was 1)"
        );
    }

    #[test]
    fn test_sar_8bit_positive() {
        let (mut cpu, mut memory) = setup();
        // SAR on 0x42 (positive) by 1 should give 0x21
        memory.write_u8(0x1000, 0x42);
        memory.write_u8(0, 0x3E); // ModR/M: op=111 (SAR)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x21, "Result should be 0x21");
    }

    #[test]
    fn test_sar_8bit_negative() {
        let (mut cpu, mut memory) = setup();
        // SAR on 0xFE (1111_1110, -2) by 1 should give 0xFF (1111_1111, -1)
        memory.write_u8(0x1000, 0xFE);
        memory.write_u8(0, 0x3E); // op=111 (SAR)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(
            memory.read_u8(0x1000),
            0xFF,
            "Result should be 0xFF (sign extended)"
        );
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should be 0");
    }

    #[test]
    fn test_rol_8bit() {
        let (mut cpu, mut memory) = setup();
        // ROL on 0x81 (1000_0001) by 1 should give 0x03 (0000_0011)
        memory.write_u8(0x1000, 0x81);
        memory.write_u8(0, 0x06); // ModR/M: op=000 (ROL)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x03, "Result should be 0x03");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry should be set (MSB rotated to LSB)"
        );
    }

    #[test]
    fn test_ror_8bit() {
        let (mut cpu, mut memory) = setup();
        // ROR on 0x81 (1000_0001) by 1 should give 0xC0 (1100_0000)
        memory.write_u8(0x1000, 0x81);
        memory.write_u8(0, 0x0E); // ModR/M: op=001 (ROR)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0xC0, "Result should be 0xC0");
        assert!(
            cpu.get_flag(cpu_flag::CARRY),
            "Carry should be set (MSB after rotation)"
        );
    }

    #[test]
    fn test_rcl_8bit_carry_clear() {
        let (mut cpu, mut memory) = setup();
        // RCL on 0x40 (0100_0000) by 1 with CF=0 should give 0x80
        cpu.set_flag(cpu_flag::CARRY, false);
        memory.write_u8(0x1000, 0x40);
        memory.write_u8(0, 0x16); // ModR/M: op=010 (RCL)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x80, "Result should be 0x80");
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should be clear");
    }

    #[test]
    fn test_rcl_8bit_carry_set() {
        let (mut cpu, mut memory) = setup();
        // RCL on 0x40 (0100_0000) by 1 with CF=1 should give 0x81
        cpu.set_flag(cpu_flag::CARRY, true);
        memory.write_u8(0x1000, 0x40);
        memory.write_u8(0, 0x16); // op=010 (RCL)
        memory.write_u16(1, 0x1000);

        cpu.shift_rotate_group(0xD0, &mut memory);

        assert_eq!(memory.read_u8(0x1000), 0x81, "Result should be 0x81");
        assert!(
            !cpu.get_flag(cpu_flag::CARRY),
            "Carry should be clear (MSB was 0)"
        );
    }
}

#[cfg(test)]
mod multiply_divide_tests {
    use crate::cpu::Cpu;
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_mul_8bit() {
        let (mut cpu, mut memory) = setup();
        // MUL: AL=5, operand=6, result should be AX=30 (0x001E)
        cpu.ax = 0x0005;
        memory.write_u8(0x1000, 0x06);
        memory.write_u8(0, 0x26); // ModR/M: op=100 (MUL)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax, 0x001E, "AX should be 0x001E (30)");
    }

    #[test]
    fn test_mul_8bit_overflow() {
        let (mut cpu, mut memory) = setup();
        // MUL: AL=200, operand=3, result should be 600 (0x0258)
        cpu.ax = 0x00C8; // 200
        memory.write_u8(0x1000, 0x03);
        memory.write_u8(0, 0x26); // op=100 (MUL)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax, 0x0258, "AX should be 0x0258 (600)");
        // Upper byte (AH) is non-zero, so CF and OF should be set
    }

    #[test]
    fn test_mul_16bit() {
        let (mut cpu, mut memory) = setup();
        // MUL: AX=1000, operand=50, result should be DX:AX=50000 (0x0000C350)
        cpu.ax = 1000;
        cpu.dx = 0;
        memory.write_u16(0x1000, 50);
        memory.write_u8(0, 0x26); // op=100 (MUL)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF7, &mut memory);

        assert_eq!(cpu.ax, 0xC350, "AX should be 0xC350 (low word)");
        assert_eq!(cpu.dx, 0x0000, "DX should be 0x0000 (high word)");
    }

    #[test]
    fn test_imul_8bit_positive() {
        let (mut cpu, mut memory) = setup();
        // IMUL: AL=5, operand=6, result should be AX=30
        cpu.ax = 0x0005;
        memory.write_u8(0x1000, 0x06);
        memory.write_u8(0, 0x2E); // ModR/M: op=101 (IMUL)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax, 0x001E, "AX should be 0x001E (30)");
    }

    #[test]
    fn test_imul_8bit_negative() {
        let (mut cpu, mut memory) = setup();
        // IMUL: AL=-5 (0xFB), operand=6, result should be -30 (0xFFE2)
        cpu.ax = 0x00FB; // -5 in two's complement
        memory.write_u8(0x1000, 0x06);
        memory.write_u8(0, 0x2E); // op=101 (IMUL)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax, 0xFFE2, "AX should be 0xFFE2 (-30)");
    }

    #[test]
    fn test_div_8bit() {
        let (mut cpu, mut memory) = setup();
        // DIV: AX=30, divisor=6, quotient=5 (AL), remainder=0 (AH)
        cpu.ax = 30;
        memory.write_u8(0x1000, 6);
        memory.write_u8(0, 0x36); // ModR/M: op=110 (DIV)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax & 0xFF, 5, "AL (quotient) should be 5");
        assert_eq!(cpu.ax >> 8, 0, "AH (remainder) should be 0");
    }

    #[test]
    fn test_div_8bit_with_remainder() {
        let (mut cpu, mut memory) = setup();
        // DIV: AX=17, divisor=5, quotient=3 (AL), remainder=2 (AH)
        cpu.ax = 17;
        memory.write_u8(0x1000, 5);
        memory.write_u8(0, 0x36); // op=110 (DIV)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax & 0xFF, 3, "AL (quotient) should be 3");
        assert_eq!(cpu.ax >> 8, 2, "AH (remainder) should be 2");
    }

    #[test]
    fn test_div_16bit() {
        let (mut cpu, mut memory) = setup();
        // DIV: DX:AX=100000, divisor=3, quotient=33333 (AX), remainder=1 (DX)
        cpu.dx = 0x0001; // High word of 100000 (0x186A0)
        cpu.ax = 0x86A0; // Low word
        memory.write_u16(0x1000, 3);
        memory.write_u8(0, 0x36); // op=110 (DIV)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF7, &mut memory);

        assert_eq!(cpu.ax, 33333, "AX (quotient) should be 33333");
        assert_eq!(cpu.dx, 1, "DX (remainder) should be 1");
    }

    #[test]
    fn test_div_by_zero() {
        let (mut cpu, mut memory) = setup();
        cpu.ax = 10;
        memory.write_u8(0x1000, 0);
        memory.write_u8(0, 0x36); // op=110 (DIV)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);
        // Divide by zero should set pending_exception = Some(0) (INT 0)
        assert_eq!(
            cpu.pending_exception,
            Some(0),
            "Should set divide error exception"
        );
    }

    #[test]
    fn test_idiv_8bit_positive() {
        let (mut cpu, mut memory) = setup();
        // IDIV: AX=17, divisor=5, quotient=3, remainder=2
        cpu.ax = 17;
        memory.write_u8(0x1000, 5);
        memory.write_u8(0, 0x3E); // ModR/M: op=111 (IDIV)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax & 0xFF, 3, "AL (quotient) should be 3");
        assert_eq!(cpu.ax >> 8, 2, "AH (remainder) should be 2");
    }

    #[test]
    fn test_idiv_8bit_negative_dividend() {
        let (mut cpu, mut memory) = setup();
        // IDIV: AX=-17 (0xFFEF), divisor=5, quotient=-3 (0xFD), remainder=-2 (0xFE)
        cpu.ax = 0xFFEF; // -17 as signed 16-bit
        memory.write_u8(0x1000, 5);
        memory.write_u8(0, 0x3E); // op=111 (IDIV)
        memory.write_u16(1, 0x1000);

        cpu.unary_group3(0xF6, &mut memory);

        assert_eq!(cpu.ax & 0xFF, 0xFD, "AL (quotient) should be 0xFD (-3)");
        assert_eq!(cpu.ax >> 8, 0xFE, "AH (remainder) should be 0xFE (-2)");
    }
}

#[cfg(test)]
mod data_transfer_tests {
    use crate::cpu::Cpu;
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_mov_imm_to_reg_8bit() {
        let (mut cpu, mut memory) = setup();
        // MOV AL, 0x42
        memory.write_u8(0, 0x42);
        cpu.mov_imm_to_reg(0xB0, &memory); // 0xB0 = MOV AL, imm8

        assert_eq!(cpu.ax & 0xFF, 0x42, "AL should be 0x42");
    }

    #[test]
    fn test_mov_imm_to_reg_16bit() {
        let (mut cpu, mut memory) = setup();
        // MOV AX, 0x1234
        memory.write_u16(0, 0x1234);
        cpu.mov_imm_to_reg(0xB8, &memory); // 0xB8 = MOV AX, imm16

        assert_eq!(cpu.ax, 0x1234, "AX should be 0x1234");
    }

    #[test]
    fn test_push_pop_reg() {
        let (mut cpu, mut memory) = setup();
        // PUSH AX then POP BX
        cpu.ax = 0x1234;
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;

        cpu.push_reg16(0x50, &mut memory); // PUSH AX
        assert_eq!(cpu.sp, 0x0FFE, "SP should decrement by 2");
        assert_eq!(memory.read_u16(0x0FFE), 0x1234, "Value should be on stack");

        cpu.pop_reg16(0x5B, &mut memory); // POP BX
        assert_eq!(cpu.bx, 0x1234, "BX should have popped value");
        assert_eq!(cpu.sp, 0x1000, "SP should be restored");
    }

    #[test]
    fn test_xchg_ax_reg() {
        let (mut cpu, _memory) = setup();
        // XCHG AX, BX
        cpu.ax = 0x1234;
        cpu.bx = 0x5678;

        cpu.xchg_ax_reg(0x93); // 0x93 = XCHG AX, BX

        assert_eq!(cpu.ax, 0x5678, "AX should have BX's value");
        assert_eq!(cpu.bx, 0x1234, "BX should have AX's value");
    }

    #[test]
    fn test_xchg_nop() {
        let (mut cpu, _memory) = setup();
        // XCHG AX, AX is NOP (opcode 0x90)
        cpu.ax = 0x1234;

        cpu.xchg_ax_reg(0x90);

        assert_eq!(cpu.ax, 0x1234, "AX should be unchanged (NOP)");
    }

    #[test]
    fn test_cbw() {
        let (mut cpu, _memory) = setup();
        // CBW with AL=0x7F (positive) should give AX=0x007F
        cpu.ax = 0x007F;
        cpu.cbw();
        assert_eq!(cpu.ax, 0x007F, "AX should be 0x007F");

        // CBW with AL=0x80 (negative) should give AX=0xFF80
        cpu.ax = 0x0080;
        cpu.cbw();
        assert_eq!(cpu.ax, 0xFF80, "AX should be 0xFF80 (sign extended)");
    }

    #[test]
    fn test_cwd() {
        let (mut cpu, _memory) = setup();
        // CWD with AX=0x7FFF (positive) should give DX=0x0000
        cpu.ax = 0x7FFF;
        cpu.cwd();
        assert_eq!(cpu.dx, 0x0000, "DX should be 0x0000");

        // CWD with AX=0x8000 (negative) should give DX=0xFFFF
        cpu.ax = 0x8000;
        cpu.cwd();
        assert_eq!(cpu.dx, 0xFFFF, "DX should be 0xFFFF (sign extended)");
    }

    #[test]
    fn test_pushf_popf() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;
        cpu.flags = 0x0246; // Some flags set

        cpu.pushf(&mut memory);
        assert_eq!(cpu.sp, 0x0FFE, "SP should decrement");
        // PUSHF pushes flags as-is
        assert_eq!(memory.read_u16(0x0FFE), 0x0246, "Flags should be on stack");

        cpu.flags = 0x0000; // Clear flags
        cpu.popf(&mut memory);
        // 8086 behavior: POPF only allows bits 0-11 to be modified, restores to original
        assert_eq!(cpu.flags, 0x0246, "Flags should be restored (bits 0-11)");
        assert_eq!(cpu.sp, 0x1000, "SP should be restored");
    }

    #[test]
    fn test_lea() {
        let (mut cpu, mut memory) = setup();
        // LEA AX, [BX+SI+0x10]
        cpu.bx = 0x1000;
        cpu.si = 0x0020;
        memory.write_u8(0, 0x40); // ModR/M: mode=01 (8-bit disp), reg=000 (AX), r/m=000 ([BX+SI])
        memory.write_u8(1, 0x10); // 8-bit displacement

        cpu.lea(&memory);

        assert_eq!(cpu.ax, 0x1030, "AX should contain effective address 0x1030");
    }

    #[test]
    fn test_lahf_sahf() {
        let (mut cpu, _memory) = setup();
        // Set some flags and test LAHF
        cpu.flags = 0x0246; // SF, ZF, AF, PF, CF set
        cpu.ax = 0x0000;

        cpu.lahf();
        assert_eq!(cpu.ax >> 8, 0x46, "AH should contain low byte of flags");

        // Test SAHF
        cpu.ax = 0x8700; // Set AH to different value
        cpu.sahf();
        assert_eq!(cpu.flags & 0xFF, 0x87, "Low byte of flags should match AH");
    }
}

#[cfg(test)]
mod control_flow_tests {
    use crate::cpu::{Cpu, cpu_flag};
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_jmp_short_forward() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        memory.write_u8(0x0100, 0x10); // Jump forward by 16 bytes

        cpu.jmp_short(&memory);

        // IP after fetch_byte is 0x0101, plus offset 0x10 = 0x0111
        assert_eq!(cpu.ip, 0x0111, "IP should be 0x0111");
    }

    #[test]
    fn test_jmp_short_backward() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        memory.write_u8(0x0100, 0xF0); // Jump backward by 16 bytes (-16 as i8)

        cpu.jmp_short(&memory);

        // IP after fetch is 0x0101, plus offset -16 = 0x00F1
        assert_eq!(cpu.ip, 0x00F1, "IP should be 0x00F1");
    }

    #[test]
    fn test_jz_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.set_flag(cpu_flag::ZERO, true);
        memory.write_u8(0x0100, 0x10); // Offset

        cpu.jmp_conditional(0x74, &memory); // JZ/JE

        assert_eq!(cpu.ip, 0x0111, "Jump should be taken");
    }

    #[test]
    fn test_jz_not_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.set_flag(cpu_flag::ZERO, false);
        memory.write_u8(0x0100, 0x10);

        cpu.jmp_conditional(0x74, &memory); // JZ/JE

        assert_eq!(cpu.ip, 0x0101, "Jump should not be taken");
    }

    #[test]
    fn test_jnz_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.set_flag(cpu_flag::ZERO, false);
        memory.write_u8(0x0100, 0x10);

        cpu.jmp_conditional(0x75, &memory); // JNZ/JNE

        assert_eq!(cpu.ip, 0x0111, "Jump should be taken");
    }

    #[test]
    fn test_jc_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.set_flag(cpu_flag::CARRY, true);
        memory.write_u8(0x0100, 0x10);

        cpu.jmp_conditional(0x72, &memory); // JC/JB

        assert_eq!(cpu.ip, 0x0111, "Jump should be taken");
    }

    #[test]
    fn test_jnc_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.set_flag(cpu_flag::CARRY, false);
        memory.write_u8(0x0100, 0x10);

        cpu.jmp_conditional(0x73, &memory); // JNC/JAE

        assert_eq!(cpu.ip, 0x0111, "Jump should be taken");
    }

    #[test]
    fn test_js_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.set_flag(cpu_flag::SIGN, true);
        memory.write_u8(0x0100, 0x10);

        cpu.jmp_conditional(0x78, &memory); // JS

        assert_eq!(cpu.ip, 0x0111, "Jump should be taken");
    }

    #[test]
    fn test_loop_decrement_and_jump() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0005;
        memory.write_u8(0x0100, 0xF0); // Jump back by 16

        cpu.loop_inst(&memory);

        assert_eq!(cpu.cx, 0x0004, "CX should be decremented");
        assert_eq!(cpu.ip, 0x00F1, "Jump should be taken (CX != 0)");
    }

    #[test]
    fn test_loop_no_jump_when_zero() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0001;
        memory.write_u8(0x0100, 0xF0);

        cpu.loop_inst(&memory);

        assert_eq!(cpu.cx, 0x0000, "CX should be decremented to 0");
        assert_eq!(cpu.ip, 0x0101, "Jump should not be taken (CX = 0)");
    }

    #[test]
    fn test_loope_jump_when_zero_flag_set() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0005;
        cpu.set_flag(cpu_flag::ZERO, true);
        memory.write_u8(0x0100, 0xF0);

        cpu.loope(&memory);

        assert_eq!(cpu.cx, 0x0004, "CX should be decremented");
        assert_eq!(cpu.ip, 0x00F1, "Jump should be taken (CX != 0 and ZF = 1)");
    }

    #[test]
    fn test_loope_no_jump_when_zero_flag_clear() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0005;
        cpu.set_flag(cpu_flag::ZERO, false);
        memory.write_u8(0x0100, 0xF0);

        cpu.loope(&memory);

        assert_eq!(cpu.cx, 0x0004, "CX should be decremented");
        assert_eq!(cpu.ip, 0x0101, "Jump should not be taken (ZF = 0)");
    }

    #[test]
    fn test_loopne_jump_when_zero_flag_clear() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0005;
        cpu.set_flag(cpu_flag::ZERO, false);
        memory.write_u8(0x0100, 0xF0);

        cpu.loopne(&memory);

        assert_eq!(cpu.cx, 0x0004, "CX should be decremented");
        assert_eq!(cpu.ip, 0x00F1, "Jump should be taken (CX != 0 and ZF = 0)");
    }

    #[test]
    fn test_jcxz_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0000;
        memory.write_u8(0x0100, 0x10);

        cpu.jcxz(&memory);

        assert_eq!(cpu.ip, 0x0111, "Jump should be taken (CX = 0)");
    }

    #[test]
    fn test_jcxz_not_taken() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.cx = 0x0001;
        memory.write_u8(0x0100, 0x10);

        cpu.jcxz(&memory);

        assert_eq!(cpu.ip, 0x0101, "Jump should not be taken (CX != 0)");
    }

    #[test]
    fn test_call_near_and_ret() {
        let (mut cpu, mut memory) = setup();
        cpu.ip = 0x0100;
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;

        // CALL with offset +0x0050
        memory.write_u16(0x0100, 0x0050);
        cpu.call_near(&mut memory);

        assert_eq!(cpu.ip, 0x0152, "IP should jump to 0x0152 (0x0102 + 0x0050)");
        assert_eq!(cpu.sp, 0x0FFE, "SP should be decremented");
        assert_eq!(
            memory.read_u16(0x0FFE),
            0x0102,
            "Return address should be on stack"
        );

        // RET
        cpu.ret(0xC3, &mut memory);

        assert_eq!(cpu.ip, 0x0102, "IP should be restored");
        assert_eq!(cpu.sp, 0x1000, "SP should be restored");
    }

    #[test]
    fn test_hlt() {
        let (mut cpu, _memory) = setup();
        cpu.halted = false;

        cpu.hlt();

        assert!(cpu.halted, "CPU should be halted");
    }
}

#[cfg(test)]
mod string_tests {
    use crate::cpu::{Cpu, RepeatPrefix, cpu_flag};
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_movsb_forward() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0000;
        cpu.di = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false); // Forward

        memory.write_u8(0x10000, 0x42);

        cpu.movs(0xA4, &mut memory); // MOVSB

        assert_eq!(memory.read_u8(0x20000), 0x42, "Byte should be copied");
        assert_eq!(cpu.si, 0x0001, "SI should increment");
        assert_eq!(cpu.di, 0x0001, "DI should increment");
    }

    #[test]
    fn test_movsw_backward() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0010;
        cpu.di = 0x0010;
        cpu.set_flag(cpu_flag::DIRECTION, true); // Backward

        memory.write_u16(0x10010, 0x1234);

        cpu.movs(0xA5, &mut memory); // MOVSW

        assert_eq!(memory.read_u16(0x20010), 0x1234, "Word should be copied");
        assert_eq!(cpu.si, 0x000E, "SI should decrement by 2");
        assert_eq!(cpu.di, 0x000E, "DI should decrement by 2");
    }

    #[test]
    fn test_rep_movsb() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0000;
        cpu.di = 0x0000;
        cpu.cx = 0x0004; // Copy 4 bytes
        cpu.set_flag(cpu_flag::DIRECTION, false);
        cpu.repeat_prefix = Some(RepeatPrefix::Rep);

        memory.write_u8(0x10000, 0x41);
        memory.write_u8(0x10001, 0x42);
        memory.write_u8(0x10002, 0x43);
        memory.write_u8(0x10003, 0x44);

        cpu.movs(0xA4, &mut memory);

        assert_eq!(memory.read_u8(0x20000), 0x41, "First byte copied");
        assert_eq!(memory.read_u8(0x20001), 0x42, "Second byte copied");
        assert_eq!(memory.read_u8(0x20002), 0x43, "Third byte copied");
        assert_eq!(memory.read_u8(0x20003), 0x44, "Fourth byte copied");
        assert_eq!(cpu.cx, 0x0000, "CX should be zero");
        assert_eq!(cpu.si, 0x0004, "SI should be incremented");
    }

    #[test]
    fn test_cmpsb_equal() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0000;
        cpu.di = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false);

        memory.write_u8(0x10000, 0x42);
        memory.write_u8(0x20000, 0x42);

        cpu.cmps(0xA6, &memory); // CMPSB

        assert!(
            cpu.get_flag(cpu_flag::ZERO),
            "Zero flag should be set (equal)"
        );
        assert_eq!(cpu.si, 0x0001, "SI should increment");
        assert_eq!(cpu.di, 0x0001, "DI should increment");
    }

    #[test]
    fn test_cmpsb_not_equal() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.es = 0x2000;
        cpu.si = 0x0000;
        cpu.di = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false);

        memory.write_u8(0x10000, 0x42);
        memory.write_u8(0x20000, 0x43);

        cpu.cmps(0xA6, &memory);

        assert!(
            !cpu.get_flag(cpu_flag::ZERO),
            "Zero flag should not be set (not equal)"
        );
    }

    #[test]
    fn test_scasb_found() {
        let (mut cpu, mut memory) = setup();
        cpu.ax = 0x0042;
        cpu.es = 0x2000;
        cpu.di = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false);

        memory.write_u8(0x20000, 0x42);

        cpu.scas(0xAE, &memory); // SCASB

        assert!(
            cpu.get_flag(cpu_flag::ZERO),
            "Zero flag should be set (found)"
        );
        assert_eq!(cpu.di, 0x0001, "DI should increment");
    }

    #[test]
    fn test_lodsb() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.si = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false);

        memory.write_u8(0x10000, 0x42);

        cpu.lods(0xAC, &memory); // LODSB

        assert_eq!(cpu.ax & 0xFF, 0x42, "AL should contain loaded byte");
        assert_eq!(cpu.si, 0x0001, "SI should increment");
    }

    #[test]
    fn test_lodsw() {
        let (mut cpu, mut memory) = setup();
        cpu.ds = 0x1000;
        cpu.si = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false);

        memory.write_u16(0x10000, 0x1234);

        cpu.lods(0xAD, &memory); // LODSW

        assert_eq!(cpu.ax, 0x1234, "AX should contain loaded word");
        assert_eq!(cpu.si, 0x0002, "SI should increment by 2");
    }

    #[test]
    fn test_stosb() {
        let (mut cpu, mut memory) = setup();
        cpu.ax = 0x0042;
        cpu.es = 0x2000;
        cpu.di = 0x0000;
        cpu.set_flag(cpu_flag::DIRECTION, false);

        cpu.stos(0xAA, &mut memory); // STOSB

        assert_eq!(memory.read_u8(0x20000), 0x42, "Byte should be stored");
        assert_eq!(cpu.di, 0x0001, "DI should increment");
    }

    #[test]
    fn test_rep_stosw() {
        let (mut cpu, mut memory) = setup();
        cpu.ax = 0x1234;
        cpu.es = 0x2000;
        cpu.di = 0x0000;
        cpu.cx = 0x0003; // Store 3 words
        cpu.set_flag(cpu_flag::DIRECTION, false);
        cpu.repeat_prefix = Some(RepeatPrefix::Rep);

        cpu.stos(0xAB, &mut memory); // STOSW

        assert_eq!(memory.read_u16(0x20000), 0x1234, "First word stored");
        assert_eq!(memory.read_u16(0x20002), 0x1234, "Second word stored");
        assert_eq!(memory.read_u16(0x20004), 0x1234, "Third word stored");
        assert_eq!(cpu.cx, 0x0000, "CX should be zero");
        assert_eq!(cpu.di, 0x0006, "DI should be incremented by 6");
    }

    #[test]
    fn test_cld_std() {
        let (mut cpu, _memory) = setup();

        cpu.cld();
        assert!(!cpu.get_flag(cpu_flag::DIRECTION), "DF should be clear");

        cpu.std_flag();
        assert!(cpu.get_flag(cpu_flag::DIRECTION), "DF should be set");
    }
}

#[cfg(test)]
mod bcd_tests {
    use crate::cpu::{Cpu, cpu_flag};
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    #[test]
    fn test_daa_no_adjust() {
        let (mut cpu, _memory) = setup();
        // AL = 0x12 (valid BCD), no adjustment needed
        cpu.ax = 0x0012;
        cpu.set_flag(cpu_flag::CARRY, false);
        cpu.set_flag(cpu_flag::AUXILIARY, false);

        cpu.daa();

        assert_eq!(cpu.ax & 0xFF, 0x12, "AL should remain 0x12");
    }

    #[test]
    fn test_daa_lower_nibble_adjust() {
        let (mut cpu, _memory) = setup();
        // AL = 0x1A (lower nibble > 9), should add 6 to lower nibble
        cpu.ax = 0x001A;
        cpu.set_flag(cpu_flag::CARRY, false);
        cpu.set_flag(cpu_flag::AUXILIARY, false);

        cpu.daa();

        assert_eq!(cpu.ax & 0xFF, 0x20, "AL should be 0x20");
    }

    #[test]
    fn test_aaa_no_adjust() {
        let (mut cpu, _memory) = setup();
        // AL = 0x05 (valid), no adjustment needed
        cpu.ax = 0x0005;
        cpu.set_flag(cpu_flag::AUXILIARY, false);

        cpu.aaa();

        assert_eq!(cpu.ax & 0x0F, 0x05, "AL lower nibble should remain 5");
        assert!(!cpu.get_flag(cpu_flag::CARRY), "Carry should not be set");
    }

    #[test]
    fn test_aaa_adjust() {
        let (mut cpu, _memory) = setup();
        // AL = 0x0F (> 9), should adjust
        cpu.ax = 0x000F;
        cpu.set_flag(cpu_flag::AUXILIARY, false);

        cpu.aaa();

        assert_eq!(cpu.ax & 0x0F, 0x05, "AL lower nibble should be 5");
        assert_eq!((cpu.ax >> 8) & 0xFF, 0x01, "AH should increment");
        assert!(cpu.get_flag(cpu_flag::CARRY), "Carry should be set");
    }
}

#[cfg(test)]
mod flags_8086_tests {
    use crate::cpu::Cpu;
    use crate::memory::Memory;

    fn setup() -> (Cpu, Memory) {
        (Cpu::new(), Memory::new())
    }

    /// Test that PUSHF pushes flags as-is (8086 behavior)
    #[test]
    fn test_pushf_8086_flag_masking() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;

        // Set some flag bits
        cpu.flags = 0x0246;

        cpu.pushf(&mut memory);

        let pushed_flags = memory.read_u16(0x0FFE);

        // On 8086: PUSHF just pushes flags as-is
        assert_eq!(
            pushed_flags, 0x0246,
            "PUSHF should push flags as-is (got {:#06X})",
            pushed_flags
        );
    }

    /// Test that PUSHF preserves all bits internally set
    #[test]
    fn test_pushf_preserves_bits() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;

        // Set flags with various patterns
        cpu.flags = 0x0BCD;

        cpu.pushf(&mut memory);

        let pushed_flags = memory.read_u16(0x0FFE);

        // Should push exactly what's in flags
        assert_eq!(
            pushed_flags, cpu.flags,
            "PUSHF should push flags exactly as they are"
        );
    }

    /// Test that POPF only allows bits 0-11 to be modified
    #[test]
    fn test_popf_8086_bit_restriction() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x0FFE;
        cpu.ss = 0x0000;
        cpu.flags = 0x0000;

        // Try to pop flags with all bits set
        memory.write_u16(0x0FFE, 0xFFFF);

        cpu.popf(&mut memory);

        // Bits 12-15 should not be modifiable on 8086
        assert_eq!(
            cpu.flags & 0xF000,
            0x0000,
            "POPF should not allow modification of bits 12-15"
        );
        // Bit 1 should always be 1
        assert_eq!(cpu.flags & 0x0002, 0x0002, "Bit 1 should always be set");
        // Other low bits should be set
        assert_eq!(cpu.flags & 0x0FFD, 0x0FFD, "Bits 0,2-11 should be set");
    }

    /// Test that POPF forces bit 1 to always be set (reserved bit)
    #[test]
    fn test_popf_reserved_bit() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x0FFE;
        cpu.ss = 0x0000;

        // Try to pop flags with bit 1 clear
        memory.write_u16(0x0FFE, 0x0000);

        cpu.popf(&mut memory);

        // Bit 1 should still be set (reserved)
        assert_eq!(
            cpu.flags & 0x0002,
            0x0002,
            "Reserved bit 1 must always be set"
        );
    }

    /// Test CPU detection sequence (simulates real 8086 detection code)
    #[test]
    fn test_cpu_detection_8086() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;

        // Simulate CPU detection: try to clear all flags
        cpu.flags = 0x0002; // Start with bit 1 set (reserved)
        cpu.pushf(&mut memory);
        let flags_after_clear = memory.read_u16(0x0FFE);

        // On 8086, PUSHF pushes flags as-is
        assert_eq!(
            flags_after_clear, 0x0002,
            "After clearing flags, should be 0x0002"
        );

        // Simulate: try to set high bits via POPF
        memory.write_u16(0x0FFE, 0xF000);
        cpu.sp = 0x0FFE; // Position SP to pop the value we just wrote
        cpu.popf(&mut memory);
        cpu.pushf(&mut memory);
        let flags_after_set = memory.read_u16(0x0FFE);

        // On 8086, POPF can't modify bits 12-15, so they stay 0
        // PUSHF then pushes 0x0002 (only bit 1 set)
        assert_eq!(
            flags_after_set & 0xF000,
            0x0000,
            "After trying to set high bits via POPF, they should remain 0 (8086)"
        );
        assert_eq!(
            flags_after_set, 0x0002,
            "Flags should be 0x0002 after POPF can't modify high bits"
        );
    }

    /// Test that INT pushes flags as-is
    #[test]
    fn test_int_pushf() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;
        cpu.cs = 0x0000;
        cpu.ip = 0x0100;
        cpu.flags = 0x0246;

        // Set up INT vector (doesn't matter where it points for this test)
        memory.write_u16(0x14 * 4, 0x0200); // offset
        memory.write_u16(0x14 * 4 + 2, 0x0000); // segment

        // Execute INT (needs the interrupt number as next byte)
        memory.write_u8(0x0100, 0x14); // INT 0x14
        cpu.int(&mut memory);

        // Check that flags were pushed as-is
        let pushed_flags = memory.read_u16(0x0FFE);
        assert_eq!(pushed_flags, 0x0246, "INT should push flags as-is");
    }

    /// Test that IRET pops flags with 8086 restrictions
    #[test]
    fn test_iret_popf_restriction() {
        let (mut cpu, mut memory) = setup();
        cpu.sp = 0x0FFA;
        cpu.ss = 0x0000;

        // Set up stack as if returning from interrupt
        memory.write_u16(0x0FFA, 0x0100); // IP
        memory.write_u16(0x0FFC, 0x0000); // CS
        memory.write_u16(0x0FFE, 0xFFFF); // FLAGS with all bits set

        cpu.iret(&mut memory);

        // Check that high bits weren't set
        assert_eq!(
            cpu.flags & 0xF000,
            0x0000,
            "IRET should not allow modification of bits 12-15"
        );
        // Check that bit 1 is set
        assert_eq!(
            cpu.flags & 0x0002,
            0x0002,
            "IRET should force bit 1 to be set"
        );
    }

    #[test]
    fn test_push_sp_8086_behavior() {
        let mut cpu = Cpu::new();
        let mut memory = Memory::new();

        // Set initial SP value
        cpu.sp = 0x1000;
        cpu.ss = 0x0000;

        // PUSH SP (opcode 0x54)
        cpu.push_reg16(0x54, &mut memory);

        // On 8086: PUSH SP should push SP-2 (the value after decrement)
        // After PUSH SP with initial SP=0x1000:
        // - SP is decremented to 0x0FFE
        // - The value 0x0FFE is pushed to [SS:0x0FFE]
        assert_eq!(cpu.sp, 0x0FFE, "SP should be decremented by 2");

        // Verify the value written to memory is 0x0FFE (SP-2)
        let addr = Cpu::physical_address(cpu.ss, cpu.sp);
        let pushed_value = memory.read_u16(addr);
        assert_eq!(
            pushed_value, 0x0FFE,
            "PUSH SP should push SP-2 (8086 behavior), not original SP"
        );

        // Verify CPU detection sequence works correctly
        // This simulates: PUSH SP / POP BX / CMP BX, SP
        cpu.sp = 0x2000;
        cpu.push_reg16(0x54, &mut memory); // PUSH SP
        let bx_value = cpu.pop(&mut memory); // POP BX

        // On 8086: After PUSH SP (original=0x2000), the value 0x1FFE is pushed
        //          After POP BX, BX=0x1FFE but SP=0x2000 (restored after POP)
        //          So BX != SP (this is how software detects 8086)
        // On 80286+: After PUSH SP, the value 0x2000 is pushed
        //           After POP BX, BX=0x2000 and SP=0x2000 (both equal)
        assert_eq!(
            bx_value, 0x1FFE,
            "BX should get SP-2 (0x1FFE) from the pushed value"
        );
        assert_eq!(cpu.sp, 0x2000, "SP should be restored to 0x2000 after POP");
        assert_ne!(
            bx_value, cpu.sp,
            "After PUSH SP / POP BX, BX should NOT equal SP on 8086"
        );
    }
}

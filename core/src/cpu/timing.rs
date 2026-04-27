//! CPU Instruction Cycle Timing
//!
//! This module provides accurate cycle timing for 8086 instructions based on
//! Intel 8086 Family User's Manual specifications (Table 2-20 and Table 2-21).
//!
//! # References
//! - Intel 8086 Family User's Manual: https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf
//! - Effective Address (EA) calculation: Pages 2-61 to 2-75
//! - Instruction timing: Table 2-21

/// Calculate Effective Address (EA) calculation cycles
///
/// The 8086 CPU takes additional cycles to calculate memory addresses based on
/// the addressing mode complexity. This function computes those cycles based on
/// the ModR/M byte fields.
///
/// # Arguments
/// - `mode`: ModR/M mode field (bits 7-6)
///   - 0b00: No displacement (except r/m=110 is direct address)
///   - 0b01: 8-bit signed displacement
///   - 0b10: 16-bit displacement
///   - 0b11: Register mode (no EA calculation)
/// - `rm`: ModR/M r/m field (bits 2-0) - specifies addressing mode
/// - `has_segment_override`: true if a segment override prefix was used
///
/// # Returns
/// Number of clock cycles required for EA calculation (0 for register mode)
///
/// # EA Calculation Cycles (from Intel 8086 manual Table 2-20)
///
/// ## Mode 00 (no displacement, except r/m=110):
/// - [BX+SI]: 7 cycles
/// - [BX+DI]: 8 cycles
/// - [BP+SI]: 8 cycles
/// - [BP+DI]: 7 cycles
/// - [SI]: 5 cycles
/// - [DI]: 5 cycles
/// - [direct address]: 6 cycles (r/m=110 special case)
/// - [BX]: 5 cycles
///
/// ## Mode 01 (8-bit displacement):
/// - [BX+SI+disp8]: 11 cycles
/// - [BX+DI+disp8]: 12 cycles
/// - [BP+SI+disp8]: 12 cycles
/// - [BP+DI+disp8]: 11 cycles
/// - [SI+disp8]: 9 cycles
/// - [DI+disp8]: 9 cycles
/// - [BP+disp8]: 9 cycles
/// - [BX+disp8]: 9 cycles
///
/// ## Mode 10 (16-bit displacement):
/// - [BX+SI+disp16]: 11 cycles
/// - [BX+DI+disp16]: 12 cycles
/// - [BP+SI+disp16]: 12 cycles
/// - [BP+DI+disp16]: 11 cycles
/// - [SI+disp16]: 9 cycles
/// - [DI+disp16]: 9 cycles
/// - [BP+disp16]: 9 cycles
/// - [BX+disp16]: 9 cycles
///
/// ## Mode 11 (register):
/// - No EA calculation: 0 cycles
///
/// ## Segment Override Penalty:
/// - +2 cycles if segment override prefix was used
pub(crate) fn calculate_ea_cycles(mode: u8, rm: u8, has_segment_override: bool) -> u32 {
    let base_ea = match (mode, rm) {
        // Register mode - no EA calculation
        (0b11, _) => 0,

        // Mode 00 (no displacement except r/m=110)
        (0b00, 0b000) => 7, // [BX+SI]
        (0b00, 0b001) => 8, // [BX+DI]
        (0b00, 0b010) => 8, // [BP+SI]
        (0b00, 0b011) => 7, // [BP+DI]
        (0b00, 0b100) => 5, // [SI]
        (0b00, 0b101) => 5, // [DI]
        (0b00, 0b110) => 6, // Direct address [disp16]
        (0b00, 0b111) => 5, // [BX]

        // Mode 01 (8-bit displacement)
        (0b01, 0b000) => 11, // [BX+SI+disp8]
        (0b01, 0b001) => 12, // [BX+DI+disp8]
        (0b01, 0b010) => 12, // [BP+SI+disp8]
        (0b01, 0b011) => 11, // [BP+DI+disp8]
        (0b01, 0b100) => 9,  // [SI+disp8]
        (0b01, 0b101) => 9,  // [DI+disp8]
        (0b01, 0b110) => 9,  // [BP+disp8]
        (0b01, 0b111) => 9,  // [BX+disp8]

        // Mode 10 (16-bit displacement)
        (0b10, 0b000) => 11, // [BX+SI+disp16]
        (0b10, 0b001) => 12, // [BX+DI+disp16]
        (0b10, 0b010) => 12, // [BP+SI+disp16]
        (0b10, 0b011) => 11, // [BP+DI+disp16]
        (0b10, 0b100) => 9,  // [SI+disp16]
        (0b10, 0b101) => 9,  // [DI+disp16]
        (0b10, 0b110) => 9,  // [BP+disp16]
        (0b10, 0b111) => 9,  // [BX+disp16]

        _ => unreachable!(
            "Invalid mode/rm combination: mode={:02b}, rm={:03b}",
            mode, rm
        ),
    };

    // Add segment override penalty (2 cycles per Intel manual)
    // But only for memory modes - register mode has no EA calculation
    let seg_penalty = if has_segment_override && mode != 0b11 {
        2
    } else {
        0
    };

    base_ea + seg_penalty
}

/// Instruction cycle timing constants from Intel 8086 Family User's Manual
///
/// These values represent base cycle counts for instructions. Many instructions
/// add EA (Effective Address) calculation cycles when accessing memory.
///
/// Notation: "+EA" means add EA calculation cycles (see calculate_ea_cycles)
#[allow(dead_code)]
pub mod cycles {
    //
    // Data Transfer Instructions
    //

    /// MOV register to register: 2 cycles
    pub const MOV_REG_REG: u32 = 2;

    /// MOV register to memory: 9 cycles + EA
    pub const MOV_REG_MEM: u32 = 9;

    /// MOV memory to register: 8 cycles + EA
    pub const MOV_MEM_REG: u32 = 8;

    /// MOV immediate to register (8-bit or 16-bit): 4 cycles
    pub const MOV_IMM_REG: u32 = 4;

    /// MOV immediate to memory: 10 cycles + EA
    pub const MOV_IMM_MEM: u32 = 10;

    /// MOV memory to accumulator (direct addressing): 10 cycles
    pub const MOV_MEM_ACC: u32 = 10;

    /// MOV accumulator to memory (direct addressing): 10 cycles
    pub const MOV_ACC_MEM: u32 = 10;

    /// MOV segment register to register/memory: 2 cycles (reg), 9+EA (mem)
    pub const MOV_SEGREG_RM_REG: u32 = 2;
    pub const MOV_SEGREG_RM_MEM: u32 = 9;

    /// MOV register/memory to segment register: 2 cycles (reg), 8+EA (mem)
    pub const MOV_RM_SEGREG_REG: u32 = 2;
    pub const MOV_RM_SEGREG_MEM: u32 = 8;

    /// PUSH register: 11 cycles
    pub const PUSH_REG: u32 = 11;

    /// PUSH segment register: 10 cycles
    pub const PUSH_SEGREG: u32 = 10;

    /// PUSH memory: 16 cycles + EA
    pub const PUSH_MEM: u32 = 16;

    /// PUSH immediate (80186+): 10 cycles
    pub const PUSH_IMM: u32 = 10;

    /// POP register: 8 cycles
    pub const POP_REG: u32 = 8;

    /// POP segment register: 8 cycles
    pub const POP_SEGREG: u32 = 8;

    /// POP memory: 17 cycles + EA
    pub const POP_MEM: u32 = 17;

    /// XCHG register with accumulator: 3 cycles
    pub const XCHG_REG_ACC: u32 = 3;

    /// XCHG register with register: 4 cycles
    pub const XCHG_REG_REG: u32 = 4;

    /// XCHG register with memory: 17 cycles + EA
    pub const XCHG_REG_MEM: u32 = 17;

    /// LEA (Load Effective Address): 2 cycles + EA
    pub const LEA: u32 = 2;

    /// LDS (Load pointer using DS): 16 cycles + EA
    pub const LDS: u32 = 16;

    /// LES (Load pointer using ES): 16 cycles + EA
    pub const LES: u32 = 16;

    /// PUSHA (Push all - 80186+): 36 cycles
    pub const PUSHA: u32 = 36;

    /// POPA (Pop all - 80186+): 51 cycles
    pub const POPA: u32 = 51;

    //
    // Arithmetic Instructions
    //

    /// ADD register to register: 3 cycles
    pub const ADD_REG_REG: u32 = 3;

    /// ADD register to memory: 16 cycles + EA
    pub const ADD_REG_MEM: u32 = 16;

    /// ADD memory to register: 9 cycles + EA
    pub const ADD_MEM_REG: u32 = 9;

    /// ADD immediate to register: 4 cycles
    pub const ADD_IMM_REG: u32 = 4;

    /// ADD immediate to memory: 17 cycles + EA
    pub const ADD_IMM_MEM: u32 = 17;

    /// ADD immediate to accumulator: 4 cycles
    pub const ADD_IMM_ACC: u32 = 4;

    /// ADC (same as ADD)
    pub const ADC_REG_REG: u32 = 3;
    pub const ADC_REG_MEM: u32 = 16;
    pub const ADC_MEM_REG: u32 = 9;
    pub const ADC_IMM_REG: u32 = 4;
    pub const ADC_IMM_MEM: u32 = 17;
    pub const ADC_IMM_ACC: u32 = 4;

    /// SUB (same as ADD)
    pub const SUB_REG_REG: u32 = 3;
    pub const SUB_REG_MEM: u32 = 16;
    pub const SUB_MEM_REG: u32 = 9;
    pub const SUB_IMM_REG: u32 = 4;
    pub const SUB_IMM_MEM: u32 = 17;
    pub const SUB_IMM_ACC: u32 = 4;

    /// SBB (same as ADD)
    pub const SBB_REG_REG: u32 = 3;
    pub const SBB_REG_MEM: u32 = 16;
    pub const SBB_MEM_REG: u32 = 9;
    pub const SBB_IMM_REG: u32 = 4;
    pub const SBB_IMM_MEM: u32 = 17;
    pub const SBB_IMM_ACC: u32 = 4;

    /// INC register: 2 cycles
    pub const INC_REG: u32 = 2;

    /// INC memory: 15 cycles + EA
    pub const INC_MEM: u32 = 15;

    /// DEC register: 2 cycles
    pub const DEC_REG: u32 = 2;

    /// DEC memory: 15 cycles + EA
    pub const DEC_MEM: u32 = 15;

    /// NEG register: 3 cycles
    pub const NEG_REG: u32 = 3;

    /// NEG memory: 16 cycles + EA
    pub const NEG_MEM: u32 = 16;

    /// CMP register with register: 3 cycles
    pub const CMP_REG_REG: u32 = 3;

    /// CMP register with memory: 9 cycles + EA
    pub const CMP_REG_MEM: u32 = 9;

    /// CMP memory with register: 9 cycles + EA
    pub const CMP_MEM_REG: u32 = 9;

    /// CMP immediate with register: 4 cycles
    pub const CMP_IMM_REG: u32 = 4;

    /// CMP immediate with memory: 10 cycles + EA
    pub const CMP_IMM_MEM: u32 = 10;

    /// CMP immediate with accumulator: 4 cycles
    pub const CMP_IMM_ACC: u32 = 4;

    /// MUL register (8-bit): 70-77 cycles (data-dependent, use average)
    pub const MUL_REG8: u32 = 74;

    /// MUL register (16-bit): 118-133 cycles (data-dependent, use average)
    pub const MUL_REG16: u32 = 126;

    /// MUL memory (8-bit): 76-83 cycles + EA
    pub const MUL_MEM8: u32 = 80;

    /// MUL memory (16-bit): 124-139 cycles + EA
    pub const MUL_MEM16: u32 = 132;

    /// IMUL register (8-bit): 80-98 cycles (data-dependent, use average)
    pub const IMUL_REG8: u32 = 89;

    /// IMUL register (16-bit): 128-154 cycles (data-dependent, use average)
    pub const IMUL_REG16: u32 = 141;

    /// IMUL memory (8-bit): 86-104 cycles + EA
    pub const IMUL_MEM8: u32 = 95;

    /// IMUL memory (16-bit): 134-160 cycles + EA
    pub const IMUL_MEM16: u32 = 147;

    /// IMUL with immediate (80186+): 22-25 cycles (register), 25-28 + EA (memory)
    pub const IMUL_IMM_REG: u32 = 24;
    pub const IMUL_IMM_MEM: u32 = 27;

    /// DIV register (8-bit): 80-90 cycles (data-dependent, use pessimistic)
    pub const DIV_REG8: u32 = 90;

    /// DIV register (16-bit): 144-162 cycles (data-dependent, use pessimistic)
    pub const DIV_REG16: u32 = 162;

    /// DIV memory (8-bit): 86-96 cycles + EA
    pub const DIV_MEM8: u32 = 96;

    /// DIV memory (16-bit): 150-168 cycles + EA
    pub const DIV_MEM16: u32 = 168;

    /// IDIV register (8-bit): 101-112 cycles (data-dependent, use pessimistic)
    pub const IDIV_REG8: u32 = 112;

    /// IDIV register (16-bit): 165-184 cycles (data-dependent, use pessimistic)
    pub const IDIV_REG16: u32 = 184;

    /// IDIV memory (8-bit): 107-118 cycles + EA
    pub const IDIV_MEM8: u32 = 118;

    /// IDIV memory (16-bit): 171-190 cycles + EA
    pub const IDIV_MEM16: u32 = 190;

    /// CBW (Convert Byte to Word): 2 cycles
    pub const CBW: u32 = 2;

    /// CWD (Convert Word to Doubleword): 5 cycles
    pub const CWD: u32 = 5;

    //
    // Logical Instructions
    //

    /// AND register with register: 3 cycles
    pub const AND_REG_REG: u32 = 3;
    pub const AND_REG_MEM: u32 = 16;
    pub const AND_MEM_REG: u32 = 9;
    pub const AND_IMM_REG: u32 = 4;
    pub const AND_IMM_MEM: u32 = 17;
    pub const AND_IMM_ACC: u32 = 4;

    /// OR (same as AND)
    pub const OR_REG_REG: u32 = 3;
    pub const OR_REG_MEM: u32 = 16;
    pub const OR_MEM_REG: u32 = 9;
    pub const OR_IMM_REG: u32 = 4;
    pub const OR_IMM_MEM: u32 = 17;
    pub const OR_IMM_ACC: u32 = 4;

    /// XOR (same as AND)
    pub const XOR_REG_REG: u32 = 3;
    pub const XOR_REG_MEM: u32 = 16;
    pub const XOR_MEM_REG: u32 = 9;
    pub const XOR_IMM_REG: u32 = 4;
    pub const XOR_IMM_MEM: u32 = 17;
    pub const XOR_IMM_ACC: u32 = 4;

    /// NOT register: 3 cycles
    pub const NOT_REG: u32 = 3;
    pub const NOT_MEM: u32 = 16;

    /// TEST register with register: 3 cycles
    pub const TEST_REG_REG: u32 = 3;
    pub const TEST_REG_MEM: u32 = 9;
    pub const TEST_IMM_REG: u32 = 5;
    pub const TEST_IMM_MEM: u32 = 11;
    pub const TEST_IMM_ACC: u32 = 4;

    //
    // Shift and Rotate Instructions
    //

    /// Shift/Rotate register by 1: 2 cycles
    pub const SHIFT_REG_1: u32 = 2;

    /// Shift/Rotate memory by 1: 15 cycles + EA
    pub const SHIFT_MEM_1: u32 = 15;

    /// Shift/Rotate register by CL: 8 + 4*count cycles
    pub const SHIFT_REG_CL_BASE: u32 = 8;
    pub const SHIFT_REG_CL_PER_COUNT: u32 = 4;

    /// Shift/Rotate memory by CL: 20 + 4*count cycles + EA
    pub const SHIFT_MEM_CL_BASE: u32 = 20;
    pub const SHIFT_MEM_CL_PER_COUNT: u32 = 4;

    /// Shift/Rotate register by immediate (80186+): 5 + count cycles
    pub const SHIFT_REG_IMM_BASE: u32 = 5;
    pub const SHIFT_REG_IMM_PER_COUNT: u32 = 1;

    /// Shift/Rotate memory by immediate (80186+): 17 + count cycles + EA
    pub const SHIFT_MEM_IMM_BASE: u32 = 17;
    pub const SHIFT_MEM_IMM_PER_COUNT: u32 = 1;

    //
    // Control Transfer Instructions
    //

    /// JMP short (within -128 to +127 bytes): 15 cycles
    pub const JMP_SHORT: u32 = 15;

    /// JMP near direct: 15 cycles
    pub const JMP_NEAR_DIRECT: u32 = 15;

    /// JMP near indirect through register: 11 cycles
    pub const JMP_NEAR_INDIRECT_REG: u32 = 11;

    /// JMP near indirect through memory: 18 cycles + EA
    pub const JMP_NEAR_INDIRECT_MEM: u32 = 18;

    /// JMP far direct: 15 cycles
    pub const JMP_FAR_DIRECT: u32 = 15;

    /// JMP far indirect through memory: 24 cycles + EA
    pub const JMP_FAR_INDIRECT_MEM: u32 = 24;

    /// Conditional jump (taken): 16 cycles
    pub const CONDITIONAL_JUMP_TAKEN: u32 = 16;

    /// Conditional jump (not taken): 4 cycles
    pub const CONDITIONAL_JUMP_NOT_TAKEN: u32 = 4;

    /// LOOP (taken): 17 cycles
    pub const LOOP_TAKEN: u32 = 17;

    /// LOOP (not taken, CX=0): 5 cycles
    pub const LOOP_NOT_TAKEN: u32 = 5;

    /// LOOPE/LOOPZ (taken): 18 cycles
    pub const LOOPE_TAKEN: u32 = 18;

    /// LOOPE/LOOPZ (not taken): 6 cycles
    pub const LOOPE_NOT_TAKEN: u32 = 6;

    /// LOOPNE/LOOPNZ (taken): 19 cycles
    pub const LOOPNE_TAKEN: u32 = 19;

    /// LOOPNE/LOOPNZ (not taken): 5 cycles
    pub const LOOPNE_NOT_TAKEN: u32 = 5;

    /// JCXZ (taken): 18 cycles
    pub const JCXZ_TAKEN: u32 = 18;

    /// JCXZ (not taken): 6 cycles
    pub const JCXZ_NOT_TAKEN: u32 = 6;

    /// CALL near direct: 19 cycles
    pub const CALL_NEAR_DIRECT: u32 = 19;

    /// CALL near indirect through register: 16 cycles
    pub const CALL_NEAR_INDIRECT_REG: u32 = 16;

    /// CALL near indirect through memory: 21 cycles + EA
    pub const CALL_NEAR_INDIRECT_MEM: u32 = 21;

    /// CALL far direct: 28 cycles
    pub const CALL_FAR_DIRECT: u32 = 28;

    /// CALL far indirect through memory: 37 cycles + EA
    pub const CALL_FAR_INDIRECT_MEM: u32 = 37;

    /// RET near: 8 cycles
    pub const RET_NEAR: u32 = 8;

    /// RET near with pop value: 12 cycles
    pub const RET_NEAR_POP: u32 = 12;

    /// RET far: 18 cycles
    pub const RET_FAR: u32 = 18;

    /// RET far with pop value: 17 cycles
    pub const RET_FAR_POP: u32 = 17;

    /// INT (software interrupt): 51 cycles
    pub const INT: u32 = 51;

    /// INT 3 (breakpoint): 52 cycles
    pub const INT3: u32 = 52;

    /// INTO (interrupt on overflow, taken): 53 cycles
    pub const INTO_TAKEN: u32 = 53;

    /// INTO (interrupt on overflow, not taken): 4 cycles
    pub const INTO_NOT_TAKEN: u32 = 4;

    /// IRET (interrupt return): 24 cycles
    pub const IRET: u32 = 24;

    //
    // String Instructions (per iteration)
    //

    /// MOVSB/MOVSW: 18 cycles per iteration
    pub const MOVS: u32 = 18;

    /// REP MOVSB/MOVSW: 9 + 17*CX cycles
    pub const REP_MOVS_BASE: u32 = 9;
    pub const REP_MOVS_PER_ITER: u32 = 17;

    /// CMPSB/CMPSW: 22 cycles per iteration
    pub const CMPS: u32 = 22;

    /// REPE/REPNE CMPSB/CMPSW: 9 + 22*count cycles (count <= CX)
    pub const REP_CMPS_BASE: u32 = 9;
    pub const REP_CMPS_PER_ITER: u32 = 22;

    /// SCASB/SCASW: 15 cycles per iteration
    pub const SCAS: u32 = 15;

    /// REPE/REPNE SCASB/SCASW: 9 + 15*count cycles (count <= CX)
    pub const REP_SCAS_BASE: u32 = 9;
    pub const REP_SCAS_PER_ITER: u32 = 15;

    /// LODSB/LODSW: 12 cycles per iteration
    pub const LODS: u32 = 12;

    /// REP LODSB/LODSW: 9 + 13*CX cycles
    pub const REP_LODS_BASE: u32 = 9;
    pub const REP_LODS_PER_ITER: u32 = 13;

    /// STOSB/STOSW: 11 cycles per iteration
    pub const STOS: u32 = 11;

    /// REP STOSB/STOSW: 9 + 10*CX cycles
    pub const REP_STOS_BASE: u32 = 9;
    pub const REP_STOS_PER_ITER: u32 = 10;

    //
    // I/O Instructions
    //

    /// IN AL/AX, imm8: 10 cycles
    pub const IN_IMM: u32 = 10;

    /// IN AL/AX, DX: 8 cycles
    pub const IN_DX: u32 = 8;

    /// OUT imm8, AL/AX: 10 cycles
    pub const OUT_IMM: u32 = 10;

    /// OUT DX, AL/AX: 8 cycles
    pub const OUT_DX: u32 = 8;

    //
    // Miscellaneous Instructions
    //

    /// NOP (XCHG AX, AX): 3 cycles
    pub const NOP: u32 = 3;

    /// HLT: 2 cycles
    pub const HLT: u32 = 2;

    /// XLAT: 11 cycles
    pub const XLAT: u32 = 11;

    /// LAHF (Load AH from Flags): 4 cycles
    pub const LAHF: u32 = 4;

    /// SAHF (Store AH into Flags): 4 cycles
    pub const SAHF: u32 = 4;

    /// PUSHF: 10 cycles
    pub const PUSHF: u32 = 10;

    /// POPF: 8 cycles
    pub const POPF: u32 = 8;

    /// CLC/STC/CMC/CLD/STD/CLI/STI: 2 cycles
    pub const FLAG_OPS: u32 = 2;

    /// AAA/AAS: 4 cycles
    pub const AAA: u32 = 4;
    pub const AAS: u32 = 4;

    /// DAA/DAS: 4 cycles
    pub const DAA: u32 = 4;
    pub const DAS: u32 = 4;

    /// AAM: 83 cycles
    pub const AAM: u32 = 83;

    /// AAD: 60 cycles
    pub const AAD: u32 = 60;

    /// ESC (coprocessor escape): 2 cycles (no coprocessor present)
    pub const ESC: u32 = 2;

    /// BOUND (80186+): 33-35 cycles if in bounds, 48-51 if out of bounds
    pub const BOUND_IN: u32 = 35;
    pub const BOUND_OUT: u32 = 51;

    /// ENTER (80186+): 15 cycles (level=0), 25+16*level cycles (level>0)
    pub const ENTER_LEVEL0: u32 = 15;
    pub const ENTER_LEVEL_BASE: u32 = 25;
    pub const ENTER_LEVEL_PER: u32 = 16;

    /// LEAVE (80186+): 8 cycles
    pub const LEAVE: u32 = 8;

    /// LOADALL (286, undocumented 0F 05): 195 cycles
    pub const LOADALL: u32 = 195;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_log::test]
    fn test_ea_cycles_mode_00_no_disp() {
        // Mode 00 addressing modes (no displacement)
        assert_eq!(calculate_ea_cycles(0b00, 0b000, false), 7); // [BX+SI]
        assert_eq!(calculate_ea_cycles(0b00, 0b001, false), 8); // [BX+DI]
        assert_eq!(calculate_ea_cycles(0b00, 0b010, false), 8); // [BP+SI]
        assert_eq!(calculate_ea_cycles(0b00, 0b011, false), 7); // [BP+DI]
        assert_eq!(calculate_ea_cycles(0b00, 0b100, false), 5); // [SI]
        assert_eq!(calculate_ea_cycles(0b00, 0b101, false), 5); // [DI]
        assert_eq!(calculate_ea_cycles(0b00, 0b110, false), 6); // Direct [disp16]
        assert_eq!(calculate_ea_cycles(0b00, 0b111, false), 5); // [BX]
    }

    #[test_log::test]
    fn test_ea_cycles_mode_01_disp8() {
        // Mode 01 addressing modes (8-bit displacement)
        assert_eq!(calculate_ea_cycles(0b01, 0b000, false), 11); // [BX+SI+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b001, false), 12); // [BX+DI+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b010, false), 12); // [BP+SI+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b011, false), 11); // [BP+DI+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b100, false), 9); // [SI+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b101, false), 9); // [DI+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b110, false), 9); // [BP+disp8]
        assert_eq!(calculate_ea_cycles(0b01, 0b111, false), 9); // [BX+disp8]
    }

    #[test_log::test]
    fn test_ea_cycles_mode_10_disp16() {
        // Mode 10 addressing modes (16-bit displacement)
        assert_eq!(calculate_ea_cycles(0b10, 0b000, false), 11); // [BX+SI+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b001, false), 12); // [BX+DI+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b010, false), 12); // [BP+SI+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b011, false), 11); // [BP+DI+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b100, false), 9); // [SI+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b101, false), 9); // [DI+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b110, false), 9); // [BP+disp16]
        assert_eq!(calculate_ea_cycles(0b10, 0b111, false), 9); // [BX+disp16]
    }

    #[test_log::test]
    fn test_ea_cycles_mode_11_register() {
        // Mode 11 is register mode - no EA calculation
        for rm in 0..8 {
            assert_eq!(calculate_ea_cycles(0b11, rm, false), 0);
        }
    }

    #[test_log::test]
    fn test_ea_cycles_segment_override() {
        // Segment override adds 2 cycles
        assert_eq!(calculate_ea_cycles(0b00, 0b100, false), 5); // [SI] no override
        assert_eq!(calculate_ea_cycles(0b00, 0b100, true), 7); // [SI] with override

        assert_eq!(calculate_ea_cycles(0b01, 0b100, false), 9); // [SI+disp8] no override
        assert_eq!(calculate_ea_cycles(0b01, 0b100, true), 11); // [SI+disp8] with override

        // Register mode ignores segment override (no EA calculation)
        assert_eq!(calculate_ea_cycles(0b11, 0b000, false), 0);
        assert_eq!(calculate_ea_cycles(0b11, 0b000, true), 0);
    }
}

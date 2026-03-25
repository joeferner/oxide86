use crate::Computer;

#[cfg(test)]
mod tests;

mod debug_info;
use debug_info::{int_description, port_comment};

mod fpu;

mod registers;
pub use registers::{Reg8, Reg16, SegReg};

mod mem_addr;
pub use mem_addr::{MemRef, RmBase};

mod operand;
pub use operand::Operand;

mod mnemonic;
pub use mnemonic::Mnemonic;

mod instruction;
pub use instruction::Instruction;

mod cursor;
use cursor::Cursor;

// ─── ModRM decoding ───────────────────────────────────────────────────────────

/// Decode the memory address portion of a ModRM byte (mode must not be 3).
/// Reads any displacement bytes from `cur` and returns a `MemRef`.
pub(super) fn decode_mem_ref(cur: &mut Cursor, modrm: u8) -> MemRef {
    let mod_ = (modrm >> 6) & 3;
    let rm = modrm & 7;

    let (ea, seg, expr) = if mod_ == 0 && rm == 6 {
        let addr = cur.fetch16();
        let expr = format!("0x{:04x}", addr);
        (addr, cur.seg_for_direct(), expr)
    } else {
        let base = RmBase::from_bits(rm);
        let base_ea = base.compute(cur.cpu);
        let (ea, expr) = match mod_ {
            0 => (base_ea, base.asm_str().to_string()),
            1 => {
                let disp = cur.fetch() as i8;
                let ea = base_ea.wrapping_add(disp as u16);
                let expr = if disp >= 0 {
                    format!("{}+0x{:02x}", base.asm_str(), disp as u8)
                } else {
                    format!("{}-0x{:02x}", base.asm_str(), disp.unsigned_abs())
                };
                (ea, expr)
            }
            2 => {
                let disp = cur.fetch16();
                let ea = base_ea.wrapping_add(disp);
                let expr = format!("{}+0x{:04x}", base.asm_str(), disp);
                (ea, expr)
            }
            _ => unreachable!(),
        };
        (ea, cur.seg_for_base(base), expr)
    };
    MemRef { seg, ea, expr }
}

/// Decode the `rm` field of a ModRM byte into an Operand.
/// Any displacement bytes are consumed from `cur`.
fn decode_rm(cur: &mut Cursor, modrm: u8, is16: bool) -> Operand {
    let mod_ = (modrm >> 6) & 3;
    let rm = modrm & 7;

    if mod_ == 3 {
        return if is16 {
            let r = Reg16::from_bits(rm);
            Operand::Reg16(r, r.value(cur.cpu))
        } else {
            let r = Reg8::from_bits(rm);
            Operand::Reg8(r, r.value(cur.cpu))
        };
    }

    let mem = decode_mem_ref(cur, modrm);
    if is16 {
        let value = cur.read_mem_u16(mem.seg, mem.ea);
        Operand::Mem16 { mem, value }
    } else {
        let value = cur.read_mem_u8(mem.seg, mem.ea);
        Operand::Mem8 { mem, value }
    }
}

/// Decode the `reg` field of a ModRM byte into a register Operand.
fn decode_reg(cur: &Cursor, reg_bits: u8, is16: bool) -> Operand {
    if is16 {
        let r = Reg16::from_bits(reg_bits);
        Operand::Reg16(r, r.value(cur.cpu))
    } else {
        let r = Reg8::from_bits(reg_bits);
        Operand::Reg8(r, r.value(cur.cpu))
    }
}

// ─── Public decode entry point ────────────────────────────────────────────────

/// Decode a single 8086/286 instruction at `seg:offset`.
/// CPU register and memory state is read via `cpu` (typically the state immediately
/// after execution, so annotations reflect results).
pub fn decode(cpu: &dyn Computer, seg: u16, offset: u16) -> Instruction {
    let mut cur = Cursor::new(cpu, seg, offset);
    let (mnemonic, operands) = decode_inner(&mut cur);
    // FPU implicit-operand instructions: move annotation-only operands out of
    // the main operand list so they appear in annotations but not the asm column.
    let (operands, implicit_operands) = split_fpu_implicit(&mnemonic, operands);
    let comment = default_comment(&mnemonic, &operands, cpu);
    Instruction {
        segment: seg,
        offset,
        bytes: cur.bytes,
        mnemonic,
        operands,
        implicit_operands,
        seg_override: cur.seg_override,
        comment,
    }
}

/// For FPU instructions with no explicit operands (e.g. fptan, fpatan, fchs),
/// move all FpuReg operands to implicit so they show only in annotations.
/// For FPU store-to-memory instructions (fst, fstp, etc.), the ST(0) operand
/// is moved to implicit so it appears as an annotation but not in the asm text.
fn split_fpu_implicit(mnemonic: &Mnemonic, operands: Vec<Operand>) -> (Vec<Operand>, Vec<Operand>) {
    use Mnemonic::*;
    match mnemonic {
        Fchs | Fabs | Ftst | Fxam | Fld1 | Fldl2t | Fldl2e | Fldpi | Fldlg2 | Fldln2 | Fldz
        | F2xm1 | Fyl2x | Fptan | Fpatan | Fxtract | Fprem | Fyl2xp1 | Fsqrt | Frndint | Fscale => {
            (vec![], operands)
        }
        // Store-to-memory: keep FpuMem as explicit, move FpuReg(ST0) to implicit
        Fst | Fstp | Fist | Fistp | Fbstp => {
            let mut explicit = vec![];
            let mut implicit = vec![];
            for op in operands {
                match op {
                    Operand::FpuReg(..) => implicit.push(op),
                    _ => explicit.push(op),
                }
            }
            (explicit, implicit)
        }
        _ => (operands, vec![]),
    }
}

// ─── Main dispatch ────────────────────────────────────────────────────────────

fn decode_inner(cur: &mut Cursor) -> (Mnemonic, Vec<Operand>) {
    let op = cur.fetch();
    match op {
        // ── Segment override prefixes (recursive: prefix is part of the instruction) ──
        0x26 => {
            cur.seg_override = Some(SegReg::ES);
            decode_inner(cur)
        }
        0x2E => {
            cur.seg_override = Some(SegReg::CS);
            decode_inner(cur)
        }
        0x36 => {
            cur.seg_override = Some(SegReg::SS);
            decode_inner(cur)
        }
        0x3E => {
            cur.seg_override = Some(SegReg::DS);
            decode_inner(cur)
        }

        // ── MOV ──────────────────────────────────────────────────────────────

        // 88/r  MOV r/m8,  r8
        0x88 => {
            let m = cur.fetch();
            (
                Mnemonic::Mov,
                vec![
                    decode_rm(cur, m, false),
                    decode_reg(cur, (m >> 3) & 7, false),
                ],
            )
        }
        // 89/r  MOV r/m16, r16
        0x89 => {
            let m = cur.fetch();
            (
                Mnemonic::Mov,
                vec![decode_rm(cur, m, true), decode_reg(cur, (m >> 3) & 7, true)],
            )
        }
        // 8A/r  MOV r8,  r/m8
        0x8A => {
            let m = cur.fetch();
            (
                Mnemonic::Mov,
                vec![
                    decode_reg(cur, (m >> 3) & 7, false),
                    decode_rm(cur, m, false),
                ],
            )
        }
        // 8B/r  MOV r16, r/m16
        0x8B => {
            let m = cur.fetch();
            (
                Mnemonic::Mov,
                vec![decode_reg(cur, (m >> 3) & 7, true), decode_rm(cur, m, true)],
            )
        }
        // 8C/r  MOV r/m16, Sreg  (represent Sreg value as Imm16 — no dedicated SegReg operand)
        0x8C => {
            let m = cur.fetch();
            let sv = match (m >> 3) & 3 {
                0 => cur.cpu.es(),
                1 => cur.cpu.cs(),
                2 => cur.cpu.ss(),
                _ => cur.cpu.ds(),
            };
            (
                Mnemonic::Mov,
                vec![decode_rm(cur, m, true), Operand::Imm16(sv)],
            )
        }
        // 8E/r  MOV Sreg, r/m16  (represent Sreg as Imm16)
        0x8E => {
            let m = cur.fetch();
            (Mnemonic::Mov, vec![decode_rm(cur, m, true)])
        }

        // B0..B7  MOV r8, imm8
        0xB0..=0xB7 => {
            let r = Reg8::from_bits(op & 7);
            let imm = cur.fetch();
            (
                Mnemonic::Mov,
                vec![Operand::Reg8(r, r.value(cur.cpu)), Operand::Imm8(imm)],
            )
        }
        // B8..BF  MOV r16, imm16
        0xB8..=0xBF => {
            let r = Reg16::from_bits(op & 7);
            let imm = cur.fetch16();
            (
                Mnemonic::Mov,
                vec![Operand::Reg16(r, r.value(cur.cpu)), Operand::Imm16(imm)],
            )
        }
        // C6/0  MOV r/m8,  imm8
        0xC6 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, false);
            let imm = cur.fetch();
            (Mnemonic::Mov, vec![rm, Operand::Imm8(imm)])
        }
        // C7/0  MOV r/m16, imm16
        0xC7 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, true);
            let imm = cur.fetch16();
            (Mnemonic::Mov, vec![rm, Operand::Imm16(imm)])
        }
        // A0  MOV AL, moffs8
        0xA0 => {
            let ea = cur.fetch16();
            let seg = cur.seg_for_direct();
            let v = cur.read_mem_u8(seg, ea);
            (
                Mnemonic::Mov,
                vec![
                    Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                    Operand::Mem8 {
                        mem: MemRef {
                            seg,
                            ea,
                            expr: format!("0x{:04x}", ea),
                        },
                        value: v,
                    },
                ],
            )
        }
        // A1  MOV AX, moffs16
        0xA1 => {
            let ea = cur.fetch16();
            let seg = cur.seg_for_direct();
            let v = cur.read_mem_u16(seg, ea);
            (
                Mnemonic::Mov,
                vec![
                    Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                    Operand::Mem16 {
                        mem: MemRef {
                            seg,
                            ea,
                            expr: format!("0x{:04x}", ea),
                        },
                        value: v,
                    },
                ],
            )
        }
        // A2  MOV moffs8, AL
        0xA2 => {
            let ea = cur.fetch16();
            let seg = cur.seg_for_direct();
            let v = cur.cpu.ax() as u8;
            (
                Mnemonic::Mov,
                vec![
                    Operand::Mem8 {
                        mem: MemRef {
                            seg,
                            ea,
                            expr: format!("0x{:04x}", ea),
                        },
                        value: v,
                    },
                    Operand::Reg8(Reg8::AL, v),
                ],
            )
        }
        // A3  MOV moffs16, AX
        0xA3 => {
            let ea = cur.fetch16();
            let seg = cur.seg_for_direct();
            let v = cur.cpu.ax();
            (
                Mnemonic::Mov,
                vec![
                    Operand::Mem16 {
                        mem: MemRef {
                            seg,
                            ea,
                            expr: format!("0x{:04x}", ea),
                        },
                        value: v,
                    },
                    Operand::Reg16(Reg16::AX, v),
                ],
            )
        }

        // ── PUSH ─────────────────────────────────────────────────────────────

        // 50..57  PUSH r16
        0x50..=0x57 => {
            let r = Reg16::from_bits(op & 7);
            (Mnemonic::Push, vec![Operand::Reg16(r, r.value(cur.cpu))])
        }
        // PUSH Sreg
        0x06 => (Mnemonic::Push, vec![Operand::Seg(SegReg::ES, cur.cpu.es())]),
        0x0E => (Mnemonic::Push, vec![Operand::Seg(SegReg::CS, cur.cpu.cs())]),
        0x16 => (Mnemonic::Push, vec![Operand::Seg(SegReg::SS, cur.cpu.ss())]),
        0x1E => (Mnemonic::Push, vec![Operand::Seg(SegReg::DS, cur.cpu.ds())]),
        // 68  PUSH imm16  (186+)
        0x68 => {
            let imm = cur.fetch16();
            (Mnemonic::Push, vec![Operand::Imm16(imm)])
        }
        // 6A  PUSH imm8s  (186+)
        0x6A => {
            let imm = cur.fetch() as i8 as u16;
            (Mnemonic::Push, vec![Operand::Imm16(imm)])
        }

        // ── POP ──────────────────────────────────────────────────────────────

        // 58..5F  POP r16
        0x58..=0x5F => {
            let r = Reg16::from_bits(op & 7);
            (Mnemonic::Pop, vec![Operand::Reg16(r, r.value(cur.cpu))])
        }
        // POP Sreg
        0x07 => (Mnemonic::Pop, vec![Operand::Seg(SegReg::ES, cur.cpu.es())]),
        0x17 => (Mnemonic::Pop, vec![Operand::Seg(SegReg::SS, cur.cpu.ss())]),
        0x1F => (Mnemonic::Pop, vec![Operand::Seg(SegReg::DS, cur.cpu.ds())]),

        // ── XCHG ─────────────────────────────────────────────────────────────
        0x90 => (Mnemonic::Nop, vec![]),
        0x91..=0x97 => {
            let r = Reg16::from_bits(op & 7);
            (
                Mnemonic::Xchg,
                vec![
                    Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                    Operand::Reg16(r, r.value(cur.cpu)),
                ],
            )
        }
        // 86/r  XCHG r8,  r/m8
        0x86 => {
            let m = cur.fetch();
            (
                Mnemonic::Xchg,
                vec![
                    decode_reg(cur, (m >> 3) & 7, false),
                    decode_rm(cur, m, false),
                ],
            )
        }
        // 87/r  XCHG r16, r/m16
        0x87 => {
            let m = cur.fetch();
            (
                Mnemonic::Xchg,
                vec![decode_reg(cur, (m >> 3) & 7, true), decode_rm(cur, m, true)],
            )
        }

        // ── INC / DEC (register short forms) ─────────────────────────────────
        0x40..=0x47 => {
            let r = Reg16::from_bits(op & 7);
            (Mnemonic::Inc, vec![Operand::Reg16(r, r.value(cur.cpu))])
        }
        0x48..=0x4F => {
            let r = Reg16::from_bits(op & 7);
            (Mnemonic::Dec, vec![Operand::Reg16(r, r.value(cur.cpu))])
        }

        // ── ALU: ADD/OR/ADC/SBB/AND/SUB/XOR/CMP with r/m, r ─────────────────
        0x00..=0x03 => alu_rm_r(cur, op, Mnemonic::Add),
        0x08..=0x0B => alu_rm_r(cur, op, Mnemonic::Or),
        0x10..=0x13 => alu_rm_r(cur, op, Mnemonic::Adc),
        0x18..=0x1B => alu_rm_r(cur, op, Mnemonic::Sbb),
        0x20..=0x23 => alu_rm_r(cur, op, Mnemonic::And),
        0x28..=0x2B => alu_rm_r(cur, op, Mnemonic::Sub),
        0x30..=0x33 => alu_rm_r(cur, op, Mnemonic::Xor),
        0x38..=0x3B => alu_rm_r(cur, op, Mnemonic::Cmp),

        // ALU: accumulator + immediate short forms
        0x04 => (
            Mnemonic::Add,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x05 => (
            Mnemonic::Add,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x0C => (
            Mnemonic::Or,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x0D => (
            Mnemonic::Or,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x14 => (
            Mnemonic::Adc,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x15 => (
            Mnemonic::Adc,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x1C => (
            Mnemonic::Sbb,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x1D => (
            Mnemonic::Sbb,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x24 => (
            Mnemonic::And,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x25 => (
            Mnemonic::And,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x2C => (
            Mnemonic::Sub,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x2D => (
            Mnemonic::Sub,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x34 => (
            Mnemonic::Xor,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x35 => (
            Mnemonic::Xor,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),
        0x3C => (
            Mnemonic::Cmp,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0x3D => (
            Mnemonic::Cmp,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),

        // ALU immediate groups (80–83)
        0x80 | 0x82 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, false);
            let imm = cur.fetch();
            (alu_group((m >> 3) & 7), vec![rm, Operand::Imm8(imm)])
        }
        0x81 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, true);
            let imm = cur.fetch16();
            (alu_group((m >> 3) & 7), vec![rm, Operand::Imm16(imm)])
        }
        // 83: sign-extend imm8 to 16 bits
        0x83 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, true);
            let imm = cur.fetch() as i8 as u16;
            (alu_group((m >> 3) & 7), vec![rm, Operand::Imm16(imm)])
        }

        // ── TEST ─────────────────────────────────────────────────────────────
        0x84 => {
            let m = cur.fetch();
            (
                Mnemonic::Test,
                vec![
                    decode_rm(cur, m, false),
                    decode_reg(cur, (m >> 3) & 7, false),
                ],
            )
        }
        0x85 => {
            let m = cur.fetch();
            (
                Mnemonic::Test,
                vec![decode_rm(cur, m, true), decode_reg(cur, (m >> 3) & 7, true)],
            )
        }
        0xA8 => (
            Mnemonic::Test,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0xA9 => (
            Mnemonic::Test,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm16(cur.fetch16()),
            ],
        ),

        // ── SHIFT / ROTATE (D0–D3, C0–C1) ────────────────────────────────────
        0xD0 => {
            let m = cur.fetch();
            (
                shift_group((m >> 3) & 7),
                vec![decode_rm(cur, m, false), Operand::Imm8(1)],
            )
        }
        0xD1 => {
            let m = cur.fetch();
            (
                shift_group((m >> 3) & 7),
                vec![decode_rm(cur, m, true), Operand::Imm8(1)],
            )
        }
        0xD2 => {
            let m = cur.fetch();
            (
                shift_group((m >> 3) & 7),
                vec![
                    decode_rm(cur, m, false),
                    Operand::Reg8(Reg8::CL, cur.cpu.cx() as u8),
                ],
            )
        }
        0xD3 => {
            let m = cur.fetch();
            (
                shift_group((m >> 3) & 7),
                vec![
                    decode_rm(cur, m, true),
                    Operand::Reg8(Reg8::CL, cur.cpu.cx() as u8),
                ],
            )
        }
        0xC0 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, false);
            let cnt = cur.fetch();
            (shift_group((m >> 3) & 7), vec![rm, Operand::Imm8(cnt)])
        }
        0xC1 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, true);
            let cnt = cur.fetch();
            (shift_group((m >> 3) & 7), vec![rm, Operand::Imm8(cnt)])
        }

        // ── MUL / DIV / NOT / NEG group (F6/F7) ───────────────────────────────
        0xF6 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, false);
            match (m >> 3) & 7 {
                0 | 1 => {
                    let i = cur.fetch();
                    (Mnemonic::Test, vec![rm, Operand::Imm8(i)])
                }
                2 => (Mnemonic::Not, vec![rm]),
                3 => (Mnemonic::Neg, vec![rm]),
                4 => (Mnemonic::Mul, vec![rm]),
                5 => (Mnemonic::IMul, vec![rm]),
                6 => (Mnemonic::Div, vec![rm]),
                _ => (Mnemonic::IDiv, vec![rm]),
            }
        }
        0xF7 => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, true);
            match (m >> 3) & 7 {
                0 | 1 => {
                    let i = cur.fetch16();
                    (Mnemonic::Test, vec![rm, Operand::Imm16(i)])
                }
                2 => (Mnemonic::Not, vec![rm]),
                3 => (Mnemonic::Neg, vec![rm]),
                4 => (Mnemonic::Mul, vec![rm]),
                5 => (Mnemonic::IMul, vec![rm]),
                6 => (Mnemonic::Div, vec![rm]),
                _ => (Mnemonic::IDiv, vec![rm]),
            }
        }

        // ── INC/DEC/CALL/JMP/PUSH r/m (FE/FF) ───────────────────────────────
        0xFE => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, false);
            match (m >> 3) & 7 {
                0 => (Mnemonic::Inc, vec![rm]),
                1 => (Mnemonic::Dec, vec![rm]),
                _ => (Mnemonic::Unknown(op), vec![rm]),
            }
        }
        0xFF => {
            let m = cur.fetch();
            let rm = decode_rm(cur, m, true);
            match (m >> 3) & 7 {
                0 => (Mnemonic::Inc, vec![rm]),
                1 => (Mnemonic::Dec, vec![rm]),
                2 => (Mnemonic::Call, vec![rm]),
                3 => (Mnemonic::CallFar, vec![rm]),
                4 => (Mnemonic::Jmp, vec![rm]),
                5 => (Mnemonic::JmpFar, vec![rm]),
                6 => (Mnemonic::Push, vec![rm]),
                _ => (Mnemonic::Unknown(op), vec![rm]),
            }
        }

        // ── CALL ─────────────────────────────────────────────────────────────

        // E8 cw  CALL rel16
        0xE8 => {
            let rel = cur.fetch16() as i16;
            let tgt = cur.offset.wrapping_add(rel as u16);
            (Mnemonic::Call, vec![Operand::Imm16(tgt)])
        }
        // 9A cd  CALL ptr16:16
        0x9A => {
            let off = cur.fetch16();
            let seg = cur.fetch16();
            (
                Mnemonic::CallFar,
                vec![Operand::Imm16(seg), Operand::Imm16(off)],
            )
        }

        // ── JMP ──────────────────────────────────────────────────────────────

        // EB cb  JMP rel8
        0xEB => {
            let rel = cur.fetch() as i8 as i16;
            let tgt = cur.offset.wrapping_add(rel as u16);
            (Mnemonic::JmpShort, vec![Operand::Imm16(tgt)])
        }
        // E9 cw  JMP rel16
        0xE9 => {
            let rel = cur.fetch16() as i16;
            let tgt = cur.offset.wrapping_add(rel as u16);
            (Mnemonic::Jmp, vec![Operand::Imm16(tgt)])
        }
        // EA cd  JMP ptr16:16
        0xEA => {
            let off = cur.fetch16();
            let seg = cur.fetch16();
            (
                Mnemonic::JmpFar,
                vec![Operand::Imm16(seg), Operand::Imm16(off)],
            )
        }

        // ── RET ──────────────────────────────────────────────────────────────
        0xC2 => (Mnemonic::Ret, vec![Operand::Imm16(cur.fetch16())]),
        0xC3 => (Mnemonic::Ret, vec![]),
        0xCA => (Mnemonic::RetFar, vec![Operand::Imm16(cur.fetch16())]),
        0xCB => (Mnemonic::RetFar, vec![]),

        // ── Conditional jumps (all rel8) ──────────────────────────────────────
        0x70 => jcc(cur, Mnemonic::Jo),
        0x71 => jcc(cur, Mnemonic::Jno),
        0x72 => jcc(cur, Mnemonic::Jb),
        0x73 => jcc(cur, Mnemonic::Jae),
        0x74 => jcc(cur, Mnemonic::Je),
        0x75 => jcc(cur, Mnemonic::Jne),
        0x76 => jcc(cur, Mnemonic::Jbe),
        0x77 => jcc(cur, Mnemonic::Ja),
        0x78 => jcc(cur, Mnemonic::Js),
        0x79 => jcc(cur, Mnemonic::Jns),
        0x7A => jcc(cur, Mnemonic::Jp),
        0x7B => jcc(cur, Mnemonic::Jnp),
        0x7C => jcc(cur, Mnemonic::Jl),
        0x7D => jcc(cur, Mnemonic::Jge),
        0x7E => jcc(cur, Mnemonic::Jle),
        0x7F => jcc(cur, Mnemonic::Jg),
        0xE0 => jcc(cur, Mnemonic::Loopnz),
        0xE1 => jcc(cur, Mnemonic::Loopz),
        0xE2 => jcc(cur, Mnemonic::Loop),
        0xE3 => jcc(cur, Mnemonic::Jcxz),

        // ── LEA ──────────────────────────────────────────────────────────────
        0x8D => {
            let m = cur.fetch();
            (
                Mnemonic::Lea,
                vec![decode_reg(cur, (m >> 3) & 7, true), decode_rm(cur, m, true)],
            )
        }

        // ── LDS / LES ────────────────────────────────────────────────────────
        0xC4 => {
            let m = cur.fetch();
            (
                Mnemonic::Les,
                vec![decode_reg(cur, (m >> 3) & 7, true), decode_rm(cur, m, true)],
            )
        }
        0xC5 => {
            let m = cur.fetch();
            (
                Mnemonic::Lds,
                vec![decode_reg(cur, (m >> 3) & 7, true), decode_rm(cur, m, true)],
            )
        }

        // ── CBW / CWD ────────────────────────────────────────────────────────
        0x98 => (Mnemonic::Cbw, vec![]),
        0x99 => (Mnemonic::Cwd, vec![]),

        // ── String operations ─────────────────────────────────────────────────
        0xA4 => (Mnemonic::Movsb, vec![]),
        0xA5 => (Mnemonic::Movsw, vec![]),
        0xA6 => (Mnemonic::Cmpsb, vec![]),
        0xA7 => (Mnemonic::Cmpsw, vec![]),
        0xAA => (Mnemonic::Stosb, vec![]),
        0xAB => (Mnemonic::Stosw, vec![]),
        0xAC => (Mnemonic::Lodsb, vec![]),
        0xAD => (Mnemonic::Lodsw, vec![]),
        0xAE => (Mnemonic::Scasb, vec![]),
        0xAF => (Mnemonic::Scasw, vec![]),

        // REP / REPNE prefixes
        0xF2 => (Mnemonic::Repne, vec![]),
        0xF3 => (Mnemonic::Rep, vec![]),

        // ── IN / OUT ─────────────────────────────────────────────────────────
        0xE4 => (
            Mnemonic::In,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0xE5 => (
            Mnemonic::In,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Imm8(cur.fetch()),
            ],
        ),
        0xE6 => (
            Mnemonic::Out,
            vec![
                Operand::Imm8(cur.fetch()),
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
            ],
        ),
        0xE7 => (
            Mnemonic::Out,
            vec![
                Operand::Imm8(cur.fetch()),
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
            ],
        ),
        0xEC => (
            Mnemonic::In,
            vec![
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
                Operand::Reg16(Reg16::DX, cur.cpu.dx()),
            ],
        ),
        0xED => (
            Mnemonic::In,
            vec![
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
                Operand::Reg16(Reg16::DX, cur.cpu.dx()),
            ],
        ),
        0xEE => (
            Mnemonic::Out,
            vec![
                Operand::Reg16(Reg16::DX, cur.cpu.dx()),
                Operand::Reg8(Reg8::AL, cur.cpu.ax() as u8),
            ],
        ),
        0xEF => (
            Mnemonic::Out,
            vec![
                Operand::Reg16(Reg16::DX, cur.cpu.dx()),
                Operand::Reg16(Reg16::AX, cur.cpu.ax()),
            ],
        ),

        // ── XLAT ─────────────────────────────────────────────────────────────
        0xD7 => (Mnemonic::Xlat, vec![]),

        // ── Miscellaneous ─────────────────────────────────────────────────────
        0x9C => (Mnemonic::Pushf, vec![]),
        0x9D => (Mnemonic::Popf, vec![]),
        0x9E => (Mnemonic::Sahf, vec![]),
        0x9F => (Mnemonic::Lahf, vec![]),
        0xCC => (Mnemonic::Int3, vec![]),
        0xCD => {
            let n = cur.fetch();
            (Mnemonic::Int, vec![Operand::Imm8(n)])
        }
        0xCE => (Mnemonic::Into, vec![]),
        0xCF => (Mnemonic::Iret, vec![]),
        0xF0 => (Mnemonic::Lock, vec![]),
        0xF4 => (Mnemonic::Hlt, vec![]),
        0xF5 => (Mnemonic::Cmc, vec![]),
        0xF8 => (Mnemonic::Clc, vec![]),
        0xF9 => (Mnemonic::Stc, vec![]),
        0xFA => (Mnemonic::Cli, vec![]),
        0xFB => (Mnemonic::Sti, vec![]),
        0xFC => (Mnemonic::Cld, vec![]),
        0xFD => (Mnemonic::Std, vec![]),
        0x9B => (Mnemonic::Wait, vec![]),
        0x37 => (Mnemonic::Aaa, vec![]),
        0x27 => (Mnemonic::Daa, vec![]),
        0x3F => (Mnemonic::Aas, vec![]),
        0x2F => (Mnemonic::Das, vec![]),
        0xD4 => {
            cur.fetch();
            (Mnemonic::Aam, vec![])
        } // operand byte (usually 0x0A)
        0xD5 => {
            cur.fetch();
            (Mnemonic::Aad, vec![])
        }

        // ESC — 8087 FPU instructions (D8–DF)
        0xD8..=0xDF => fpu::decode_fpu(cur, op),

        _ => (Mnemonic::Unknown(op), vec![]),
    }
}

// ─── Small helpers ────────────────────────────────────────────────────────────

/// ALU with r/m and r — direction and size encoded in low 2 bits of `op`.
fn alu_rm_r(cur: &mut Cursor, op: u8, mnemo: Mnemonic) -> (Mnemonic, Vec<Operand>) {
    let dir = (op >> 1) & 1; // 0 = rm is dst, 1 = reg is dst
    let is16 = (op & 1) != 0;
    let m = cur.fetch();
    let rm = decode_rm(cur, m, is16);
    let reg = decode_reg(cur, (m >> 3) & 7, is16);
    if dir == 0 {
        (mnemo, vec![rm, reg])
    } else {
        (mnemo, vec![reg, rm])
    }
}

/// Map the /r field of opcodes 80–83 to the right ALU mnemonic.
fn alu_group(reg: u8) -> Mnemonic {
    match reg {
        0 => Mnemonic::Add,
        1 => Mnemonic::Or,
        2 => Mnemonic::Adc,
        3 => Mnemonic::Sbb,
        4 => Mnemonic::And,
        5 => Mnemonic::Sub,
        6 => Mnemonic::Xor,
        _ => Mnemonic::Cmp,
    }
}

/// Map the /r field of D0–D3 / C0–C1 to the right shift/rotate mnemonic.
fn shift_group(reg: u8) -> Mnemonic {
    match reg {
        0 => Mnemonic::Rol,
        1 => Mnemonic::Ror,
        2 => Mnemonic::Rcl,
        3 => Mnemonic::Rcr,
        4 | 6 => Mnemonic::Shl,
        5 => Mnemonic::Shr,
        _ => Mnemonic::Sar,
    }
}

/// Conditional jump with a rel8 target (post-fetch IP + sign-extended displacement).
fn jcc(cur: &mut Cursor, mnemo: Mnemonic) -> (Mnemonic, Vec<Operand>) {
    let rel = cur.fetch() as i8 as i16;
    let tgt = cur.offset.wrapping_add(rel as u16);
    (mnemo, vec![Operand::Imm16(tgt)])
}

// ─── Default comment derivation ───────────────────────────────────────────────

fn default_comment(
    mnemonic: &Mnemonic,
    operands: &[Operand],
    cpu: &dyn Computer,
) -> Option<String> {
    match mnemonic {
        Mnemonic::In => {
            // IN acc, port — port is operand[1]
            port_comment(operands.get(1))
        }
        Mnemonic::Out => {
            // OUT port, acc — port is operand[0]
            port_comment(operands.first())
        }
        Mnemonic::Int => {
            if let Some(Operand::Imm8(n)) = operands.first() {
                let ah = (cpu.ax() >> 8) as u8;
                int_description(*n, ah).map(|(desc, show_ah)| {
                    if show_ah {
                        format!("INT {:02X}h AH={:02X}h: {}", n, ah, desc)
                    } else {
                        format!("INT {:02X}h: {}", n, desc)
                    }
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

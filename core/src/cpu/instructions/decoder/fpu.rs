use crate::Computer;

use super::{Cursor, MemRef, Mnemonic, Operand, Reg16, decode_mem_ref};

pub(super) fn decode_fpu(cur: &mut Cursor, opcode: u8) -> (Mnemonic, Vec<Operand>) {
    let modrm = cur.fetch();
    let mode = (modrm >> 6) & 3;
    let reg = (modrm >> 3) & 7;
    let rm = modrm & 7;

    if mode == 0b11 {
        fpu_reg_decode(opcode, reg, rm, cur.cpu)
    } else {
        let mem = decode_mem_ref(cur, modrm);
        fpu_mem_decode(opcode, reg, mem, cur.cpu)
    }
}

fn st(cpu: &dyn Computer, i: u8) -> Operand {
    let (raw, f) = cpu.fpu_st(i);
    Operand::FpuReg(i, raw, f)
}

/// Like `st` but uses pre-execution values — for annotating store instructions.
fn st_pre(cpu: &dyn Computer, i: u8) -> Operand {
    let (raw, f) = cpu.fpu_st_pre(i);
    Operand::FpuReg(i, raw, f)
}

fn fpu_reg_decode(opcode: u8, reg: u8, rm: u8, cpu: &dyn Computer) -> (Mnemonic, Vec<Operand>) {
    use Mnemonic::*;
    match (opcode, reg, rm) {
        (0xDB, 4, 3) => (Fninit, vec![]),
        (0xDB, 4, 2) => (Fnclex, vec![]),
        (0xDF, 4, 0) => (Fnstsw, vec![Operand::Reg16(Reg16::AX, cpu.ax())]),
        (0xD9, 0, i) => (Fld, vec![st(cpu, i)]),
        (0xD9, 1, i) => (Fxch, vec![st(cpu, i)]),
        (0xD9, 4, 0) => (Fchs, vec![st(cpu, 0)]),
        (0xD9, 4, 1) => (Fabs, vec![st(cpu, 0)]),
        (0xD9, 4, 4) => (Ftst, vec![st(cpu, 0)]),
        (0xD9, 4, 5) => (Fxam, vec![st(cpu, 0)]),
        (0xD9, 5, 0) => (Fld1, vec![st(cpu, 0)]),
        (0xD9, 5, 1) => (Fldl2t, vec![st(cpu, 0)]),
        (0xD9, 5, 2) => (Fldl2e, vec![st(cpu, 0)]),
        (0xD9, 5, 3) => (Fldpi, vec![st(cpu, 0)]),
        (0xD9, 5, 4) => (Fldlg2, vec![st(cpu, 0)]),
        (0xD9, 5, 5) => (Fldln2, vec![st(cpu, 0)]),
        (0xD9, 5, 6) => (Fldz, vec![st(cpu, 0)]),
        (0xD9, 6, 0) => (F2xm1, vec![st(cpu, 0)]),
        (0xD9, 6, 1) => (Fyl2x, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD9, 6, 2) => (Fptan, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD9, 6, 3) => (Fpatan, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD9, 6, 4) => (Fxtract, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD9, 6, 6) => (Fdecstp, vec![]),
        (0xD9, 6, 7) => (Fincstp, vec![]),
        (0xD9, 7, 0) => (Fprem, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD9, 7, 1) => (Fyl2xp1, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD9, 7, 2) => (Fsqrt, vec![st(cpu, 0)]),
        (0xD9, 7, 4) => (Frndint, vec![st(cpu, 0)]),
        (0xD9, 7, 5) => (Fscale, vec![st(cpu, 0), st(cpu, 1)]),
        (0xD8, 0, i) => (Fadd, vec![st(cpu, 0), st(cpu, i)]),
        (0xD8, 1, i) => (Fmul, vec![st(cpu, 0), st(cpu, i)]),
        (0xD8, 2, i) => (Fcom, vec![st(cpu, i)]),
        (0xD8, 3, i) => (Fcomp, vec![st(cpu, i)]),
        (0xD8, 4, i) => (Fsub, vec![st(cpu, 0), st(cpu, i)]),
        (0xD8, 5, i) => (Fsubr, vec![st(cpu, 0), st(cpu, i)]),
        (0xD8, 6, i) => (Fdiv, vec![st(cpu, 0), st(cpu, i)]),
        (0xD8, 7, i) => (Fdivr, vec![st(cpu, 0), st(cpu, i)]),
        (0xDC, 0, i) => (Fadd, vec![st(cpu, i), st(cpu, 0)]),
        (0xDC, 1, i) => (Fmul, vec![st(cpu, i), st(cpu, 0)]),
        (0xDC, 4, i) => (Fsubr, vec![st(cpu, i), st(cpu, 0)]),
        (0xDC, 5, i) => (Fsub, vec![st(cpu, i), st(cpu, 0)]),
        (0xDC, 6, i) => (Fdivr, vec![st(cpu, i), st(cpu, 0)]),
        (0xDC, 7, i) => (Fdiv, vec![st(cpu, i), st(cpu, 0)]),
        (0xDD, 0, i) => (Ffree, vec![st(cpu, i)]),
        (0xDD, 3, i) => (Fstp, vec![st(cpu, i)]),
        (0xDE, 0, i) => (Faddp, vec![st(cpu, i), st(cpu, 0)]),
        (0xDE, 1, i) => (Fmulp, vec![st(cpu, i), st(cpu, 0)]),
        (0xDE, 3, 1) => (Fcompp, vec![]),
        (0xDE, 4, i) => (Fsubrp, vec![st(cpu, i), st(cpu, 0)]),
        (0xDE, 5, i) => (Fsubp, vec![st(cpu, i), st(cpu, 0)]),
        (0xDE, 6, i) => (Fdivrp, vec![st(cpu, i), st(cpu, 0)]),
        (0xDE, 7, i) => (Fdivp, vec![st(cpu, i), st(cpu, 0)]),
        _ => (Mnemonic::Unknown(opcode), vec![]),
    }
}

fn fpu_mem_decode(
    opcode: u8,
    reg: u8,
    mem: MemRef,
    cpu: &dyn Computer,
) -> (Mnemonic, Vec<Operand>) {
    use Mnemonic::*;
    let (mnemonic, bytes): (Mnemonic, u8) = match (opcode, reg) {
        (0xD8, 0) => (Fadd, 4),
        (0xD8, 1) => (Fmul, 4),
        (0xD8, 2) => (Fcom, 4),
        (0xD8, 3) => (Fcomp, 4),
        (0xD8, 4) => (Fsub, 4),
        (0xD8, 5) => (Fsubr, 4),
        (0xD8, 6) => (Fdiv, 4),
        (0xD8, 7) => (Fdivr, 4),
        (0xD9, 0) => (Fld, 4),
        (0xD9, 2) => (Fst, 4),
        (0xD9, 3) => (Fstp, 4),
        (0xD9, 4) => (Fldenv, 14),
        (0xD9, 5) => (Fldcw, 2),
        (0xD9, 6) => (Fnstenv, 14),
        (0xD9, 7) => (Fnstcw, 2),
        (0xDA, 0) => (Fiadd, 4),
        (0xDA, 1) => (Fimul, 4),
        (0xDA, 2) => (Ficom, 4),
        (0xDA, 3) => (Ficomp, 4),
        (0xDA, 4) => (Fisub, 4),
        (0xDA, 5) => (Fisubr, 4),
        (0xDA, 6) => (Fidiv, 4),
        (0xDA, 7) => (Fidivr, 4),
        (0xDB, 0) => (Fild, 4),
        (0xDB, 2) => (Fist, 4),
        (0xDB, 3) => (Fistp, 4),
        (0xDB, 5) => (Fld, 10),
        (0xDB, 7) => (Fstp, 10),
        (0xDE, 1) => (Fimul, 2),
        (0xDC, 0) => (Fadd, 8),
        (0xDC, 1) => (Fmul, 8),
        (0xDC, 2) => (Fcom, 8),
        (0xDC, 3) => (Fcomp, 8),
        (0xDC, 4) => (Fsub, 8),
        (0xDC, 5) => (Fsubr, 8),
        (0xDC, 6) => (Fdiv, 8),
        (0xDC, 7) => (Fdivr, 8),
        (0xDD, 0) => (Fld, 8),
        (0xDD, 2) => (Fst, 8),
        (0xDD, 3) => (Fstp, 8),
        (0xDD, 4) => (Frstor, 94),
        (0xDD, 6) => (Fnsave, 94),
        (0xDD, 7) => (Fnstsw, 2),
        (0xDF, 0) => (Fild, 2),
        (0xDF, 2) => (Fist, 2),
        (0xDF, 3) => (Fistp, 2),
        (0xDF, 4) => (Fbld, 10),
        (0xDF, 5) => (Fild, 8),
        (0xDF, 6) => (Fbstp, 10),
        (0xDF, 7) => (Fistp, 8),
        _ => return (Mnemonic::Unknown(opcode), vec![]),
    };
    let mut ops = vec![Operand::FpuMem { mem, bytes }];
    // For store instructions, include pre-exec ST(0) so the annotation shows
    // the value that was actually stored (before any pop).
    match mnemonic {
        Fst | Fstp | Fist | Fistp | Fbstp => ops.push(st_pre(cpu, 0)),
        _ => {}
    }
    (mnemonic, ops)
}

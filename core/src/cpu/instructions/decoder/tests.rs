//! Tests for the instruction decoder's log-line formatting.
//!
//! Every test uses `decode_line` (which calls `format_line`) so both the asm
//! column and the annotation column are exercised together.
//!
//! Key invariant: memory operands show the **symbolic** addressing expression
//! in the asm column (e.g. `[bx]`) while the annotation column always shows
//! the **resolved** effective address and value (e.g. `[0x0d1b]=0d5d`).

use super::decode;
use crate::Computer;

// ─── Test helper ─────────────────────────────────────────────────────────────

/// A minimal `Computer` backed by a byte slice with configurable registers.
struct FakeCpu<'a> {
    mem: &'a [u8],
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    cs: u16,
    ds: u16,
    ss: u16,
    es: u16,
    /// FPU ST values: (raw 10-byte representation, f64 approximation)
    fpu_st: [([u8; 10], f64); 8],
}

impl<'a> FakeCpu<'a> {
    fn new(mem: &'a [u8]) -> Self {
        Self {
            mem,
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            sp: 0,
            bp: 0,
            si: 0,
            di: 0,
            cs: 0,
            ds: 0,
            ss: 0,
            es: 0,
            fpu_st: [([0; 10], 0.0); 8],
        }
    }
}

impl Computer for FakeCpu<'_> {
    fn ax(&self) -> u16 {
        self.ax
    }
    fn bx(&self) -> u16 {
        self.bx
    }
    fn cx(&self) -> u16 {
        self.cx
    }
    fn dx(&self) -> u16 {
        self.dx
    }
    fn sp(&self) -> u16 {
        self.sp
    }
    fn bp(&self) -> u16 {
        self.bp
    }
    fn si(&self) -> u16 {
        self.si
    }
    fn di(&self) -> u16 {
        self.di
    }
    fn cs(&self) -> u16 {
        self.cs
    }
    fn ds(&self) -> u16 {
        self.ds
    }
    fn ss(&self) -> u16 {
        self.ss
    }
    fn es(&self) -> u16 {
        self.es
    }
    fn read_u8(&self, phys: u32) -> u8 {
        self.mem.get(phys as usize).copied().unwrap_or(0)
    }
    fn fpu_st(&self, i: u8) -> ([u8; 10], f64) {
        self.fpu_st[i as usize & 7]
    }
}

/// Decode at `seg:off` and return the full formatted log line.
fn decode_line(cpu: &dyn Computer, seg: u16, off: u16) -> String {
    decode(cpu, seg, off).format_line()
}

// ─── mod=00 (no displacement) ─────────────────────────────────────────────────

#[test]
fn add_bx_indirect_si() {
    // 01 37  →  add [bx], si
    // Reproduces the original bug: was showing `add [0x0d1b], si`.
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x01;
    mem[1] = 0x37;
    mem[0x0d1b] = 0x5d;
    mem[0x0d1c] = 0x0d; // value at DS:BX = 0x0d5d
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0d1b;
    cpu.si = 0x0b4b;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("add [bx], si"), "asm column: {line}");
    assert!(line.contains("[0x0d1b]=0d5d"), "EA annotation: {line}");
    assert!(line.contains("SI=0B4B"), "SI annotation: {line}");
}

#[test]
fn mov_ax_bx_si_indirect() {
    // 8B 00  →  mov ax, [bx+si]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x00;
    mem[0x0010] = 0x34;
    mem[0x0011] = 0x12; // value at DS:(BX+SI) = 0x1234
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0005;
    cpu.si = 0x000b;
    cpu.ax = 0x1234;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bx+si]"), "asm column: {line}");
    assert!(line.contains("[0x0010]=1234"), "EA annotation: {line}");
    assert!(line.contains("AX=1234"), "AX annotation: {line}");
}

#[test]
fn mov_ax_bx_di_indirect() {
    // 8B 01  →  mov ax, [bx+di]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x01;
    mem[0x0020] = 0xAB;
    mem[0x0021] = 0x00; // value = 0x00AB
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0010;
    cpu.di = 0x0010;
    cpu.ax = 0x00AB;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bx+di]"), "asm column: {line}");
    assert!(line.contains("[0x0020]=00ab"), "EA annotation: {line}");
}

#[test]
fn mov_ax_bp_si_indirect() {
    // 8B 02  →  mov ax, [bp+si]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x02;
    let mut cpu = FakeCpu::new(&mem);
    // bp+si = 0 → reads from DS:0000 (which holds the instruction bytes 8B 00)
    // value = 0x028B (little-endian: lo=0x8B hi=0x02)
    cpu.ax = 0x028B;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bp+si]"), "asm column: {line}");
    assert!(line.contains("AX=028B"), "AX annotation: {line}");
}

#[test]
fn mov_ax_bp_di_indirect() {
    // 8B 03  →  mov ax, [bp+di]
    let mem = [0x8B, 0x03];
    let cpu = FakeCpu::new(&mem);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bp+di]"), "asm column: {line}");
}

#[test]
fn mov_ax_si_indirect() {
    // 8B 04  →  mov ax, [si]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x04;
    mem[0x0200] = 0x78;
    mem[0x0201] = 0x56; // value at DS:SI = 0x5678
    let mut cpu = FakeCpu::new(&mem);
    cpu.si = 0x0200;
    cpu.ax = 0x5678;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [si]"), "asm column: {line}");
    assert!(line.contains("[0x0200]=5678"), "EA annotation: {line}");
    assert!(line.contains("AX=5678"), "AX annotation: {line}");
}

#[test]
fn mov_ax_di_indirect() {
    // 8B 05  →  mov ax, [di]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x05;
    mem[0x0300] = 0x01;
    mem[0x0301] = 0x02; // value = 0x0201
    let mut cpu = FakeCpu::new(&mem);
    cpu.di = 0x0300;
    cpu.ax = 0x0201;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [di]"), "asm column: {line}");
    assert!(line.contains("[0x0300]=0201"), "EA annotation: {line}");
}

#[test]
fn mov_ax_direct_address() {
    // 8B 06 34 12  →  mov ax, [0x1234]   (mod=00, rm=110 special case)
    let mut mem = vec![0u8; 0x2000];
    mem[0] = 0x8B;
    mem[1] = 0x06;
    mem[2] = 0x34;
    mem[3] = 0x12;
    mem[0x1234] = 0xCD;
    mem[0x1235] = 0xAB; // value = 0xABCD
    let mut cpu = FakeCpu::new(&mem);
    cpu.ax = 0xABCD;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [0x1234]"), "asm column: {line}");
    assert!(line.contains("[0x1234]=abcd"), "EA annotation: {line}");
}

#[test]
fn mov_ax_bx_indirect() {
    // 8B 07  →  mov ax, [bx]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x07;
    mem[0x0400] = 0xFF;
    mem[0x0401] = 0x00; // value = 0x00FF
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0400;
    cpu.ax = 0x00FF;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bx]"), "asm column: {line}");
    assert!(line.contains("[0x0400]=00ff"), "EA annotation: {line}");
    assert!(line.contains("AX=00FF"), "AX annotation: {line}");
}

// ─── mod=01 (8-bit displacement) ─────────────────────────────────────────────

#[test]
fn mov_ax_bx_plus_disp8_positive() {
    // 8B 47 04  →  mov ax, [bx+0x04]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x47;
    mem[2] = 0x04;
    mem[0x0104] = 0xBB;
    mem[0x0105] = 0xAA; // value = 0xAABB
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0100;
    cpu.ax = 0xAABB;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bx+0x04]"), "asm column: {line}");
    assert!(line.contains("[0x0104]=aabb"), "EA annotation: {line}");
    assert!(line.contains("AX=AABB"), "AX annotation: {line}");
}

#[test]
fn mov_ax_bx_minus_disp8() {
    // 8B 47 FC  →  mov ax, [bx-0x04]   (FC = -4 as i8)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x47;
    mem[2] = 0xFC;
    mem[0x00FC] = 0x11;
    mem[0x00FD] = 0x22; // value = 0x2211
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0100;
    cpu.ax = 0x2211;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bx-0x04]"), "asm column: {line}");
    assert!(line.contains("[0x00fc]=2211"), "EA annotation: {line}");
}

#[test]
fn mov_ax_bp_plus_disp8() {
    // 8B 46 10  →  mov ax, [bp+0x10]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x46;
    mem[2] = 0x10;
    mem[0x0110] = 0x99;
    mem[0x0111] = 0x88; // value = 0x8899
    let mut cpu = FakeCpu::new(&mem);
    cpu.bp = 0x0100;
    cpu.ax = 0x8899;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bp+0x10]"), "asm column: {line}");
    assert!(line.contains("[0x0110]=8899"), "EA annotation: {line}");
}

// ─── mod=10 (16-bit displacement) ────────────────────────────────────────────

#[test]
fn mov_ax_bx_plus_disp16() {
    // 8B 87 00 01  →  mov ax, [bx+0x0100]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x87;
    mem[2] = 0x00;
    mem[3] = 0x01;
    mem[0x0200] = 0x55;
    mem[0x0201] = 0x44; // value = 0x4455
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0100;
    cpu.ax = 0x4455;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [bx+0x0100]"), "asm column: {line}");
    assert!(line.contains("[0x0200]=4455"), "EA annotation: {line}");
}

#[test]
fn mov_ax_si_plus_disp16() {
    // 8B 84 20 00  →  mov ax, [si+0x0020]
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x8B;
    mem[1] = 0x84;
    mem[2] = 0x20;
    mem[3] = 0x00;
    mem[0x0120] = 0x66;
    mem[0x0121] = 0x77; // value = 0x7766
    let mut cpu = FakeCpu::new(&mem);
    cpu.si = 0x0100;
    cpu.ax = 0x7766;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [si+0x0020]"), "asm column: {line}");
    assert!(line.contains("[0x0120]=7766"), "EA annotation: {line}");
}

// ─── mod=11 (register — no memory, annotation shows register values) ──────────

#[test]
fn add_ax_bx_register() {
    // 01 D8  →  add ax, bx
    let mut cpu = FakeCpu::new(&[0x01, 0xD8]);
    cpu.ax = 0x0002;
    cpu.bx = 0x0001;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("add ax, bx"), "asm column: {line}");
    assert!(line.contains("AX=0002"), "AX annotation: {line}");
    assert!(line.contains("BX=0001"), "BX annotation: {line}");
}

// ─── moffs (A0-A3) ───────────────────────────────────────────────────────────

#[test]
fn mov_al_moffs8() {
    // A0 34 12  →  mov al, [0x1234]
    let mut mem = vec![0u8; 0x2000];
    mem[0] = 0xA0;
    mem[1] = 0x34;
    mem[2] = 0x12;
    mem[0x1234] = 0x42;
    let mut cpu = FakeCpu::new(&mem);
    cpu.ax = 0x42;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov al, [0x1234]"), "asm column: {line}");
    assert!(line.contains("[0x1234]=42"), "EA annotation: {line}");
}

#[test]
fn mov_ax_moffs16() {
    // A1 78 56  →  mov ax, [0x5678]
    let mut mem = vec![0u8; 0x6000];
    mem[0] = 0xA1;
    mem[1] = 0x78;
    mem[2] = 0x56;
    mem[0x5678] = 0xEF;
    mem[0x5679] = 0xBE; // value = 0xBEEF
    let mut cpu = FakeCpu::new(&mem);
    cpu.ax = 0xBEEF;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov ax, [0x5678]"), "asm column: {line}");
    assert!(line.contains("[0x5678]=beef"), "EA annotation: {line}");
}

#[test]
fn mov_moffs8_al() {
    // A2 34 12  →  mov [0x1234], al
    let mut mem = vec![0u8; 0x2000];
    mem[0] = 0xA2;
    mem[1] = 0x34;
    mem[2] = 0x12;
    mem[0x1234] = 0x7F;
    let mut cpu = FakeCpu::new(&mem);
    cpu.ax = 0x7F;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov [0x1234], al"), "asm column: {line}");
    assert!(line.contains("[0x1234]=7f"), "EA annotation: {line}");
    assert!(line.contains("AL=7F"), "AL annotation: {line}");
}

#[test]
fn mov_moffs16_ax() {
    // A3 78 56  →  mov [0x5678], ax
    let mut mem = vec![0u8; 0x6000];
    mem[0] = 0xA3;
    mem[1] = 0x78;
    mem[2] = 0x56;
    mem[0x5678] = 0x34;
    mem[0x5679] = 0x12; // value = 0x1234
    let mut cpu = FakeCpu::new(&mem);
    cpu.ax = 0x1234;

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("mov [0x5678], ax"), "asm column: {line}");
    assert!(line.contains("[0x5678]=1234"), "EA annotation: {line}");
    assert!(line.contains("AX=1234"), "AX annotation: {line}");
}

// ─── lds: asm expr + full annotation ─────────────────────────────────────────

#[test]
fn lds_bx_direct() {
    // C5 1E BE 03  →  lds bx, [0x03be]
    // ModRM 1E: mod=00, reg=011(BX), rm=110(disp16)
    // Mirrors: `lds bx, [0x03be]   BX=0D1B [0x03be]=0d1b @0108:03BE(0143E)`
    let mut mem = vec![0u8; 0x10000];
    // Instruction at seg=0x0108, off=0x03be → phys = 0x0108<<4 + 0x03be = 0x143e
    let phys_ip: usize = (0x0108u32 << 4) as usize + 0x03be;
    mem[phys_ip] = 0xC5;
    mem[phys_ip + 1] = 0x1E;
    mem[phys_ip + 2] = 0xBE;
    mem[phys_ip + 3] = 0x03;
    // Value at DS:0x03be (DS=0 → phys 0x03be)
    mem[0x03be] = 0x1b;
    mem[0x03bf] = 0x0d; // word = 0x0d1b
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0d1b;

    let line = decode_line(&cpu, 0x0108, 0x03be);
    assert!(line.starts_with("0108:03BE"), "location prefix: {line}");
    assert!(line.contains("lds bx, [0x03be]"), "asm column: {line}");
    assert!(line.contains("BX=0D1B"), "BX annotation: {line}");
    assert!(line.contains("[0x03be]=0d1b"), "EA annotation: {line}");
}

// ─── PUSH/POP segment registers ───────────────────────────────────────────────

#[test]
fn push_cs() {
    // 0E  →  push cs   (was incorrectly shown as `push 0x275d`)
    let mut mem = vec![0u8; 0x28000];
    let phys: usize = (0x275Du32 << 4) as usize + 0x01B1;
    mem[phys] = 0x0E;
    let mut cpu = FakeCpu::new(&mem);
    cpu.cs = 0x275D;
    let line = decode_line(&cpu, 0x275D, 0x01B1);
    assert!(line.contains("push cs"), "asm column: {line}");
    assert!(line.contains("CS=275D"), "CS annotation: {line}");
}

#[test]
fn push_es() {
    // 06  →  push es
    let mut cpu = FakeCpu::new(&[0x06]);
    cpu.es = 0x1234;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("push es"), "asm column: {line}");
    assert!(line.contains("ES=1234"), "ES annotation: {line}");
}

#[test]
fn push_ss() {
    // 16  →  push ss
    let mut cpu = FakeCpu::new(&[0x16]);
    cpu.ss = 0xABCD;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("push ss"), "asm column: {line}");
    assert!(line.contains("SS=ABCD"), "SS annotation: {line}");
}

#[test]
fn push_ds() {
    // 1E  →  push ds
    let mut cpu = FakeCpu::new(&[0x1E]);
    cpu.ds = 0x5678;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("push ds"), "asm column: {line}");
    assert!(line.contains("DS=5678"), "DS annotation: {line}");
}

// ─── JMP short vs near ────────────────────────────────────────────────────────

#[test]
fn jmp_short() {
    // EB 01  →  jmp short 0x01b1   (was shown as `jmp 0x01b1`, indistinct from near)
    // At 275D:01AE: after fetching 2 bytes IP=01B0, target=01B0+01=01B1
    let mut mem = vec![0u8; 0x28000];
    let phys: usize = (0x275Du32 << 4) as usize + 0x01AE;
    mem[phys] = 0xEB;
    mem[phys + 1] = 0x01;
    let mut cpu = FakeCpu::new(&mem);
    cpu.cs = 0x275D;
    let line = decode_line(&cpu, 0x275D, 0x01AE);
    assert!(line.contains("jmp short 0x01b1"), "asm column: {line}");
}

#[test]
fn jmp_near() {
    // E9 03 00  →  jmp 0x0006   (near jump — no "short")
    let cpu = FakeCpu::new(&[0xE9, 0x03, 0x00]);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("jmp 0x0006"), "asm column: {line}");
    assert!(!line.contains("short"), "should not say short: {line}");
}

// ─── FPU: mod=11 (register operands) ─────────────────────────────────────────

#[test]
fn fpu_fninit_no_operands() {
    // DB E3  →  fninit   (mod=11, reg=4, rm=3 → no operands)
    let cpu = FakeCpu::new(&[0xDB, 0xE3]);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fninit"), "asm column: {line}");
}

#[test]
fn fpu_fld_st2_single_reg_operand() {
    // D9 C2  →  fld st(2)   (mod=11, reg=0, rm=2)
    let cpu = FakeCpu::new(&[0xD9, 0xC2]);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fld st(2)"), "asm column: {line}");
}

#[test]
fn fpu_fadd_d8_st0_sti_operand_order() {
    // D8 C2  →  fadd st(0), st(2)   (D8: dest=st(0), src=st(i))
    let cpu = FakeCpu::new(&[0xD8, 0xC2]);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fadd st(0), st(2)"), "asm column: {line}");
}

#[test]
fn fpu_fadd_dc_sti_st0_operand_order() {
    // DC C3  →  fadd st(3), st(0)   (DC: dest=st(i), src=st(0) — reversed from D8)
    let cpu = FakeCpu::new(&[0xDC, 0xC3]);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fadd st(3), st(0)"), "asm column: {line}");
}

#[test]
fn fpu_fnstsw_ax_reg16_operand() {
    // DF E0  →  fnstsw ax   (mod=11, reg=4, rm=0 → Reg16 operand)
    let mut cpu = FakeCpu::new(&[0xDF, 0xE0]);
    cpu.ax = 0x4200;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fnstsw ax"), "asm column: {line}");
    assert!(line.contains("AX=4200"), "AX annotation: {line}");
}

#[test]
fn fpu_fcompp_no_operand_special_encoding() {
    // DE D9  →  fcompp   (mod=11, reg=3, rm=1 — only one valid rm in this slot)
    let cpu = FakeCpu::new(&[0xDE, 0xD9]);
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fcompp"), "asm column: {line}");
}

// ─── FPU: mod!=11 (memory operands, various widths) ───────────────────────────

#[test]
fn fpu_fadd_dword_mem() {
    // D8 07  →  fadd dword [bx]   (D8/reg=0 → 4-byte float)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xD8;
    mem[1] = 0x07; // modrm: mod=00, reg=0, rm=7 (BX)
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0200;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fadd dword [bx]"), "asm column: {line}");
    assert!(line.contains("@0000:0200"), "EA annotation: {line}");
}

#[test]
fn fpu_fadd_qword_mem() {
    // DC 07  →  fadd qword [bx]   (DC/reg=0 → 8-byte float, same mnemonic as D8)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xDC;
    mem[1] = 0x07;
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0300;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fadd qword [bx]"), "asm column: {line}");
    assert!(line.contains("@0000:0300"), "EA annotation: {line}");
}

#[test]
fn fpu_fldcw_word_mem() {
    // D9 2F  →  fldcw word [bx]   (D9/reg=5 → 2-byte control word)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xD9;
    mem[1] = 0x2F; // modrm: mod=00, reg=5, rm=7 (BX)
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0100;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fldcw word [bx]"), "asm column: {line}");
    assert!(line.contains("@0000:0100"), "EA annotation: {line}");
}

#[test]
fn fpu_fld_tword_mem() {
    // DB 2F  →  fld tword [bx]   (DB/reg=5 → 10-byte extended float)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xDB;
    mem[1] = 0x2F; // modrm: mod=00, reg=5, rm=7 (BX)
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0400;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fld tword [bx]"), "asm column: {line}");
    assert!(line.contains("@0000:0400"), "EA annotation: {line}");
}

#[test]
fn fpu_fnsave_ptr_mem() {
    // DD 37  →  fnsave ptr [bx]   (DD/reg=6 → 94-byte FPU state)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xDD;
    mem[1] = 0x37; // modrm: mod=00, reg=6, rm=7 (BX)
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0500;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fnsave ptr [bx]"), "asm column: {line}");
    assert!(line.contains("@0000:0500"), "EA annotation: {line}");
}

#[test]
fn fpu_fld_dword_mem_with_disp8() {
    // D9 47 04  →  fld dword [bx+0x04]   (mod=01 disp8)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xD9;
    mem[1] = 0x47; // modrm: mod=01, reg=0, rm=7
    mem[2] = 0x04;
    let mut cpu = FakeCpu::new(&mem);
    cpu.bx = 0x0100;
    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fld dword [bx+0x04]"), "asm column: {line}");
    assert!(line.contains("@0000:0104"), "EA annotation: {line}");
}

// ─── FPU: register annotations showing ST values after execution ─────────────

/// Build the 10-byte LE representation and f64 for a known F80 value.
/// F80 layout: bytes[0..8] = mantissa LE, bytes[8..10] = (sign|exponent) LE.
fn make_fpu_st(sign: bool, exp: u16, mant: u64) -> ([u8; 10], f64) {
    let mut raw = [0u8; 10];
    raw[0..8].copy_from_slice(&mant.to_le_bytes());
    let exp_sign = exp | (if sign { 0x8000 } else { 0 });
    raw[8..10].copy_from_slice(&exp_sign.to_le_bytes());
    use crate::cpu::f80::F80;
    let f = F80 { sign, exp, mant }.to_f64();
    (raw, f)
}

#[test]
fn fpu_faddp_shows_st_annotations() {
    // DE C1  →  faddp st(1), st(0)   (mod=11, opcode=DE, reg=0, rm=1)
    // After execution, st(0) and st(1) should appear in annotations.
    let mut cpu = FakeCpu::new(&[0xDE, 0xC1]);
    // ST(0) = 1.0:  exp=0x3FFF, mant=0x8000000000000000
    cpu.fpu_st[0] = make_fpu_st(false, 0x3FFF, 0x8000_0000_0000_0000);
    // ST(1) = 3.5:  exp=0x4000, mant=0xE000000000000000
    cpu.fpu_st[1] = make_fpu_st(false, 0x4000, 0xE000_0000_0000_0000);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("faddp st(1), st(0)"), "asm column: {line}");
    // ST(1) annotation: raw hex = 0x4000E000000000000000(3.5)
    assert!(
        line.contains("st(1)=0x4000E000000000000000"),
        "st(1) raw: {line}"
    );
    assert!(line.contains("(3.5"), "st(1) f64 value: {line}");
    // ST(0) annotation: raw hex = 0x3FFF8000000000000000(1.0)
    assert!(
        line.contains("st(0)=0x3FFF8000000000000000"),
        "st(0) raw: {line}"
    );
    assert!(line.contains("(1.0"), "st(0) f64 value: {line}");
}

#[test]
fn fpu_fmul_d8_shows_st_annotations() {
    // D8 C9  →  fmul st(0), st(1)   (mod=11, opcode=D8, reg=1, rm=1)
    let mut cpu = FakeCpu::new(&[0xD8, 0xC9]);
    // ST(0) = -2.0:  sign=true, exp=0x4000, mant=0x8000000000000000
    cpu.fpu_st[0] = make_fpu_st(true, 0x4000, 0x8000_0000_0000_0000);
    // ST(1) = 0.0 (all zeros)
    cpu.fpu_st[1] = ([0u8; 10], 0.0);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("fmul st(0), st(1)"), "asm column: {line}");
    // ST(0) = -2.0: sign bit set → 0xC000...
    assert!(
        line.contains("st(0)=0xC0008000000000000000"),
        "st(0) raw: {line}"
    );
    assert!(line.contains("(-2.0"), "st(0) f64 value: {line}");
    // ST(1) = 0.0: all zeros
    assert!(
        line.contains("st(1)=0x00000000000000000000"),
        "st(1) raw: {line}"
    );
    assert!(line.contains("(0.0"), "st(1) f64 value: {line}");
}

#[test]
fn fpu_fstp_qword_mem_shows_st0_annotation() {
    // DD 1E 68 6E  →  fstp qword [0x6e68]
    // ST(0) should appear as an implicit annotation (not in the asm column).
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xDD;
    mem[1] = 0x1E;
    mem[2] = 0x68;
    mem[3] = 0x6E;
    let mut cpu = FakeCpu::new(&mem);
    // ST(0) = ~0.942478 (some representative value)
    cpu.fpu_st[0] = make_fpu_st(false, 0x3FFE, 0xF146398F5B4A846A);

    let line = decode_line(&cpu, 0, 0);
    // Asm column should show only the memory operand, not st(0)
    assert!(line.contains("fstp qword [0x6e68]"), "asm column: {line}");
    assert!(
        !line.contains("fstp qword [0x6e68], st"),
        "st(0) should NOT be in asm column: {line}"
    );
    // Memory address annotation
    assert!(
        line.contains("@0000:6E68"),
        "memory address annotation: {line}"
    );
    // ST(0) value annotation
    assert!(
        line.contains("st(0)=0x3FFEF146398F5B4A846A"),
        "st(0) raw: {line}"
    );
    assert!(line.contains("(0.942"), "st(0) f64 approx: {line}");
}

#[test]
fn wait_fpu_folds_onto_one_line() {
    // 9B D9 EB  →  wait fldpi
    // WAIT followed by a FPU escape should be combined: "wait fldpi"
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x9B; // WAIT
    mem[1] = 0xD9; // FPU escape
    mem[2] = 0xEB; // fldpi (reg-form: modrm=0xEB, mode=11 reg=5 rm=3)
    let cpu = FakeCpu::new(&mem);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("wait fldpi"), "asm column: {line}");
}

#[test]
fn wait_alone_stays_on_own_line() {
    // 9B  →  wait  (not followed by FPU escape)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0x9B; // WAIT
    mem[1] = 0x90; // nop (not a FPU escape)
    let cpu = FakeCpu::new(&mem);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("wait"), "asm column: {line}");
    assert!(!line.contains("wait nop"), "should not fold nop: {line}");
}

#[test]
fn rep_movsb_folds_onto_one_line() {
    // F3 A4  →  rep movsb
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xF3; // REP
    mem[1] = 0xA4; // movsb
    let cpu = FakeCpu::new(&mem);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("rep movsb"), "asm column: {line}");
}

#[test]
fn repne_scasb_folds_onto_one_line() {
    // F2 AE  →  repne scasb
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xF2; // REPNE
    mem[1] = 0xAE; // scasb
    let cpu = FakeCpu::new(&mem);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("repne scasb"), "asm column: {line}");
}

#[test]
fn rep_cmpsb_folds_as_repe() {
    // F3 A6  →  repe cmpsb  (0xF3 + CMPS/SCAS means "repeat while equal")
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xF3; // REP/REPE
    mem[1] = 0xA6; // cmpsb
    let cpu = FakeCpu::new(&mem);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("repe cmpsb"), "asm column: {line}");
}

#[test]
fn rep_alone_stays_on_own_line() {
    // F3 90  →  rep  (nop is not a string op)
    let mut mem = vec![0u8; 0x10000];
    mem[0] = 0xF3; // REP
    mem[1] = 0x90; // nop
    let cpu = FakeCpu::new(&mem);

    let line = decode_line(&cpu, 0, 0);
    assert!(line.contains("rep"), "asm column: {line}");
    assert!(!line.contains("rep nop"), "should not fold nop: {line}");
}

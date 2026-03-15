use std::fmt;

use crate::Computer;

// ─── Register types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg8 {
    AL,
    CL,
    DL,
    BL,
    AH,
    CH,
    DH,
    BH,
}

impl Reg8 {
    fn from_bits(bits: u8) -> Self {
        match bits & 7 {
            0 => Reg8::AL,
            1 => Reg8::CL,
            2 => Reg8::DL,
            3 => Reg8::BL,
            4 => Reg8::AH,
            5 => Reg8::CH,
            6 => Reg8::DH,
            _ => Reg8::BH,
        }
    }

    fn value(self, cpu: &dyn Computer) -> u8 {
        match self {
            Reg8::AL => cpu.ax() as u8,
            Reg8::CL => cpu.cx() as u8,
            Reg8::DL => cpu.dx() as u8,
            Reg8::BL => cpu.bx() as u8,
            Reg8::AH => (cpu.ax() >> 8) as u8,
            Reg8::CH => (cpu.cx() >> 8) as u8,
            Reg8::DH => (cpu.dx() >> 8) as u8,
            Reg8::BH => (cpu.bx() >> 8) as u8,
        }
    }
}

impl fmt::Display for Reg8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Reg8::AL => "al",
            Reg8::CL => "cl",
            Reg8::DL => "dl",
            Reg8::BL => "bl",
            Reg8::AH => "ah",
            Reg8::CH => "ch",
            Reg8::DH => "dh",
            Reg8::BH => "bh",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg16 {
    AX,
    CX,
    DX,
    BX,
    SP,
    BP,
    SI,
    DI,
}

impl Reg16 {
    fn from_bits(bits: u8) -> Self {
        match bits & 7 {
            0 => Reg16::AX,
            1 => Reg16::CX,
            2 => Reg16::DX,
            3 => Reg16::BX,
            4 => Reg16::SP,
            5 => Reg16::BP,
            6 => Reg16::SI,
            _ => Reg16::DI,
        }
    }

    fn value(self, cpu: &dyn Computer) -> u16 {
        match self {
            Reg16::AX => cpu.ax(),
            Reg16::CX => cpu.cx(),
            Reg16::DX => cpu.dx(),
            Reg16::BX => cpu.bx(),
            Reg16::SP => cpu.sp(),
            Reg16::BP => cpu.bp(),
            Reg16::SI => cpu.si(),
            Reg16::DI => cpu.di(),
        }
    }
}

impl fmt::Display for Reg16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Reg16::AX => "ax",
            Reg16::CX => "cx",
            Reg16::DX => "dx",
            Reg16::BX => "bx",
            Reg16::SP => "sp",
            Reg16::BP => "bp",
            Reg16::SI => "si",
            Reg16::DI => "di",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegReg {
    ES,
    CS,
    SS,
    DS,
}

impl fmt::Display for SegReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SegReg::ES => "es",
            SegReg::CS => "cs",
            SegReg::SS => "ss",
            SegReg::DS => "ds",
        })
    }
}

// ─── Memory addressing ────────────────────────────────────────────────────────

/// The base component of a ModRM memory reference (rm field, mod != 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmBase {
    BxSi,
    BxDi,
    BpSi,
    BpDi,
    Si,
    Di,
    Bp,
    Bx,
}

impl RmBase {
    fn from_bits(bits: u8) -> Self {
        match bits & 7 {
            0 => RmBase::BxSi,
            1 => RmBase::BxDi,
            2 => RmBase::BpSi,
            3 => RmBase::BpDi,
            4 => RmBase::Si,
            5 => RmBase::Di,
            6 => RmBase::Bp,
            _ => RmBase::Bx,
        }
    }

    fn compute(self, cpu: &dyn Computer) -> u16 {
        match self {
            RmBase::BxSi => cpu.bx().wrapping_add(cpu.si()),
            RmBase::BxDi => cpu.bx().wrapping_add(cpu.di()),
            RmBase::BpSi => cpu.bp().wrapping_add(cpu.si()),
            RmBase::BpDi => cpu.bp().wrapping_add(cpu.di()),
            RmBase::Si => cpu.si(),
            RmBase::Di => cpu.di(),
            RmBase::Bp => cpu.bp(),
            RmBase::Bx => cpu.bx(),
        }
    }

    /// Default segment register (BP-based addressing uses SS; others use DS).
    fn default_seg(self, cpu: &dyn Computer) -> u16 {
        match self {
            RmBase::BpSi | RmBase::BpDi | RmBase::Bp => cpu.ss(),
            _ => cpu.ds(),
        }
    }
}

/// A resolved memory reference: segment + effective address.
#[derive(Debug, Clone)]
pub struct MemRef {
    pub seg: u16,
    pub ea: u16,
}

impl MemRef {
    pub fn phys(&self) -> u32 {
        ((self.seg as u32) << 4).wrapping_add(self.ea as u32)
    }
}

// ─── Operands ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Operand {
    Reg8(Reg8, u8),    // register + its value at decode time
    Reg16(Reg16, u16), // register + its value at decode time
    Seg(SegReg, u16),  // segment register + its value at decode time
    Imm8(u8),
    Imm16(u16),
    Mem8 { mem: MemRef, value: u8 },
    Mem16 { mem: MemRef, value: u16 },
}

impl Operand {
    /// The text that goes inside the instruction mnemonic line, e.g. `bx`, `[0x7c11]`, `0x04`.
    pub fn asm_str(&self) -> String {
        match self {
            Operand::Reg8(r, _) => r.to_string(),
            Operand::Reg16(r, _) => r.to_string(),
            Operand::Seg(r, _) => r.to_string(),
            Operand::Imm8(v) => format!("0x{:02x}", v),
            Operand::Imm16(v) => format!("0x{:04x}", v),
            Operand::Mem8 { mem, .. } => format!("[0x{:04x}]", mem.ea),
            Operand::Mem16 { mem, .. } => format!("[0x{:04x}]", mem.ea),
        }
    }

    /// Extra data shown on the right side of the log line, or `None` for immediates.
    pub fn annotation(&self) -> Option<String> {
        match self {
            Operand::Reg8(r, v) => Some(format!("{}={:02X}", r.to_string().to_uppercase(), v)),
            Operand::Reg16(r, v) => Some(format!("{}={:04X}", r.to_string().to_uppercase(), v)),
            Operand::Seg(r, v) => Some(format!("{}={:04X}", r.to_string().to_uppercase(), v)),
            Operand::Imm8(_) | Operand::Imm16(_) => None,
            Operand::Mem8 { mem, value } => Some(format!(
                "[0x{:04x}]={:02x} @{:04X}:{:04X}({:05X})",
                mem.ea,
                value,
                mem.seg,
                mem.ea,
                mem.phys()
            )),
            Operand::Mem16 { mem, value } => Some(format!(
                "[0x{:04x}]={:04x} @{:04X}:{:04X}({:05X})",
                mem.ea,
                value,
                mem.seg,
                mem.ea,
                mem.phys()
            )),
        }
    }
}

// ─── Mnemonics ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mnemonic {
    // Data transfer
    Mov,
    Push,
    Pop,
    Xchg,
    In,
    Out,
    Xlat,
    Lea,
    Lds,
    Les,
    Lahf,
    Sahf,
    Pushf,
    Popf,
    // Arithmetic
    Add,
    Adc,
    Sub,
    Sbb,
    Inc,
    Dec,
    Neg,
    Cmp,
    Mul,
    IMul,
    Div,
    IDiv,
    Cbw,
    Cwd,
    Daa,
    Das,
    Aaa,
    Aas,
    Aam,
    Aad,
    // Logic
    And,
    Or,
    Xor,
    Not,
    Test,
    // Shift / rotate
    Shl,
    Shr,
    Sar,
    Rol,
    Ror,
    Rcl,
    Rcr,
    // String ops
    Movsb,
    Movsw,
    Cmpsb,
    Cmpsw,
    Scasb,
    Scasw,
    Lodsb,
    Lodsw,
    Stosb,
    Stosw,
    // Control transfer
    Call,
    CallFar,
    Ret,
    RetFar,
    Jmp,
    JmpFar,
    Ja,
    Jae,
    Jb,
    Jbe,
    Jcxz,
    Je,
    Jg,
    Jge,
    Jl,
    Jle,
    Jne,
    Jno,
    Jns,
    Jo,
    Jp,
    Jnp,
    Js,
    Loop,
    Loopz,
    Loopnz,
    // Interrupt
    Int,
    Int3,
    Into,
    Iret,
    // Processor control
    Nop,
    Hlt,
    Wait,
    Lock,
    Cli,
    Sti,
    Cld,
    Std,
    Clc,
    Stc,
    Cmc,
    // Prefixes
    Rep,
    Repne,
    // Unknown / unimplemented
    Unknown(u8),
}

impl fmt::Display for Mnemonic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Mnemonic::Mov => "mov",
            Mnemonic::Push => "push",
            Mnemonic::Pop => "pop",
            Mnemonic::Xchg => "xchg",
            Mnemonic::In => "in",
            Mnemonic::Out => "out",
            Mnemonic::Xlat => "xlat",
            Mnemonic::Lea => "lea",
            Mnemonic::Lds => "lds",
            Mnemonic::Les => "les",
            Mnemonic::Lahf => "lahf",
            Mnemonic::Sahf => "sahf",
            Mnemonic::Pushf => "pushf",
            Mnemonic::Popf => "popf",
            Mnemonic::Add => "add",
            Mnemonic::Adc => "adc",
            Mnemonic::Sub => "sub",
            Mnemonic::Sbb => "sbb",
            Mnemonic::Inc => "inc",
            Mnemonic::Dec => "dec",
            Mnemonic::Neg => "neg",
            Mnemonic::Cmp => "cmp",
            Mnemonic::Mul => "mul",
            Mnemonic::IMul => "imul",
            Mnemonic::Div => "div",
            Mnemonic::IDiv => "idiv",
            Mnemonic::Cbw => "cbw",
            Mnemonic::Cwd => "cwd",
            Mnemonic::Daa => "daa",
            Mnemonic::Das => "das",
            Mnemonic::Aaa => "aaa",
            Mnemonic::Aas => "aas",
            Mnemonic::Aam => "aam",
            Mnemonic::Aad => "aad",
            Mnemonic::And => "and",
            Mnemonic::Or => "or",
            Mnemonic::Xor => "xor",
            Mnemonic::Not => "not",
            Mnemonic::Test => "test",
            Mnemonic::Shl => "shl",
            Mnemonic::Shr => "shr",
            Mnemonic::Sar => "sar",
            Mnemonic::Rol => "rol",
            Mnemonic::Ror => "ror",
            Mnemonic::Rcl => "rcl",
            Mnemonic::Rcr => "rcr",
            Mnemonic::Movsb => "movsb",
            Mnemonic::Movsw => "movsw",
            Mnemonic::Cmpsb => "cmpsb",
            Mnemonic::Cmpsw => "cmpsw",
            Mnemonic::Scasb => "scasb",
            Mnemonic::Scasw => "scasw",
            Mnemonic::Lodsb => "lodsb",
            Mnemonic::Lodsw => "lodsw",
            Mnemonic::Stosb => "stosb",
            Mnemonic::Stosw => "stosw",
            Mnemonic::Call => "call",
            Mnemonic::CallFar => "call far",
            Mnemonic::Ret => "ret",
            Mnemonic::RetFar => "retf",
            Mnemonic::Jmp => "jmp",
            Mnemonic::JmpFar => "jmp far",
            Mnemonic::Ja => "ja",
            Mnemonic::Jae => "jae",
            Mnemonic::Jb => "jb",
            Mnemonic::Jbe => "jbe",
            Mnemonic::Jcxz => "jcxz",
            Mnemonic::Je => "je",
            Mnemonic::Jg => "jg",
            Mnemonic::Jge => "jge",
            Mnemonic::Jl => "jl",
            Mnemonic::Jle => "jle",
            Mnemonic::Jne => "jne",
            Mnemonic::Jno => "jno",
            Mnemonic::Jns => "jns",
            Mnemonic::Jo => "jo",
            Mnemonic::Jp => "jp",
            Mnemonic::Jnp => "jnp",
            Mnemonic::Js => "js",
            Mnemonic::Loop => "loop",
            Mnemonic::Loopz => "loopz",
            Mnemonic::Loopnz => "loopnz",
            Mnemonic::Int => "int",
            Mnemonic::Int3 => "int3",
            Mnemonic::Into => "into",
            Mnemonic::Iret => "iret",
            Mnemonic::Nop => "nop",
            Mnemonic::Hlt => "hlt",
            Mnemonic::Wait => "wait",
            Mnemonic::Lock => "lock",
            Mnemonic::Cli => "cli",
            Mnemonic::Sti => "sti",
            Mnemonic::Cld => "cld",
            Mnemonic::Std => "std",
            Mnemonic::Clc => "clc",
            Mnemonic::Stc => "stc",
            Mnemonic::Cmc => "cmc",
            Mnemonic::Rep => "rep",
            Mnemonic::Repne => "repne",
            Mnemonic::Unknown(b) => return write!(f, "db 0x{:02x}", b),
        };
        f.write_str(s)
    }
}

// ─── Instruction ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Instruction {
    /// Segment the instruction was fetched from.
    pub segment: u16,
    /// Offset (within segment) of the first byte.
    pub offset: u16,
    /// Raw bytes that make up this instruction (including any prefix).
    pub bytes: Vec<u8>,
    pub mnemonic: Mnemonic,
    /// Operands in source-order: [dst, src] where applicable.
    pub operands: Vec<Operand>,
    /// Segment override prefix, if present.
    pub seg_override: Option<SegReg>,
    /// Optional comment shown at the end of the line, prefixed with ";".
    pub comment: Option<String>,
}

impl Instruction {
    /// Format as a single log line:
    /// `SSSS:OOOO  BB BB BB BB    mnemonic dst, src       ANNOTATIONS    ; comment`
    pub fn format_line(&self) -> String {
        let location = format!("{:04X}:{:04X}", self.segment, self.offset);

        let byte_str: String = self
            .bytes
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");

        let asm = match self.operands.len() {
            0 => self.mnemonic.to_string(),
            1 => format!("{} {}", self.mnemonic, self.operands[0].asm_str()),
            _ => format!(
                "{} {}, {}",
                self.mnemonic,
                self.operands[0].asm_str(),
                self.operands[1].asm_str()
            ),
        };

        let annotations: Vec<String> = self
            .operands
            .iter()
            .filter_map(|op| op.annotation())
            .collect();
        let ann_str = annotations.join(" ");

        let base = if ann_str.is_empty() {
            format!("{} {:<20} {}", location, byte_str, asm)
        } else {
            format!("{} {:<20} {:<30} {}", location, byte_str, asm, ann_str)
        };

        match &self.comment {
            Some(c) => format!("{}    ; {}", base, c),
            None => base,
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_line())
    }
}

// ─── Decode cursor ────────────────────────────────────────────────────────────

struct Cursor<'a> {
    cpu: &'a dyn Computer,
    /// Segment being fetched from.
    seg: u16,
    /// Current fetch offset (advances as bytes are consumed).
    offset: u16,
    /// Accumulated raw bytes.
    bytes: Vec<u8>,
    /// Active segment override prefix (if any).
    seg_override: Option<SegReg>,
}

impl<'a> Cursor<'a> {
    fn new(cpu: &'a dyn Computer, seg: u16, offset: u16) -> Self {
        Self {
            cpu,
            seg,
            offset,
            bytes: Vec::new(),
            seg_override: None,
        }
    }

    /// Fetch the next instruction byte and advance the cursor.
    fn fetch(&mut self) -> u8 {
        let phys = ((self.seg as u32) << 4).wrapping_add(self.offset as u32);
        let b = self.cpu.read_u8(phys);
        self.bytes.push(b);
        self.offset = self.offset.wrapping_add(1);
        b
    }

    /// Fetch a little-endian 16-bit word.
    fn fetch16(&mut self) -> u16 {
        let lo = self.fetch() as u16;
        let hi = self.fetch() as u16;
        lo | (hi << 8)
    }

    fn read_mem_u8(&self, seg: u16, ea: u16) -> u8 {
        self.cpu
            .read_u8(((seg as u32) << 4).wrapping_add(ea as u32))
    }

    fn read_mem_u16(&self, seg: u16, ea: u16) -> u16 {
        let lo = self.read_mem_u8(seg, ea) as u16;
        let hi = self.read_mem_u8(seg, ea.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Resolve which segment to use for a given RmBase (honouring any prefix override).
    fn seg_for_base(&self, base: RmBase) -> u16 {
        self.seg_override
            .map(|s| self.seg_val(s))
            .unwrap_or_else(|| base.default_seg(self.cpu))
    }

    /// Resolve which segment to use for a direct [imm16] address.
    fn seg_for_direct(&self) -> u16 {
        self.seg_override
            .map(|s| self.seg_val(s))
            .unwrap_or_else(|| self.cpu.ds())
    }

    fn seg_val(&self, s: SegReg) -> u16 {
        match s {
            SegReg::ES => self.cpu.es(),
            SegReg::CS => self.cpu.cs(),
            SegReg::SS => self.cpu.ss(),
            SegReg::DS => self.cpu.ds(),
        }
    }
}

// ─── ModRM decoding ───────────────────────────────────────────────────────────

/// Decode the `rm` field of a ModRM byte into an Operand.
/// Any displacement bytes are consumed from `cur`.
fn decode_rm(cur: &mut Cursor, modrm: u8, is16: bool) -> Operand {
    let mod_ = (modrm >> 6) & 3;
    let rm = modrm & 7;

    if mod_ == 3 {
        // Register operand
        return if is16 {
            let r = Reg16::from_bits(rm);
            Operand::Reg16(r, r.value(cur.cpu))
        } else {
            let r = Reg8::from_bits(rm);
            Operand::Reg8(r, r.value(cur.cpu))
        };
    }

    // Memory operand
    let (ea, seg) = if mod_ == 0 && rm == 6 {
        // Special case: [disp16] direct address
        let addr = cur.fetch16();
        (addr, cur.seg_for_direct())
    } else {
        let base = RmBase::from_bits(rm);
        let base_ea = base.compute(cur.cpu);
        let ea = match mod_ {
            0 => base_ea,
            1 => base_ea.wrapping_add(cur.fetch() as i8 as u16),
            2 => base_ea.wrapping_add(cur.fetch16()),
            _ => unreachable!(),
        };
        (ea, cur.seg_for_base(base))
    };

    let mem = MemRef { seg, ea };
    if is16 {
        let value = cur.read_mem_u16(seg, ea);
        Operand::Mem16 { mem, value }
    } else {
        let value = cur.read_mem_u8(seg, ea);
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
    let comment = default_comment(&mnemonic, &operands, cpu);
    Instruction {
        segment: seg,
        offset,
        bytes: cur.bytes,
        mnemonic,
        operands,
        seg_override: cur.seg_override,
        comment,
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
                        mem: MemRef { seg, ea },
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
                        mem: MemRef { seg, ea },
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
                        mem: MemRef { seg, ea },
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
                        mem: MemRef { seg, ea },
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
        0x06 => (Mnemonic::Push, vec![Operand::Imm16(cur.cpu.es())]),
        0x0E => (Mnemonic::Push, vec![Operand::Imm16(cur.cpu.cs())]),
        0x16 => (Mnemonic::Push, vec![Operand::Imm16(cur.cpu.ss())]),
        0x1E => (Mnemonic::Push, vec![Operand::Imm16(cur.cpu.ds())]),
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
            (Mnemonic::Jmp, vec![Operand::Imm16(tgt)])
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

/// Extract a port number from an IN/OUT port operand and look up its name.
/// Handles both immediate ports (`imm8`) and DX-indirect ports (`Reg16(DX, value)`).
fn port_comment(port_op: Option<&Operand>) -> Option<String> {
    let port = match port_op? {
        Operand::Imm8(p) => *p as u16,
        Operand::Reg16(Reg16::DX, v) => *v,
        _ => return None,
    };
    io_port_name(port).map(|s| s.to_string())
}

fn io_port_name(port: u16) -> Option<&'static str> {
    match port {
        // PIC
        0x0020 => Some("PIC1 Command"),
        0x0021 => Some("PIC1 Mask"),
        0x00A0 => Some("PIC2 Command"),
        0x00A1 => Some("PIC2 Mask"),
        // PIT
        0x0040 => Some("PIT Channel 0"),
        0x0041 => Some("PIT Channel 1"),
        0x0042 => Some("PIT Channel 2"),
        0x0043 => Some("PIT Control"),
        // System Control Port B
        0x0061 => Some("System Control Port B"),
        // Keyboard Controller
        0x0060 => Some("KBC Data"),
        0x0064 => Some("KBC Status/Command"),
        // RTC
        0x0070 => Some("RTC Register Select"),
        0x0071 => Some("RTC Data"),
        // CGA/VGA CRTC
        0x03B4 => Some("MDA CRTC Address"),
        0x03B5 => Some("MDA CRTC Data"),
        0x03C0 => Some("VGA AC Address/Data"),
        0x03C1 => Some("VGA AC Data Read"),
        0x03C7 => Some("VGA DAC Read Index"),
        0x03C8 => Some("VGA DAC Write Index"),
        0x03C9 => Some("VGA DAC Data"),
        0x03D4 => Some("CGA CRTC Address"),
        0x03D5 => Some("CGA CRTC Data"),
        0x03D9 => Some("CGA Color Select"),
        0x03DA => Some("CGA Status"),
        // FDC
        0x03F2 => Some("FDC DOR"),
        0x03F4 => Some("FDC Status"),
        0x03F5 => Some("FDC Data"),
        0x03F7 => Some("FDC DIR"),
        // HDC
        0x01F0 => Some("HDC Data"),
        0x01F1 => Some("HDC Error/Features"),
        0x01F2 => Some("HDC Sector Count"),
        0x01F3 => Some("HDC Sector Number"),
        0x01F4 => Some("HDC Cylinder Low"),
        0x01F5 => Some("HDC Cylinder High"),
        0x01F6 => Some("HDC Drive/Head"),
        0x01F7 => Some("HDC Command/Status"),
        0x03F6 => Some("HDC Device Control"),
        // UART (COM ports)
        0x03F8 => Some("COM1 Data"),
        0x03F9 => Some("COM1 IER/DLM"),
        0x03FA => Some("COM1 IIR/FCR"),
        0x03FB => Some("COM1 LCR"),
        0x03FC => Some("COM1 MCR"),
        0x03FD => Some("COM1 LSR"),
        0x03FE => Some("COM1 MSR"),
        0x02F8 => Some("COM2 Data"),
        0x02F9 => Some("COM2 IER/DLM"),
        0x02FA => Some("COM2 IIR/FCR"),
        0x02FB => Some("COM2 LCR"),
        0x02FC => Some("COM2 MCR"),
        0x02FD => Some("COM2 LSR"),
        0x02FE => Some("COM2 MSR"),
        0x03E8 => Some("COM3 Data"),
        0x02E8 => Some("COM4 Data"),
        _ => None,
    }
}

fn int_description(int_num: u8, ah: u8) -> Option<(&'static str, bool)> {
    match int_num {
        0x08 => Some(("timer interrupt", false)),
        0x09 => Some(("keyboard interrupt", false)),
        0x10 => {
            let desc = match ah {
                0x00 => "set video mode",
                0x01 => "set cursor shape",
                0x02 => "set cursor position",
                0x03 => "get cursor position",
                0x05 => "select active page",
                0x06 => "scroll up",
                0x07 => "scroll down",
                0x08 => "read char/attr",
                0x09 => "write char/attr",
                0x0A => "write char",
                0x0B => "set color palette",
                0x0E => "teletype output",
                0x0F => "get video mode",
                0x10 => "palette registers",
                0x11 => "character generator",
                0x12 => "alternate function select",
                0x15 => "return physical display params",
                0x1A => "display combination code",
                _ => return None,
            };
            Some((desc, true))
        }
        0x11 => Some(("get equipment list", false)),
        0x12 => Some(("get memory size", false)),
        0x13 => {
            let desc = match ah {
                0x00 => "reset disk",
                0x01 => "get disk status",
                0x02 => "read sectors",
                0x03 => "write sectors",
                0x04 => "verify sectors",
                0x08 => "get drive params",
                0x15 => "get disk type",
                0x16 => "detect disk change",
                0x18 => "set DASD type",
                _ => return None,
            };
            Some((desc, true))
        }
        0x14 => {
            let desc = match ah {
                0x00 => "initialize serial port",
                0x01 => "write char",
                0x02 => "read char",
                0x03 => "get status",
                _ => return None,
            };
            Some((desc, true))
        }
        0x15 => {
            let desc = match ah {
                0x10 => "TopView multi-DOS",
                0x41 => "wait external event",
                0x4F => "keyboard intercept",
                0x88 => "get extended memory",
                0x91 => "device interrupt complete",
                0xC0 => "get system config",
                0xC1 => "get EBDA segment",
                0xC2 => "PS/2 mouse services",
                _ => return None,
            };
            Some((desc, true))
        }
        0x16 => {
            let desc = match ah {
                0x00 => "read char",
                0x01 => "check keystroke",
                0x02 => "get shift flags",
                0x55 => "word TSR check",
                0x92 => "get keyboard capabilities",
                0xA2 => "122-key capability check",
                _ => return None,
            };
            Some((desc, true))
        }
        0x17 => {
            let desc = match ah {
                0x01 => "initialize printer",
                _ => return None,
            };
            Some((desc, true))
        }
        0x1A => {
            let desc = match ah {
                0x00 => "get system time",
                0x01 => "set system time",
                0x02 => "read RTC time",
                0x03 => "set RTC time",
                0x04 => "read RTC date",
                0x05 => "set RTC date",
                _ => return None,
            };
            Some((desc, true))
        }
        0x21 => {
            let desc = match ah {
                0x02 => "write char",
                0x09 => "write string",
                0x4C => "exit",
                _ => return None,
            };
            Some((desc, true))
        }
        0x74 => Some(("PS/2 mouse interrupt", false)),
        _ => None,
    }
}

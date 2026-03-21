use super::{MemRef, Reg8, Reg16, SegReg};

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
            Operand::Mem8 { mem, .. } => format!("[{}]", mem.expr),
            Operand::Mem16 { mem, .. } => format!("[{}]", mem.expr),
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

use std::fmt;

use crate::Computer;

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
    pub(super) fn from_bits(bits: u8) -> Self {
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

    pub(super) fn value(self, cpu: &dyn Computer) -> u8 {
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
    pub(super) fn from_bits(bits: u8) -> Self {
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

    pub(super) fn value(self, cpu: &dyn Computer) -> u16 {
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

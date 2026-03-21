use crate::Computer;

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
    pub(super) fn from_bits(bits: u8) -> Self {
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

    pub(super) fn compute(self, cpu: &dyn Computer) -> u16 {
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

    pub(super) fn asm_str(self) -> &'static str {
        match self {
            RmBase::BxSi => "bx+si",
            RmBase::BxDi => "bx+di",
            RmBase::BpSi => "bp+si",
            RmBase::BpDi => "bp+di",
            RmBase::Si => "si",
            RmBase::Di => "di",
            RmBase::Bp => "bp",
            RmBase::Bx => "bx",
        }
    }

    /// Default segment register (BP-based addressing uses SS; others use DS).
    pub(super) fn default_seg(self, cpu: &dyn Computer) -> u16 {
        match self {
            RmBase::BpSi | RmBase::BpDi | RmBase::Bp => cpu.ss(),
            _ => cpu.ds(),
        }
    }
}

/// A resolved memory reference: segment + effective address + symbolic expression.
#[derive(Debug, Clone)]
pub struct MemRef {
    pub seg: u16,
    pub ea: u16,
    /// Symbolic addressing expression, e.g. `"bx"`, `"bx+si+0x04"`, `"0x1234"`.
    pub expr: String,
}

impl MemRef {
    pub fn phys(&self) -> u32 {
        ((self.seg as u32) << 4).wrapping_add(self.ea as u32)
    }
}

/// CPU type enumeration for emulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CpuType {
    /// Intel 8086 (1978) - 16-bit CPU, 1 MB addressable memory
    #[default]
    I8086,
    /// Intel 80286 (1982) - 16-bit CPU with protected mode, up to 16 MB memory
    I80286,
    /// Intel 80386 (1985) - 32-bit CPU with 32-bit registers and addressing
    I80386,
    /// Intel 80486 (1989) - Enhanced 386 with integrated FPU
    I80486,
}

impl CpuType {
    /// Parse CPU type from string (e.g., "8086", "286", "386", "486")
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "8086" | "86" => Some(Self::I8086),
            "286" | "80286" => Some(Self::I80286),
            "386" | "80386" => Some(Self::I80386),
            "486" | "80486" => Some(Self::I80486),
            _ => None,
        }
    }

    /// Get the display name for this CPU type
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::I8086 => "8086",
            Self::I80286 => "80286",
            Self::I80386 => "80386",
            Self::I80486 => "80486",
        }
    }

    /// Get the max extended memory size in KB for this CPU type
    /// Extended memory is memory above 1 MB (0x100000)
    /// Only available on 286+ CPUs
    pub(crate) fn max_extended_memory_kb(&self) -> u16 {
        match self {
            Self::I8086 => 0,      // 8086 has no extended memory
            Self::I80286 => 15360, // 286: 16 MB total - 1 MB = 15 MB = 15360 KB
            Self::I80386 => 65535, // 386: Return max value (64 MB)
            Self::I80486 => 65535, // 486: Return max value (64 MB)
        }
    }

    /// Returns true if this CPU supports 286+ instructions (PUSHA/POPA, BOUND,
    /// PUSH imm, IMUL 3-op, INS/OUTS, ENTER/LEAVE, shift/rotate by immediate).
    pub(crate) fn is_286_or_later(&self) -> bool {
        !matches!(self, Self::I8086)
    }

    // TODO
    // Check if this CPU supports 32-bit instructions
    // pub(crate) fn supports_32bit(&self) -> bool {
    //     matches!(self, Self::I80386 | Self::I80486)
    // }

    /// Check if this CPU supports protected mode (286+)
    pub(crate) fn supports_protected_mode(&self) -> bool {
        !matches!(self, Self::I8086)
    }
}

impl std::fmt::Display for CpuType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

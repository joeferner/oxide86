// MIGRATED  /// CPU type enumeration for emulation
// MIGRATED  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
// MIGRATED  pub enum CpuType {
// MIGRATED      /// Intel 8086 (1978) - 16-bit CPU, 1 MB addressable memory
// MIGRATED      #[default]
// MIGRATED      I8086,
// MIGRATED      /// Intel 80286 (1982) - 16-bit CPU with protected mode, up to 16 MB memory
// MIGRATED      I80286,
// MIGRATED      /// Intel 80386 (1985) - 32-bit CPU with 32-bit registers and addressing
// MIGRATED      I80386,
// MIGRATED      /// Intel 80486 (1989) - Enhanced 386 with integrated FPU
// MIGRATED      I80486,
// MIGRATED  }
// MIGRATED  
// MIGRATED  impl CpuType {
// MIGRATED      /// Parse CPU type from string (e.g., "8086", "286", "386", "486")
// MIGRATED      pub fn parse(s: &str) -> Option<Self> {
// MIGRATED          match s.to_lowercase().as_str() {
// MIGRATED              "8086" | "86" => Some(Self::I8086),
// MIGRATED              "286" | "80286" => Some(Self::I80286),
// MIGRATED              "386" | "80386" => Some(Self::I80386),
// MIGRATED              "486" | "80486" => Some(Self::I80486),
// MIGRATED              _ => None,
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Get the display name for this CPU type
// MIGRATED      pub fn name(&self) -> &'static str {
// MIGRATED          match self {
// MIGRATED              Self::I8086 => "8086",
// MIGRATED              Self::I80286 => "80286",
// MIGRATED              Self::I80386 => "80386",
// MIGRATED              Self::I80486 => "80486",
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Get the max extended memory size in KB for this CPU type
// MIGRATED      /// Extended memory is memory above 1 MB (0x100000)
// MIGRATED      /// Only available on 286+ CPUs
// MIGRATED      pub fn max_extended_memory_kb(&self) -> u16 {
// MIGRATED          match self {
// MIGRATED              Self::I8086 => 0,      // 8086 has no extended memory
// MIGRATED              Self::I80286 => 15360, // 286: 16 MB total - 1 MB = 15 MB = 15360 KB
// MIGRATED              Self::I80386 => 65535, // 386: Return max value (64 MB)
// MIGRATED              Self::I80486 => 65535, // 486: Return max value (64 MB)
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Check if this CPU supports 32-bit instructions
// MIGRATED      pub fn supports_32bit(&self) -> bool {
// MIGRATED          matches!(self, Self::I80386 | Self::I80486)
// MIGRATED      }
// MIGRATED  
// MIGRATED      /// Check if this CPU supports protected mode
// MIGRATED      pub fn supports_protected_mode(&self) -> bool {
// MIGRATED          !matches!(self, Self::I8086)
// MIGRATED      }
// MIGRATED  }
// MIGRATED  
// MIGRATED  impl std::fmt::Display for CpuType {
// MIGRATED      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// MIGRATED          write!(f, "{}", self.name())
// MIGRATED      }
// MIGRATED  }
// MIGRATED  
use anyhow::{Context, Result};

// MIGRATED  pub fn parse_hex_or_dec(s: &str) -> Result<u16> {
// MIGRATED      if let Some(hex) = s.strip_prefix("0x") {
// MIGRATED          u16::from_str_radix(hex, 16).with_context(|| format!("Invalid hex value: {}", s))
// MIGRATED      } else {
// MIGRATED          s.parse::<u16>()
// MIGRATED              .with_context(|| format!("Invalid decimal value: {}", s))
// MIGRATED      }
// MIGRATED  }

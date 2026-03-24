use std::fmt;

use super::{Mnemonic, Operand, SegReg};

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
    /// Implicit operands shown only in the annotation column (not in the asm mnemonic).
    pub implicit_operands: Vec<Operand>,
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
            .chain(self.implicit_operands.iter())
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

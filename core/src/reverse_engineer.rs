use std::collections::{BTreeMap, BTreeSet};

use crate::dis::FlowKind;
use crate::physical_address;

pub struct ReverseEntry {
    pub cs: u16,
    pub ip: u16,
    pub text: String,
    pub bytes: Vec<u8>,
}

pub struct ReverseEngineer {
    instructions: BTreeMap<usize, ReverseEntry>,
    call_targets: BTreeSet<usize>,
    jump_targets: BTreeSet<usize>,
    data_reads: BTreeMap<usize, (u8, u16)>, // phys_addr -> (val, 16-bit offset)
    entry_address: Option<usize>,
}

/// Scan instruction text for `[0x????]` patterns and replace with data labels.
fn substitute_data_labels(text: &str, offset_to_label: &BTreeMap<u16, String>) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < bytes.len() {
        // Look for "[0x" followed by exactly 4 hex digits and "]"
        if bytes[i] == b'['
            && i + 8 < bytes.len()
            && bytes[i + 1] == b'0'
            && bytes[i + 2] == b'x'
            && bytes[i + 7] == b']'
        {
            let hex = &text[i + 3..i + 7];
            if let Ok(offset) = u16::from_str_radix(hex, 16) {
                if let Some(label) = offset_to_label.get(&offset) {
                    out.push('[');
                    out.push_str(label);
                    out.push(']');
                    i += 8;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

impl ReverseEngineer {
    pub fn new() -> Self {
        Self {
            instructions: BTreeMap::new(),
            call_targets: BTreeSet::new(),
            jump_targets: BTreeSet::new(),
            data_reads: BTreeMap::new(),
            entry_address: None,
        }
    }

    pub fn record_instruction(
        &mut self,
        cs: u16,
        ip: u16,
        flow: &FlowKind,
        text: String,
        bytes: Vec<u8>,
    ) {
        let addr = physical_address(cs, ip);
        if self.entry_address.is_none() {
            self.entry_address = Some(addr);
        }
        // First-seen wins for instruction text/bytes
        self.instructions.entry(addr).or_insert(ReverseEntry {
            cs,
            ip,
            text,
            bytes,
        });

        // Record control-flow targets using current CS for near transfers
        match flow {
            FlowKind::Call(target) => {
                self.call_targets.insert(physical_address(cs, *target));
            }
            FlowKind::CallFar(seg, off) => {
                self.call_targets.insert(physical_address(*seg, *off));
            }
            FlowKind::Jump(target) | FlowKind::ConditionalJump(target) => {
                self.jump_targets.insert(physical_address(cs, *target));
            }
            FlowKind::JumpFar(seg, off) => {
                self.jump_targets.insert(physical_address(*seg, *off));
            }
            _ => {}
        }
    }

    pub fn record_data_read(&mut self, addr: usize, val: u8, ds: u16) {
        let offset = addr.wrapping_sub(ds as usize * 16) as u16;
        self.data_reads.entry(addr).or_insert((val, offset));
    }

    pub fn to_asm_string(&self) -> String {
        let mut out = String::new();
        out.push_str("; Reverse Engineered by Oxide86\n");

        // All addresses that need labels
        let labeled: BTreeSet<usize> = self
            .call_targets
            .iter()
            .chain(self.jump_targets.iter())
            .chain(self.entry_address.iter())
            .copied()
            .collect();

        // Emit data section
        let data_addrs: Vec<usize> = self
            .data_reads
            .keys()
            .filter(|&&a| !self.instructions.contains_key(&a))
            .copied()
            .collect();

        // Build a map from 16-bit offset -> label name for instruction text substitution
        let offset_to_label: BTreeMap<u16, String> = data_addrs
            .iter()
            .map(|&a| {
                let (_, offset) = self.data_reads[&a];
                (offset, format!("data_{a:05X}"))
            })
            .collect();

        // Re-emit instructions with data label substitutions applied
        out.clear();
        out.push_str("; Reverse Engineered by Oxide86\n");
        let mut prev_addr: Option<usize> = None;
        for (&addr, entry) in &self.instructions {
            if let Some(prev_end) = prev_addr
                && addr > prev_end
            {
                out.push('\n');
            }

            if labeled.contains(&addr) {
                out.push('\n');
                out.push_str(&format!("{}:\n", self.label_name(addr)));
            }

            let bytes_hex = entry
                .bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ");

            let text = substitute_data_labels(&entry.text, &offset_to_label);

            out.push_str(&format!(
                "    {:04X}:{:04X}  {:<20}  {}\n",
                entry.cs, entry.ip, bytes_hex, text
            ));

            prev_addr = Some(addr + entry.bytes.len());
        }

        if !data_addrs.is_empty() {
            out.push_str("\n; Data\n");
            let mut i = 0;
            while i < data_addrs.len() {
                let group_start = data_addrs[i];
                // Find consecutive run
                let mut j = i + 1;
                while j < data_addrs.len() && data_addrs[j] == data_addrs[j - 1] + 1 {
                    j += 1;
                }
                out.push_str(&format!("\ndata_{:05X}:\n", group_start));
                for &a in &data_addrs[i..j] {
                    let (val, _) = self.data_reads[&a];
                    let ascii = if (0x20..0x7F).contains(&val) {
                        format!(" '{}'", val as char)
                    } else {
                        String::new()
                    };
                    out.push_str(&format!("    db 0x{val:02x}    ; {val:3}{ascii}\n"));
                }
                i = j;
            }
        }

        out
    }

    fn label_name(&self, addr: usize) -> String {
        if Some(addr) == self.entry_address
            && !self.call_targets.contains(&addr)
            && !self.jump_targets.contains(&addr)
        {
            return "entry".to_string();
        }
        if self.call_targets.contains(&addr) {
            format!("sub_{addr:05X}")
        } else if Some(addr) == self.entry_address {
            "entry".to_string()
        } else {
            format!("loc_{addr:05X}")
        }
    }
}

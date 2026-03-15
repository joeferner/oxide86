use std::collections::{BTreeMap, BTreeSet};

use crate::cpu::instructions::decoder::{Instruction, Mnemonic, Operand};
use crate::dis::FlowKind;
use crate::physical_address;

pub struct ReverseEntry {
    pub cs: u16,
    pub ip: u16,
    pub bytes: Vec<u8>,
    pub mnemonic: Mnemonic,
    pub operands: Vec<Operand>,
    pub data_refs: Vec<usize>, // implicit data accesses not covered by explicit operands
}

pub struct ReverseEngineer {
    instructions: BTreeMap<usize, ReverseEntry>,
    call_targets: BTreeSet<usize>,
    jump_targets: BTreeSet<usize>,
    data_reads: BTreeMap<usize, u8>, // phys_addr -> byte value
    entry_address: Option<usize>,
}

/// Render an operand, substituting Mem8/Mem16 physical addresses with data labels where known.
fn render_operand(op: &Operand, phys_to_label: &BTreeMap<usize, String>) -> String {
    match op {
        Operand::Mem8 { mem, .. } | Operand::Mem16 { mem, .. } => {
            let phys = mem.phys() as usize;
            if let Some(label) = phys_to_label.get(&phys) {
                format!("[{label}]")
            } else {
                op.asm_str()
            }
        }
        _ => op.asm_str(),
    }
}

/// Render instruction text with data label substitution applied to memory operands.
fn render_instruction(entry: &ReverseEntry, phys_to_label: &BTreeMap<usize, String>) -> String {
    let ops: Vec<String> = entry
        .operands
        .iter()
        .map(|op| render_operand(op, phys_to_label))
        .collect();
    match ops.len() {
        0 => entry.mnemonic.to_string(),
        1 => format!("{} {}", entry.mnemonic, ops[0]),
        _ => format!("{} {}, {}", entry.mnemonic, ops[0], ops[1]),
    }
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

    pub fn record_instruction(&mut self, instr: &Instruction, flow: &FlowKind, data_refs: Vec<usize>) {
        let cs = instr.segment;
        let ip = instr.offset;
        let addr = physical_address(cs, ip);

        if self.entry_address.is_none() {
            self.entry_address = Some(addr);
        }

        // First-seen wins for instruction text/bytes (deduplicates loops)
        self.instructions.entry(addr).or_insert_with(|| ReverseEntry {
            cs,
            ip,
            bytes: instr.bytes.clone(),
            mnemonic: instr.mnemonic.clone(),
            operands: instr.operands.clone(),
            data_refs,
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

    pub fn record_data_read(&mut self, addr: usize, val: u8) {
        self.data_reads.entry(addr).or_insert(val);
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

        // Data addresses that aren't instruction addresses
        let data_addrs: Vec<usize> = self
            .data_reads
            .keys()
            .filter(|&&a| !self.instructions.contains_key(&a))
            .copied()
            .collect();

        // Physical address -> label name, for operand substitution
        let phys_to_label: BTreeMap<usize, String> = data_addrs
            .iter()
            .map(|&a| (a, format!("data_{a:05X}")))
            .collect();

        // Emit instructions with structured operand label substitution
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

            let text = render_instruction(entry, &phys_to_label);

            let comment = if entry.data_refs.is_empty() {
                String::new()
            } else {
                let refs = entry.data_refs.iter()
                    .map(|&a| phys_to_label.get(&a).map(|s| s.as_str()).unwrap_or("?"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("  ; [{refs}]")
            };

            out.push_str(&format!(
                "    {:04X}:{:04X}  {:<20}  {}{}\n",
                entry.cs, entry.ip, bytes_hex, text, comment
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
                    let val = self.data_reads[&a];
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

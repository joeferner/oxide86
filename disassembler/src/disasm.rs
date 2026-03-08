use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use oxide86_core::{
    dis::{disasm_one, DisasmResult, FlowKind},
    ByteReader,
};

use crate::loader::LoadedImage;

/// ByteReader backed by the loaded image data.
struct ImageReader<'a> {
    data: &'a [u8],
    /// Physical base address: image[0] corresponds to this linear address.
    base: usize,
}

impl ByteReader for ImageReader<'_> {
    fn read_u8(&self, addr: usize) -> u8 {
        self.data
            .get(addr.wrapping_sub(self.base))
            .copied()
            .unwrap_or(0xFF)
    }
}

/// Type of a data region declared in the config.
#[derive(Debug, Clone)]
pub enum DataType {
    /// Null-terminated ASCII string.
    String,
    /// Raw byte array of a fixed length.
    Bytes(usize),
}

/// A data region declared in the config (not disassembled as code).
#[derive(Debug, Clone)]
pub struct DataRegion {
    pub data_type: DataType,
    pub label: Option<String>,
}

/// A decoded instruction at a specific address.
pub struct DisasmEntry {
    pub cs: u16,
    pub ip: u16,
    pub result: DisasmResult,
}

/// The full disassembly output.
pub struct Disassembly {
    /// Instructions keyed by linear address.
    pub instructions: BTreeMap<usize, DisasmEntry>,
    /// Linear addresses that need a label (branch/call targets).
    pub labels: BTreeSet<usize>,
    /// Which labels were reached via CALL (vs JMP) — used for naming.
    pub call_targets: BTreeSet<usize>,
    /// Entry point addresses (labeled "entry", "entry1", etc.).
    pub entry_points: Vec<usize>,
    /// Custom label names supplied by the user config, keyed by linear address.
    pub custom_labels: HashMap<usize, String>,
    /// User-supplied comments keyed by linear address.
    pub comments: HashMap<usize, String>,
    /// User-declared data regions keyed by linear address.
    pub data_regions: HashMap<usize, DataRegion>,
    /// First valid linear address (load_segment << 4).
    pub image_base: usize,
    /// One past the last valid linear address.
    pub image_end: usize,
}

pub fn disassemble(
    image: &LoadedImage,
    entries: &[(u16, u16, Option<String>)],
    comments: HashMap<usize, String>,
    data_regions: HashMap<usize, DataRegion>,
) -> Disassembly {
    let base = image.base_linear;
    let reader = ImageReader {
        data: &image.data,
        base,
    };
    let image_end = base + image.data.len();

    let mut instructions: BTreeMap<usize, DisasmEntry> = BTreeMap::new();
    let mut labels: BTreeSet<usize> = BTreeSet::new();
    let mut call_targets: BTreeSet<usize> = BTreeSet::new();
    let mut custom_labels: HashMap<usize, String> = HashMap::new();
    let mut visited: HashSet<usize> = HashSet::new();
    let mut worklist: VecDeque<(u16, u16)> = VecDeque::new();

    let entry_points: Vec<usize> = entries
        .iter()
        .map(|&(cs, ip, _)| linear_addr(cs, ip))
        .collect();

    for (cs, ip, name) in entries {
        let linear = linear_addr(*cs, *ip);
        labels.insert(linear);
        worklist.push_back((*cs, *ip));
        if let Some(n) = name {
            custom_labels.insert(linear, n.clone());
        }
    }

    while let Some((cs, mut ip)) = worklist.pop_front() {
        loop {
            let linear = linear_addr(cs, ip);
            if linear < base || linear >= image_end || visited.contains(&linear) {
                break;
            }
            visited.insert(linear);

            let result = disasm_one(&reader, cs, ip);
            let next_ip = result.next_ip;
            let flow = result.flow.clone();

            instructions.insert(linear, DisasmEntry { cs, ip, result });

            match flow {
                FlowKind::Continue => {
                    ip = next_ip;
                }
                FlowKind::Jump(target) => {
                    let tlinear = linear_addr(cs, target);
                    labels.insert(tlinear);
                    if !visited.contains(&tlinear) {
                        worklist.push_back((cs, target));
                    }
                    break;
                }
                FlowKind::JumpFar(seg, off) => {
                    let tlinear = linear_addr(seg, off);
                    labels.insert(tlinear);
                    if !visited.contains(&tlinear) {
                        worklist.push_back((seg, off));
                    }
                    break;
                }
                FlowKind::ConditionalJump(target) => {
                    let tlinear = linear_addr(cs, target);
                    labels.insert(tlinear);
                    if !visited.contains(&tlinear) {
                        worklist.push_back((cs, target));
                    }
                    // Fall through too
                    ip = next_ip;
                }
                FlowKind::Call(target) => {
                    let tlinear = linear_addr(cs, target);
                    labels.insert(tlinear);
                    call_targets.insert(tlinear);
                    if !visited.contains(&tlinear) {
                        worklist.push_back((cs, target));
                    }
                    // Continue after the call
                    ip = next_ip;
                }
                FlowKind::CallFar(seg, off) => {
                    let tlinear = linear_addr(seg, off);
                    labels.insert(tlinear);
                    call_targets.insert(tlinear);
                    if !visited.contains(&tlinear) {
                        worklist.push_back((seg, off));
                    }
                    ip = next_ip;
                }
                FlowKind::Return | FlowKind::Halt => {
                    break;
                }
                FlowKind::IndirectTransfer => {
                    break;
                }
            }
        }
    }

    Disassembly {
        instructions,
        labels,
        call_targets,
        entry_points,
        custom_labels,
        comments,
        data_regions,
        image_base: base,
        image_end,
    }
}

fn linear_addr(cs: u16, ip: u16) -> usize {
    ((cs as usize) << 4) + ip as usize
}

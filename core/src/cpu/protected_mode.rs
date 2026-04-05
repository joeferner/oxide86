use crate::bus::Bus;

/// Cached descriptor state for a segment register (the "hidden" part).
/// In real mode these are derived from the segment value (base = seg << 4).
/// In protected mode they are loaded from the GDT/LDT.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SegmentCache {
    /// 24-bit base address (on 286)
    pub base: u32,
    /// 16-bit limit
    pub limit: u16,
    /// Access rights byte from the descriptor
    pub access: u8,
}

impl Default for SegmentCache {
    fn default() -> Self {
        Self {
            base: 0,
            limit: 0xFFFF,
            access: 0,
        }
    }
}

impl SegmentCache {
    /// Create a real-mode segment cache from a segment register value.
    pub fn from_real_mode(segment: u16) -> Self {
        Self {
            base: (segment as u32) << 4,
            limit: 0xFFFF,
            access: 0x93, // present, DPL 0, data read/write, accessed
        }
    }
}

/// 286 segment descriptor (8 bytes in the GDT/LDT).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SegmentDescriptor {
    /// 24-bit base address (on 286)
    pub base: u32,
    /// 16-bit segment limit
    pub limit: u16,
    /// Access byte: P(1) | DPL(2) | S(1) | TYPE(4)
    pub access: u8,
}

impl SegmentDescriptor {
    /// Parse a 286 segment descriptor from 8 bytes read from a descriptor table.
    pub fn from_bytes(bytes: &[u8; 8]) -> Self {
        let limit = u16::from_le_bytes([bytes[0], bytes[1]]);
        let base = bytes[2] as u32 | ((bytes[3] as u32) << 8) | ((bytes[4] as u32) << 16);
        let access = bytes[5];
        Self {
            base,
            limit,
            access,
        }
    }

    /// Is the Present bit set?
    pub fn is_present(&self) -> bool {
        self.access & 0x80 != 0
    }

    /// Descriptor Privilege Level (0–3)
    #[allow(dead_code)]
    pub fn dpl(&self) -> u8 {
        (self.access >> 5) & 0x03
    }

    /// Is this a system/gate descriptor (S=0) or code/data (S=1)?
    pub fn is_code_or_data(&self) -> bool {
        self.access & 0x10 != 0
    }

    /// For code/data descriptors: is this a code segment? (TYPE bit 3)
    pub fn is_code(&self) -> bool {
        self.is_code_or_data() && (self.access & 0x08 != 0)
    }

    /// For data descriptors: is this writable? (TYPE bit 1)
    #[allow(dead_code)]
    pub fn is_writable(&self) -> bool {
        !self.is_code() && (self.access & 0x02 != 0)
    }

    /// For code descriptors: is this readable? (TYPE bit 1)
    #[allow(dead_code)]
    pub fn is_readable(&self) -> bool {
        self.is_code() && (self.access & 0x02 != 0)
    }

    /// Convert to a segment cache entry
    pub fn to_cache(&self) -> SegmentCache {
        SegmentCache {
            base: self.base,
            limit: self.limit,
            access: self.access,
        }
    }
}

/// 286 gate descriptor (8 bytes in the IDT or GDT/LDT).
/// Used for interrupt gates, trap gates, call gates, and task gates.
///
/// Format:
///   bytes 0-1: offset (low 16 bits of handler address)
///   bytes 2-3: selector (code segment selector for the handler)
///   byte  4:   word count (for call gates) or reserved (0 for int/trap gates)
///   byte  5:   access byte: P(1) | DPL(2) | 0(1) | TYPE(4)
///   bytes 6-7: reserved (0 on 286)
#[derive(Debug, Clone, Copy)]
pub(crate) struct GateDescriptor {
    /// Handler offset within the target code segment
    pub offset: u16,
    /// Selector for the target code segment
    pub selector: u16,
    /// Access byte: P(1) | DPL(2) | 0(1) | TYPE(4)
    pub access: u8,
}

/// 286 gate types (low 4 bits of access byte)
#[allow(dead_code)]
pub(crate) mod gate_type {
    pub const TASK_GATE_286: u8 = 0x01;
    pub const INTERRUPT_GATE_286: u8 = 0x06;
    pub const TRAP_GATE_286: u8 = 0x07;
}

impl GateDescriptor {
    /// Parse a gate descriptor from 8 bytes.
    pub fn from_bytes(bytes: &[u8; 8]) -> Self {
        let offset = u16::from_le_bytes([bytes[0], bytes[1]]);
        let selector = u16::from_le_bytes([bytes[2], bytes[3]]);
        let access = bytes[5];
        Self {
            offset,
            selector,
            access,
        }
    }

    /// Is the Present bit set?
    pub fn is_present(&self) -> bool {
        self.access & 0x80 != 0
    }

    /// Gate type (low 4 bits of access)
    pub fn gate_type(&self) -> u8 {
        self.access & 0x0F
    }

    /// Is this an interrupt gate? (clears IF on entry)
    pub fn is_interrupt_gate(&self) -> bool {
        self.gate_type() == gate_type::INTERRUPT_GATE_286
    }

    /// Is this a trap gate? (preserves IF on entry)
    pub fn is_trap_gate(&self) -> bool {
        self.gate_type() == gate_type::TRAP_GATE_286
    }
}

/// Load a gate descriptor from the IDT.
/// Returns None if the interrupt number is out of bounds.
pub(crate) fn load_idt_gate(
    bus: &Bus,
    idtr_base: u32,
    idtr_limit: u16,
    int_num: u8,
) -> Option<GateDescriptor> {
    let offset = (int_num as u32) * 8;
    if offset + 7 > idtr_limit as u32 {
        return None;
    }
    let addr = (idtr_base + offset) as usize;
    let mut bytes = [0u8; 8];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = bus.memory_read_u8(addr + i);
    }
    Some(GateDescriptor::from_bytes(&bytes))
}

/// Load a descriptor from a descriptor table in memory.
/// Returns None if the selector is out of bounds.
pub(crate) fn load_descriptor_from_table(
    bus: &Bus,
    table_base: u32,
    table_limit: u16,
    selector: u16,
) -> Option<SegmentDescriptor> {
    // Selector format: index(13) | TI(1) | RPL(2)
    // The index * 8 gives the byte offset into the table
    let index = (selector & 0xFFF8) as u32;
    if index + 7 > table_limit as u32 {
        return None;
    }
    let addr = (table_base + index) as usize;
    let mut bytes = [0u8; 8];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = bus.memory_read_u8(addr + i);
    }
    Some(SegmentDescriptor::from_bytes(&bytes))
}

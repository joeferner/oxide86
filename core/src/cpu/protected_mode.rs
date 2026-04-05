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
        let base = bytes[2] as u32
            | ((bytes[3] as u32) << 8)
            | ((bytes[4] as u32) << 16);
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
    for i in 0..8 {
        bytes[i] = bus.memory_read_u8(addr + i);
    }
    Some(SegmentDescriptor::from_bytes(&bytes))
}

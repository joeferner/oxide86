use crate::Computer;

use super::{RmBase, SegReg};

pub struct Cursor<'a> {
    pub cpu: &'a dyn Computer,
    /// Segment being fetched from.
    pub seg: u16,
    /// Current fetch offset (advances as bytes are consumed).
    pub offset: u16,
    /// Accumulated raw bytes.
    pub bytes: Vec<u8>,
    /// Active segment override prefix (if any).
    pub seg_override: Option<SegReg>,
}

impl<'a> Cursor<'a> {
    pub fn new(cpu: &'a dyn Computer, seg: u16, offset: u16) -> Self {
        Self {
            cpu,
            seg,
            offset,
            bytes: Vec::new(),
            seg_override: None,
        }
    }

    /// Fetch the next instruction byte and advance the cursor.
    pub fn fetch(&mut self) -> u8 {
        let phys = ((self.seg as u32) << 4).wrapping_add(self.offset as u32);
        let b = self.cpu.read_u8(phys);
        self.bytes.push(b);
        self.offset = self.offset.wrapping_add(1);
        b
    }

    /// Fetch a little-endian 16-bit word.
    pub fn fetch16(&mut self) -> u16 {
        let lo = self.fetch() as u16;
        let hi = self.fetch() as u16;
        lo | (hi << 8)
    }

    pub fn read_mem_u8(&self, seg: u16, ea: u16) -> u8 {
        self.cpu
            .read_u8(((seg as u32) << 4).wrapping_add(ea as u32))
    }

    pub fn read_mem_u16(&self, seg: u16, ea: u16) -> u16 {
        let lo = self.read_mem_u8(seg, ea) as u16;
        let hi = self.read_mem_u8(seg, ea.wrapping_add(1)) as u16;
        lo | (hi << 8)
    }

    /// Resolve which segment to use for a given RmBase (honouring any prefix override).
    pub fn seg_for_base(&self, base: RmBase) -> u16 {
        self.seg_override
            .map(|s| self.seg_val(s))
            .unwrap_or_else(|| base.default_seg(self.cpu))
    }

    /// Resolve which segment to use for a direct [imm16] address.
    pub fn seg_for_direct(&self) -> u16 {
        self.seg_override
            .map(|s| self.seg_val(s))
            .unwrap_or_else(|| self.cpu.ds())
    }

    pub fn seg_val(&self, s: SegReg) -> u16 {
        match s {
            SegReg::ES => self.cpu.es(),
            SegReg::CS => self.cpu.cs(),
            SegReg::SS => self.cpu.ss(),
            SegReg::DS => self.cpu.ds(),
        }
    }
}

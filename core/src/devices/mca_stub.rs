use std::any::Any;

use crate::Device;

// MCA channel/slot setup ports
const PORT_CHANNEL_SELECT: u16 = 0x0094;
const PORT_ADAPTER_SELECT: u16 = 0x0096;

// POS (Programmable Option Select) register ranges — two access windows
const PORT_POS_ALT_START: u16 = 0xF000;
const PORT_POS_ALT_END: u16 = 0xF007;
const PORT_POS_START: u16 = 0x0100;
const PORT_POS_END: u16 = 0x0107;

/// Stub for MCA (Micro Channel Architecture) bus hardware.
///
/// Real MCA PS/2 systems expose slot-selection ports (0x0094, 0x0096) and
/// two windows into per-slot POS registers (0x0100-0x0107 and 0xF000-0xF007).
/// We emulate an ISA-only machine, so all POS reads return 0xFF (no adapter)
/// and slot-select writes are silently accepted.
pub struct McaStub {
    channel_select: u8,
    adapter_select: u8,
}

impl McaStub {
    pub fn new() -> Self {
        Self {
            channel_select: 0xFF,
            adapter_select: 0x00,
        }
    }
}

impl Device for McaStub {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.channel_select = 0xFF;
        self.adapter_select = 0x00;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            PORT_CHANNEL_SELECT => Some(self.channel_select),
            PORT_ADAPTER_SELECT => Some(self.adapter_select),
            PORT_POS_ALT_START..=PORT_POS_ALT_END => Some(0xFF),
            PORT_POS_START..=PORT_POS_END => Some(0xFF),
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        match port {
            PORT_CHANNEL_SELECT => {
                self.channel_select = val;
                true
            }
            PORT_ADAPTER_SELECT => {
                self.adapter_select = val;
                true
            }
            PORT_POS_ALT_START..=PORT_POS_ALT_END | PORT_POS_START..=PORT_POS_END => true,
            _ => false,
        }
    }
}

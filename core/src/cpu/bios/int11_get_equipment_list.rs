use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::bda::{BDA_EQUIPMENT_LIST, BDA_START},
    },
};

impl Cpu {
    /// INT 0x11 - Get Equipment List
    /// Returns the equipment configuration word from the BIOS Data Area
    /// Input: None
    /// Output: AX = equipment list word
    ///
    /// Equipment list bits:
    /// - Bit 0: Floppy drive installed
    /// - Bits 1: Math coprocessor installed
    /// - Bits 4-5: Initial video mode (00=reserved, 01=40x25 color, 10=80x25 color, 11=80x25 mono)
    /// - Bits 6-7: Number of floppy drives minus 1
    /// - Bits 9-11: Number of serial ports
    /// - Bits 14-15: Number of printers
    pub(in crate::cpu) fn handle_int11_get_equipment_list(&mut self, bus: &mut Bus) {
        // Read equipment list from BDA at offset 0x10 (2 bytes)
        let equipment = bus.memory_read_u16(BDA_START + BDA_EQUIPMENT_LIST);
        self.ax = equipment;
    }
}

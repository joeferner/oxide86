use crate::{
    bus::Bus,
    cpu::{Cpu, bios::bda::bda_get_equipment_list},
};

impl Cpu {
    /// INT 0x11 - Get Equipment List
    /// Returns the equipment configuration word from the BIOS Data Area
    /// Input: None
    /// Output: AX = equipment list word
    pub(in crate::cpu) fn handle_int11_get_equipment_list(&mut self, bus: &mut Bus) {
        let equipment = bda_get_equipment_list(bus);
        self.ax = equipment;
    }
}

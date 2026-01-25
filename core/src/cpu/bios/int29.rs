use super::Bios;
use crate::cpu::Cpu;
use crate::memory::Memory;

impl Cpu {
    /// Handle INT 29h - Fast Console Output
    /// This is used by DOS for faster character output than INT 21h AH=02h
    pub(super) fn handle_int29<T: Bios>(&mut self, _memory: &mut Memory, io: &mut T) {
        // AL = character to output
        let ch = (self.ax & 0xFF) as u8;

        // Output the character
        io.write_char(ch);
    }
}

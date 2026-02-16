use crate::Bus;
use crate::cpu::Cpu;

impl Cpu {
    /// Handle INT 29h - Fast Console Output
    /// This is used by DOS for faster character output than INT 21h AH=02h
    /// Routes through the video teletype output for proper screen handling
    pub(super) fn handle_int29(&mut self, bus: &mut Bus) {
        // AL = character to output
        // Use teletype output (same as INT 10h, AH=0Eh) for proper scrolling
        self.int10_teletype_output(bus);
    }
}

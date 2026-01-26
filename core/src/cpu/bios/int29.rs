use crate::cpu::Cpu;

impl Cpu {
    /// Handle INT 29h - Fast Console Output
    /// This is used by DOS for faster character output than INT 21h AH=02h
    /// Routes through the video teletype output for proper screen handling
    pub(super) fn handle_int29(&mut self, video: &mut crate::video::Video) {
        // AL = character to output
        // Use teletype output (same as INT 10h, AH=0Eh) for proper scrolling
        self.int10_teletype_output(video);
    }
}

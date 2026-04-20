pub(super) struct SoundBlasterMixer {
    regs: [u8; 256],
    index: u8,
}

impl SoundBlasterMixer {
    pub(super) fn new() -> Self {
        let mut regs = [0u8; 256];
        regs[0x22] = 0xFF; // master volume full (SBPro)
        regs[0x26] = 0xFF; // FM volume full
        regs[0x28] = 0xFF; // CD volume full
        regs[0x30] = 0xF8; // master L (SB16)
        regs[0x31] = 0xF8; // master R (SB16)
        regs[0x32] = 0xF8; // voice L (SB16)
        regs[0x33] = 0xF8; // voice R (SB16)
        regs[0x80] = 0x04; // IRQ select: IRQ5
        regs[0x81] = 0x02; // DMA select: DMA1
        Self { regs, index: 0 }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn current_index(&self) -> u8 {
        self.index
    }

    pub(super) fn write_index(&mut self, val: u8) {
        self.index = val;
    }

    pub(super) fn read_data(&self) -> u8 {
        self.regs[self.index as usize]
    }

    pub(super) fn write_data(&mut self, val: u8) {
        if self.index == 0x00 {
            self.reset();
            return;
        }
        // 0x82 is read-only IRQ status — ignore writes
        if self.index != 0x82 {
            self.regs[self.index as usize] = val;
        }
    }
}

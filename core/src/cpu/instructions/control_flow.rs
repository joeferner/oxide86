use crate::{
    cpu::{Cpu, cpu_flag, timing},
    memory_bus::MemoryBus,
};

impl Cpu {
    /// HLT - Halt (opcode F4)
    /// Stops instruction execution until a hardware interrupt occurs
    pub(in crate::cpu) fn hlt(&mut self) {
        self.halted = true;

        // HLT: 2 cycles
        self.last_instruction_cycles = timing::cycles::HLT;
    }

    /// INT - Software Interrupt (opcode CD)
    /// Calls interrupt handler
    pub(in crate::cpu) fn int(&mut self, bus: &mut MemoryBus) {
        let int_num = self.fetch_byte(bus);

        // Push flags, CS, and IP
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);
        // Clear IF and TF
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);
        // Load interrupt vector from interrupt vector table (IVT)
        // IVT starts at 0x00000, each entry is 4 bytes (offset, segment)
        let ivt_addr = (int_num as usize) * 4;
        let offset = bus.read_u16(ivt_addr);
        let segment = bus.read_u16(ivt_addr + 2);
        self.ip = offset;
        self.cs = segment;

        // INT: 51 cycles
        self.last_instruction_cycles = timing::cycles::INT;
    }

    /// IRET - Interrupt Return (opcode CF)
    /// Returns from interrupt handler
    pub(in crate::cpu) fn iret(&mut self, memory_bus: &mut MemoryBus) {
        // Pop IP, CS, and flags
        let new_ip = self.pop(memory_bus);
        let new_cs = self.pop(memory_bus);
        let new_flags = self.pop(memory_bus);

        self.ip = new_ip;
        self.cs = new_cs;
        // 8086 behavior: only allow bits 0-11 to be modified, force bit 1 to 1
        self.flags = (new_flags & 0x0FFF) | 0x0002;

        // IRET: 24 cycles
        self.last_instruction_cycles = timing::cycles::IRET;
    }

    /// CALL near relative (opcode E8)
    /// Call procedure at IP + signed 16-bit offset
    pub(in crate::cpu) fn call_near(&mut self, memory_bus: &mut MemoryBus) {
        let offset = self.fetch_word(memory_bus) as i16;
        // Push return address (current IP after reading offset)
        self.push(self.ip, memory_bus);
        // Jump to target
        self.ip = self.ip.wrapping_add(offset as u16);

        // CALL near direct: 19 cycles
        self.last_instruction_cycles = timing::cycles::CALL_NEAR_DIRECT;
    }
}

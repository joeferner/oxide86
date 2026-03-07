use crate::bus::Bus;
use crate::cpu::Cpu;
use crate::physical_address;

/// Information about a decoded instruction
pub(crate) struct DecodedInstruction {
    /// Human-readable assembly string
    pub text: String,
    /// Raw bytes that make up this instruction
    pub bytes: Vec<u8>,
    /// Formatted string of input register values (e.g., "AX=1234 CX=5678")
    pub reg_values: String,
    /// Formatted string of memory values (e.g., "[0x24bc]=1234 [bx+4]=5678")
    pub mem_values: String,
}

/// Decode instruction at given CS:IP and return detailed information with register values
impl Cpu {
    pub(in crate::cpu) fn decode_instruction_with_regs(&self, bus: &Bus) -> DecodedInstruction {
        let mut decoder = InstructionDecoder::new(bus, self.cs, self.ip);
        let text = decoder.decode();

        let reg_values = decoder.format_input_registers(self);
        let mem_values = decoder.format_memory_values(self);

        // Collect the raw bytes consumed during decoding
        let bytes: Vec<u8> = {
            let start = ((self.cs as usize) << 4) + (self.ip as usize);
            let end = ((self.cs as usize) << 4) + (decoder.ip as usize);
            (start..end).map(|addr| bus.memory_read_u8(addr)).collect()
        };

        DecodedInstruction {
            text,
            bytes,
            reg_values,
            mem_values,
        }
    }
}

/// Information about a memory reference in the instruction
#[derive(Clone)]
struct MemoryRef {
    /// Display string (e.g., "[0x24bc]", "[bx+si+4]")
    display: String,
    /// ModR/M byte that encodes this memory reference (if applicable)
    modrm: Option<u8>,
    /// Segment override, or None to use default
    segment_override: Option<u16>,
    /// Direct address (for mode 00, rm 110)
    direct_address: Option<u16>,
    /// Displacement value (0 for no displacement, 8-bit or 16-bit)
    displacement: i16,
    /// Whether this is a word (16-bit) access
    is_word: bool,
}

fn port_name(port: u16) -> Option<&'static str> {
    match port {
        // PIC
        0x0020 => Some("PIC1 Command"),
        0x0021 => Some("PIC1 Mask"),
        0x00A0 => Some("PIC2 Command"),
        0x00A1 => Some("PIC2 Mask"),
        // PIT
        0x0040 => Some("PIT Channel 0"),
        0x0041 => Some("PIT Channel 1"),
        0x0042 => Some("PIT Channel 2"),
        0x0043 => Some("PIT Control"),
        // System Control Port B
        0x0061 => Some("System Control Port B"),
        // Keyboard Controller
        0x0060 => Some("KBC Data"),
        0x0064 => Some("KBC Status/Command"),
        // RTC
        0x0070 => Some("RTC Register Select"),
        0x0071 => Some("RTC Data"),
        // CGA/VGA CRTC
        0x03B4 => Some("MDA CRTC Address"),
        0x03B5 => Some("MDA CRTC Data"),
        0x03C0 => Some("VGA AC Address/Data"),
        0x03C1 => Some("VGA AC Data Read"),
        0x03C7 => Some("VGA DAC Read Index"),
        0x03C8 => Some("VGA DAC Write Index"),
        0x03C9 => Some("VGA DAC Data"),
        0x03D4 => Some("CGA CRTC Address"),
        0x03D5 => Some("CGA CRTC Data"),
        0x03D9 => Some("CGA Color Select"),
        0x03DA => Some("CGA Status"),
        // FDC
        0x03F2 => Some("FDC DOR"),
        0x03F4 => Some("FDC Status"),
        0x03F5 => Some("FDC Data"),
        0x03F7 => Some("FDC DIR"),
        // HDC
        0x01F0 => Some("HDC Data"),
        0x01F1 => Some("HDC Error/Features"),
        0x01F2 => Some("HDC Sector Count"),
        0x01F3 => Some("HDC Sector Number"),
        0x01F4 => Some("HDC Cylinder Low"),
        0x01F5 => Some("HDC Cylinder High"),
        0x01F6 => Some("HDC Drive/Head"),
        0x01F7 => Some("HDC Command/Status"),
        0x03F6 => Some("HDC Device Control"),
        // UART (COM ports)
        0x03F8 => Some("COM1 Data"),
        0x03F9 => Some("COM1 IER/DLM"),
        0x03FA => Some("COM1 IIR/FCR"),
        0x03FB => Some("COM1 LCR"),
        0x03FC => Some("COM1 MCR"),
        0x03FD => Some("COM1 LSR"),
        0x03FE => Some("COM1 MSR"),
        0x02F8 => Some("COM2 Data"),
        0x02F9 => Some("COM2 IER/DLM"),
        0x02FA => Some("COM2 IIR/FCR"),
        0x02FB => Some("COM2 LCR"),
        0x02FC => Some("COM2 MCR"),
        0x02FD => Some("COM2 LSR"),
        0x02FE => Some("COM2 MSR"),
        0x03E8 => Some("COM3 Data"),
        0x02E8 => Some("COM4 Data"),
        _ => None,
    }
}

struct InstructionDecoder<'a> {
    bus: &'a Bus,
    cs: u16,
    ip: u16,
    segment_override: Option<&'static str>,
    repeat_prefix: Option<&'static str>,
    uses_ax: bool,
    uses_bx: bool,
    uses_cx: bool,
    uses_dx: bool,
    /// Memory references found in this instruction
    memory_refs: Vec<MemoryRef>,
}

impl<'a> InstructionDecoder<'a> {
    fn new(bus: &'a Bus, cs: u16, ip: u16) -> Self {
        Self {
            bus,
            cs,
            ip,
            segment_override: None,
            repeat_prefix: None,
            uses_ax: false,
            uses_bx: false,
            uses_cx: false,
            uses_dx: false,
            memory_refs: Vec::new(),
        }
    }

    fn mark_reg_input(&mut self, reg_name: &str) {
        match reg_name {
            "ax" | "al" | "ah" => self.uses_ax = true,
            "bx" | "bl" | "bh" => self.uses_bx = true,
            "cx" | "cl" | "ch" => self.uses_cx = true,
            "dx" | "dl" | "dh" => self.uses_dx = true,
            _ => {}
        }
    }

    fn format_input_registers(&self, cpu: &Cpu) -> String {
        let mut parts = Vec::new();
        if self.uses_ax {
            parts.push(format!("AX={:04X}", cpu.ax));
        }
        if self.uses_bx {
            parts.push(format!("BX={:04X}", cpu.bx));
        }
        if self.uses_cx {
            parts.push(format!("CX={:04X}", cpu.cx));
        }
        if self.uses_dx {
            parts.push(format!("DX={:04X}", cpu.dx));
        }
        parts.join(" ")
    }

    fn format_memory_values(&self, cpu: &Cpu) -> String {
        let mut parts = Vec::new();

        for mem_ref in &self.memory_refs {
            // Calculate the effective address
            let address = if let Some(direct_addr) = mem_ref.direct_address {
                // Direct addressing mode [0x1234]
                // Use segment override if present, otherwise DS
                let segment = match mem_ref.segment_override {
                    Some(0) => cpu.es,
                    Some(1) => cpu.cs,
                    Some(2) => cpu.ss,
                    Some(3) | None => cpu.ds, // DS is default
                    Some(4) => cpu.es,        // FS -> ES for 8086
                    Some(5) => cpu.es,        // GS -> ES for 8086
                    _ => cpu.ds,
                };
                physical_address(segment, direct_addr)
            } else if let Some(modrm) = mem_ref.modrm {
                // Calculate effective address from modrm byte and CPU registers
                let rm = modrm & 0x07;

                // Calculate base offset based on addressing mode
                let base_offset = match rm {
                    0b000 => cpu.bx.wrapping_add(cpu.si),
                    0b001 => cpu.bx.wrapping_add(cpu.di),
                    0b010 => cpu.bp.wrapping_add(cpu.si),
                    0b011 => cpu.bp.wrapping_add(cpu.di),
                    0b100 => cpu.si,
                    0b101 => cpu.di,
                    0b110 => cpu.bp,
                    0b111 => cpu.bx,
                    _ => 0,
                };

                // Add displacement to get final offset
                let offset = base_offset.wrapping_add(mem_ref.displacement as u16);

                // Determine default segment (BP-based uses SS, others use DS)
                let default_segment = if matches!(rm, 0b010 | 0b011 | 0b110) {
                    cpu.ss
                } else {
                    cpu.ds
                };

                // Apply segment override if present
                let segment = match mem_ref.segment_override {
                    Some(0) => cpu.es,
                    Some(1) => cpu.cs,
                    Some(2) => cpu.ss,
                    Some(3) => cpu.ds,
                    Some(4) => cpu.es, // FS -> ES for 8086
                    Some(5) => cpu.es, // GS -> ES for 8086
                    None => default_segment,
                    _ => default_segment,
                };

                physical_address(segment, offset)
            } else {
                continue; // Skip if we can't calculate the address
            };

            // Read the value from memory
            let value = if mem_ref.is_word {
                self.bus.memory_read_u16(address)
            } else {
                self.bus.memory_read_u8(address) as u16
            };

            // Calculate segment for display (only for direct addresses without override)
            let segment = if mem_ref.direct_address.is_some() {
                match mem_ref.segment_override {
                    Some(0) => cpu.es,
                    Some(1) => cpu.cs,
                    Some(2) => cpu.ss,
                    Some(3) | None => cpu.ds,
                    Some(4) => cpu.es,
                    Some(5) => cpu.es,
                    _ => cpu.ds,
                }
            } else if let Some(modrm) = mem_ref.modrm {
                let rm = modrm & 0x07;
                let default_segment = if matches!(rm, 0b010 | 0b011 | 0b110) {
                    cpu.ss
                } else {
                    cpu.ds
                };
                match mem_ref.segment_override {
                    Some(0) => cpu.es,
                    Some(1) => cpu.cs,
                    Some(2) => cpu.ss,
                    Some(3) => cpu.ds,
                    Some(4) => cpu.es,
                    Some(5) => cpu.es,
                    None => default_segment,
                    _ => default_segment,
                }
            } else {
                cpu.ds
            };

            // Format as "display=value @seg:off(linear)"
            let offset = address - ((segment as usize) << 4);
            if mem_ref.is_word {
                parts.push(format!(
                    "{}={:04X} @{:04X}:{:04X}({:05X})",
                    mem_ref.display, value, segment, offset, address,
                ));
            } else {
                parts.push(format!(
                    "{}={:02X} @{:04X}:{:04X}({:05X})",
                    mem_ref.display, value as u8, segment, offset, address,
                ));
            }
        }

        parts.join(" ")
    }

    fn physical_address(segment: u16, offset: u16) -> usize {
        ((segment as usize) << 4) + (offset as usize)
    }

    fn fetch_byte(&mut self) -> u8 {
        let addr = Self::physical_address(self.cs, self.ip);
        let byte = self.bus.memory_read_u8(addr);
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    fn peek_byte(&self) -> u8 {
        let addr = Self::physical_address(self.cs, self.ip);
        self.bus.memory_read_u8(addr)
    }

    fn reg8_name(reg: u8) -> &'static str {
        match reg {
            0 => "al",
            1 => "cl",
            2 => "dl",
            3 => "bl",
            4 => "ah",
            5 => "ch",
            6 => "dh",
            7 => "bh",
            _ => "?",
        }
    }

    fn reg16_name(reg: u8) -> &'static str {
        match reg & 0x07 {
            0 => "ax",
            1 => "cx",
            2 => "dx",
            3 => "bx",
            4 => "sp",
            5 => "bp",
            6 => "si",
            7 => "di",
            _ => "?",
        }
    }

    fn segreg_name(reg: u8) -> &'static str {
        match reg & 0x03 {
            0 => "es",
            1 => "cs",
            2 => "ss",
            3 => "ds",
            _ => "?",
        }
    }

    fn decode_modrm(&mut self, w: bool) -> (u8, u8, String) {
        let modrm = self.fetch_byte();
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        if mode == 0b11 {
            // Register mode
            let rm_str = if w {
                Self::reg16_name(rm).to_string()
            } else {
                Self::reg8_name(rm).to_string()
            };
            return (reg, rm, rm_str);
        }

        // Memory mode - build effective address string
        let mut ea = String::new();

        // Add segment override if present
        let segment_override_value = match self.segment_override {
            Some("es") => Some(0), // Will be resolved to ES register value later
            Some("cs") => Some(1),
            Some("ss") => Some(2),
            Some("ds") => Some(3),
            Some("fs") => Some(4),
            Some("gs") => Some(5),
            _ => None,
        };

        if let Some(seg) = self.segment_override {
            ea.push_str(seg);
            ea.push(':');
        }

        ea.push('[');

        // Track displacement
        let mut displacement: i16 = 0;

        // Base addressing mode
        let base = match rm {
            0b000 => "bx+si",
            0b001 => "bx+di",
            0b010 => "bp+si",
            0b011 => "bp+di",
            0b100 => "si",
            0b101 => "di",
            0b110 => {
                if mode == 0b00 {
                    // Direct address
                    let disp = self.fetch_word();
                    ea.push_str(&format!("0x{:04x}]", disp));

                    // Record this memory reference
                    self.memory_refs.push(MemoryRef {
                        display: ea.clone(),
                        modrm: None,
                        segment_override: segment_override_value,
                        direct_address: Some(disp),
                        displacement: 0,
                        is_word: w,
                    });

                    return (reg, rm, ea);
                } else {
                    "bp"
                }
            }
            0b111 => "bx",
            _ => "?",
        };

        ea.push_str(base);

        // Add displacement
        match mode {
            0b00 => {} // No displacement
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte() as i8;
                displacement = disp as i16;
                if disp >= 0 {
                    ea.push_str(&format!("+0x{:02x}", disp));
                } else {
                    ea.push_str(&format!("-0x{:02x}", -(disp as i16)));
                }
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word();
                displacement = disp as i16;
                if disp > 0 {
                    ea.push_str(&format!("+0x{:04x}", disp));
                }
            }
            _ => {}
        }

        ea.push(']');

        // Record this memory reference (non-direct addressing)
        self.memory_refs.push(MemoryRef {
            display: ea.clone(),
            modrm: Some(modrm),
            segment_override: segment_override_value,
            direct_address: None,
            displacement,
            is_word: w,
        });

        (reg, rm, ea)
    }

    fn decode_rm_reg(&mut self, opcode: u8, mnemonic: &str) -> String {
        let d = (opcode >> 1) & 1; // Direction bit
        let w = opcode & 1; // Width bit

        let (reg, rm, rm_str) = self.decode_modrm(w == 1);
        let reg_str = if w == 1 {
            Self::reg16_name(reg)
        } else {
            Self::reg8_name(reg)
        };

        // Track register inputs
        let is_arithmetic = matches!(
            mnemonic,
            "add" | "sub" | "and" | "or" | "xor" | "adc" | "sbb" | "cmp" | "test"
        );

        if d == 1 {
            // Register is destination, rm is source
            self.mark_rm_input(rm, w == 1);
            if is_arithmetic {
                // Destination is also read in arithmetic ops
                self.mark_reg_input(reg_str);
            }
            format!("{} {}, {}", mnemonic, reg_str, rm_str)
        } else {
            // R/M is destination, register is source
            self.mark_reg_input(reg_str);
            if is_arithmetic {
                // Destination is also read in arithmetic ops
                self.mark_rm_input(rm, w == 1);
            }
            format!("{} {}, {}", mnemonic, rm_str, reg_str)
        }
    }

    fn mark_rm_input(&mut self, rm: u8, is_16bit: bool) {
        let reg_name = if is_16bit {
            Self::reg16_name(rm)
        } else {
            Self::reg8_name(rm)
        };
        self.mark_reg_input(reg_name);
    }

    fn decode_imm_acc(&mut self, opcode: u8, mnemonic: &str) -> String {
        let w = opcode & 1;

        // For arithmetic operations, accumulator is read
        let is_arithmetic = matches!(
            mnemonic,
            "add" | "sub" | "and" | "or" | "xor" | "adc" | "sbb" | "cmp" | "test"
        );
        if is_arithmetic {
            self.mark_reg_input(if w == 1 { "ax" } else { "al" });
        }

        if w == 1 {
            let imm = self.fetch_word();
            format!("{} ax, 0x{:04x}", mnemonic, imm)
        } else {
            let imm = self.fetch_byte();
            format!("{} al, 0x{:02x}", mnemonic, imm)
        }
    }

    fn decode_arith_imm_rm(&mut self, w: bool, sign_extend: bool) -> String {
        let (reg, _rm, rm_str) = self.decode_modrm(w);

        let mnemonic = match reg {
            0 => "add",
            1 => "or",
            2 => "adc",
            3 => "sbb",
            4 => "and",
            5 => "sub",
            6 => "xor",
            7 => "cmp",
            _ => "?",
        };

        let imm_str = if w {
            if sign_extend {
                // Sign-extended 8-bit immediate to 16-bit
                let imm = self.fetch_byte() as i8 as i16 as u16;
                format!("0x{:04x}", imm)
            } else {
                let imm = self.fetch_word();
                format!("0x{:04x}", imm)
            }
        } else {
            let imm = self.fetch_byte();
            format!("0x{:02x}", imm)
        };

        format!("{} {}, {}", mnemonic, rm_str, imm_str)
    }

    fn decode_shift_rotate(&mut self, opcode: u8) -> String {
        let w = opcode & 1;
        let count_in_cl = (opcode >> 1) & 1;
        let use_imm = (0xC0..=0xC1).contains(&opcode);

        let (reg, _rm, rm_str) = self.decode_modrm(w == 1);

        let mnemonic = match reg {
            0 => "rol",
            1 => "ror",
            2 => "rcl",
            3 => "rcr",
            4 => "shl",
            5 => "shr",
            6 => "sal",
            7 => "sar",
            _ => "?",
        };

        let count_str = if use_imm {
            let count = self.fetch_byte();
            format!("0x{:02x}", count)
        } else if count_in_cl == 1 {
            "cl".to_string()
        } else {
            "1".to_string()
        };

        format!("{} {}, {}", mnemonic, rm_str, count_str)
    }

    fn decode_group3(&mut self, w: bool) -> String {
        let (reg, _rm, rm_str) = self.decode_modrm(w);

        match reg {
            0 | 1 => {
                // TEST with immediate
                let imm_str = if w {
                    let imm = self.fetch_word();
                    format!("0x{:04x}", imm)
                } else {
                    let imm = self.fetch_byte();
                    format!("0x{:02x}", imm)
                };
                format!("test {}, {}", rm_str, imm_str)
            }
            2 => format!("not {}", rm_str),
            3 => format!("neg {}", rm_str),
            4 => format!("mul {}", rm_str),
            5 => format!("imul {}", rm_str),
            6 => format!("div {}", rm_str),
            7 => format!("idiv {}", rm_str),
            _ => format!("? {}", rm_str),
        }
    }

    pub(crate) fn decode(&mut self) -> String {
        let opcode = self.fetch_byte();

        match opcode {
            // Segment override prefixes
            0x26 => {
                self.segment_override = Some("es");
                let rest = self.decode();
                format!("es: {}", rest.trim_start_matches("es: "))
            }
            0x2E => {
                self.segment_override = Some("cs");
                let rest = self.decode();
                format!("cs: {}", rest.trim_start_matches("cs: "))
            }
            0x36 => {
                self.segment_override = Some("ss");
                let rest = self.decode();
                format!("ss: {}", rest.trim_start_matches("ss: "))
            }
            0x3E => {
                self.segment_override = Some("ds");
                let rest = self.decode();
                format!("ds: {}", rest.trim_start_matches("ds: "))
            }
            0x64 => {
                self.segment_override = Some("fs");
                let rest = self.decode();
                format!("fs: {}", rest.trim_start_matches("fs: "))
            }
            0x65 => {
                self.segment_override = Some("gs");
                let rest = self.decode();
                format!("gs: {}", rest.trim_start_matches("gs: "))
            }

            // Repeat prefixes
            0xF2 => {
                self.repeat_prefix = Some("repne");
                let rest = self.decode();
                format!("repne {}", rest)
            }
            0xF3 => {
                self.repeat_prefix = Some("rep");
                let rest = self.decode();
                format!("rep {}", rest)
            }

            // LOCK prefix
            0xF0 => {
                let rest = self.decode();
                format!("lock {}", rest)
            }

            // ADD
            0x00..=0x03 => self.decode_rm_reg(opcode, "add"),
            0x04..=0x05 => self.decode_imm_acc(opcode, "add"),

            // PUSH/POP segment registers
            0x06 => "push es".to_string(),
            0x07 => "pop es".to_string(),
            0x0E => "push cs".to_string(),
            0x16 => "push ss".to_string(),
            0x17 => "pop ss".to_string(),
            0x1E => "push ds".to_string(),
            0x1F => "pop ds".to_string(),

            // OR
            0x08..=0x0B => self.decode_rm_reg(opcode, "or"),
            0x0C..=0x0D => self.decode_imm_acc(opcode, "or"),

            // ADC
            0x10..=0x13 => self.decode_rm_reg(opcode, "adc"),
            0x14..=0x15 => self.decode_imm_acc(opcode, "adc"),

            // SBB
            0x18..=0x1B => self.decode_rm_reg(opcode, "sbb"),
            0x1C..=0x1D => self.decode_imm_acc(opcode, "sbb"),

            // AND
            0x20..=0x23 => self.decode_rm_reg(opcode, "and"),
            0x24..=0x25 => self.decode_imm_acc(opcode, "and"),
            0x27 => "daa".to_string(),

            // SUB
            0x28..=0x2B => self.decode_rm_reg(opcode, "sub"),
            0x2C..=0x2D => self.decode_imm_acc(opcode, "sub"),
            0x2F => "das".to_string(),

            // XOR
            0x30..=0x33 => self.decode_rm_reg(opcode, "xor"),
            0x34..=0x35 => self.decode_imm_acc(opcode, "xor"),
            0x37 => "aaa".to_string(),

            // CMP
            0x38..=0x3B => self.decode_rm_reg(opcode, "cmp"),
            0x3C..=0x3D => self.decode_imm_acc(opcode, "cmp"),
            0x3F => "aas".to_string(),

            // INC 16-bit registers
            0x40..=0x47 => {
                let reg = opcode & 0x07;
                let reg_name = Self::reg16_name(reg);
                self.mark_reg_input(reg_name);
                format!("inc {}", reg_name)
            }

            // DEC 16-bit registers
            0x48..=0x4F => {
                let reg = opcode & 0x07;
                let reg_name = Self::reg16_name(reg);
                self.mark_reg_input(reg_name);
                format!("dec {}", reg_name)
            }

            // PUSH 16-bit registers
            0x50..=0x57 => {
                let reg = opcode & 0x07;
                let reg_name = Self::reg16_name(reg);
                self.mark_reg_input(reg_name);
                format!("push {}", reg_name)
            }

            // POP 16-bit registers
            0x58..=0x5F => {
                let reg = opcode & 0x07;
                format!("pop {}", Self::reg16_name(reg))
            }

            // PUSHA
            0x60 => "pusha".to_string(),

            // BOUND
            0x62 => {
                let (_reg, _rm, rm_str) = self.decode_modrm(true);
                format!("bound {}", rm_str)
            }

            // PUSH immediate
            0x68 => {
                let imm = self.fetch_word();
                format!("push 0x{:04x}", imm)
            }
            0x6A => {
                let imm = self.fetch_byte() as i8;
                format!("push 0x{:02x}", imm as u8)
            }

            // IMUL with immediate
            0x69 => {
                let (reg, _rm, rm_str) = self.decode_modrm(true);
                let imm = self.fetch_word();
                format!("imul {}, {}, 0x{:04x}", Self::reg16_name(reg), rm_str, imm)
            }

            // INS/OUTS
            0x6C => "insb".to_string(),
            0x6D => "insw".to_string(),
            0x6E => "outsb".to_string(),
            0x6F => "outsw".to_string(),

            // Conditional jumps
            0x70 => {
                let offset = self.fetch_byte() as i8;
                format!("jo 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x71 => {
                let offset = self.fetch_byte() as i8;
                format!("jno 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x72 => {
                let offset = self.fetch_byte() as i8;
                format!("jb 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x73 => {
                let offset = self.fetch_byte() as i8;
                format!("jnb 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x74 => {
                let offset = self.fetch_byte() as i8;
                format!("jz 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x75 => {
                let offset = self.fetch_byte() as i8;
                format!("jnz 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x76 => {
                let offset = self.fetch_byte() as i8;
                format!("jbe 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x77 => {
                let offset = self.fetch_byte() as i8;
                format!("ja 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x78 => {
                let offset = self.fetch_byte() as i8;
                format!("js 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x79 => {
                let offset = self.fetch_byte() as i8;
                format!("jns 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x7A => {
                let offset = self.fetch_byte() as i8;
                format!("jp 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x7B => {
                let offset = self.fetch_byte() as i8;
                format!("jnp 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x7C => {
                let offset = self.fetch_byte() as i8;
                format!("jl 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x7D => {
                let offset = self.fetch_byte() as i8;
                format!("jge 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x7E => {
                let offset = self.fetch_byte() as i8;
                format!("jle 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0x7F => {
                let offset = self.fetch_byte() as i8;
                format!("jg 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }

            // Arithmetic/logical immediate to r/m (0x82 is same as 0x80)
            0x80 | 0x82 => self.decode_arith_imm_rm(false, false),
            0x81 => self.decode_arith_imm_rm(true, false),
            0x83 => self.decode_arith_imm_rm(true, true),

            // TEST
            0x84..=0x85 => self.decode_rm_reg(opcode, "test"),

            // XCHG
            0x86..=0x87 => self.decode_rm_reg(opcode, "xchg"),

            // MOV
            0x88..=0x8B => self.decode_rm_reg(opcode, "mov"),

            // MOV segment register to r/m16
            0x8C => {
                let (reg, _rm, rm_str) = self.decode_modrm(true);
                format!("mov {}, {}", rm_str, Self::segreg_name(reg))
            }

            // LEA
            0x8D => {
                let (reg, _rm, rm_str) = self.decode_modrm(true);
                format!("lea {}, {}", Self::reg16_name(reg), rm_str)
            }

            // MOV r/m16 to segment register
            0x8E => {
                let (reg, rm, rm_str) = self.decode_modrm(true);
                self.mark_rm_input(rm, true);
                format!("mov {}, {}", Self::segreg_name(reg), rm_str)
            }

            // POP r/m16
            0x8F => {
                let (_reg, _rm, rm_str) = self.decode_modrm(true);
                format!("pop {}", rm_str)
            }

            // NOP / XCHG AX with registers
            0x90 => "nop".to_string(),
            0x91..=0x97 => {
                let reg = opcode & 0x07;
                format!("xchg ax, {}", Self::reg16_name(reg))
            }

            // CBW/CWD
            0x98 => "cbw".to_string(),
            0x99 => "cwd".to_string(),

            // CALL/JMP far
            0x9A => {
                let offset = self.fetch_word();
                let segment = self.fetch_word();
                format!("call 0x{:04x}:0x{:04x}", segment, offset)
            }

            // PUSHF/POPF
            0x9C => "pushf".to_string(),
            0x9D => "popf".to_string(),

            // SAHF/LAHF
            0x9E => "sahf".to_string(),
            0x9F => "lahf".to_string(),

            // MOV accumulator to/from memory
            0xA0 => {
                let offset = self.fetch_word();
                let display = format!("[0x{:04x}]", offset);
                self.memory_refs.push(MemoryRef {
                    display: display.clone(),
                    modrm: None,
                    segment_override: None,
                    direct_address: Some(offset),
                    displacement: 0,
                    is_word: false,
                });
                format!("mov al, {}", display)
            }
            0xA1 => {
                let offset = self.fetch_word();
                let display = format!("[0x{:04x}]", offset);
                self.memory_refs.push(MemoryRef {
                    display: display.clone(),
                    modrm: None,
                    segment_override: None,
                    direct_address: Some(offset),
                    displacement: 0,
                    is_word: true,
                });
                format!("mov ax, {}", display)
            }
            0xA2 => {
                let offset = self.fetch_word();
                let display = format!("[0x{:04x}]", offset);
                self.memory_refs.push(MemoryRef {
                    display: display.clone(),
                    modrm: None,
                    segment_override: None,
                    direct_address: Some(offset),
                    displacement: 0,
                    is_word: false,
                });
                self.mark_reg_input("al");
                format!("mov {}, al", display)
            }
            0xA3 => {
                let offset = self.fetch_word();
                let display = format!("[0x{:04x}]", offset);
                self.memory_refs.push(MemoryRef {
                    display: display.clone(),
                    modrm: None,
                    segment_override: None,
                    direct_address: Some(offset),
                    displacement: 0,
                    is_word: true,
                });
                self.mark_reg_input("ax");
                format!("mov {}, ax", display)
            }

            // String operations
            0xA4 => "movsb".to_string(),
            0xA5 => "movsw".to_string(),
            0xA6 => "cmpsb".to_string(),
            0xA7 => "cmpsw".to_string(),
            0xA8 => {
                let imm = self.fetch_byte();
                format!("test al, 0x{:02x}", imm)
            }
            0xA9 => {
                let imm = self.fetch_word();
                format!("test ax, 0x{:04x}", imm)
            }
            0xAA => "stosb".to_string(),
            0xAB => "stosw".to_string(),
            0xAC => "lodsb".to_string(),
            0xAD => "lodsw".to_string(),
            0xAE => "scasb".to_string(),
            0xAF => "scasw".to_string(),

            // MOV immediate to register
            0xB0..=0xB7 => {
                let reg = opcode & 0x07;
                let imm = self.fetch_byte();
                format!("mov {}, 0x{:02x}", Self::reg8_name(reg), imm)
            }
            0xB8..=0xBF => {
                let reg = opcode & 0x07;
                let imm = self.fetch_word();
                format!("mov {}, 0x{:04x}", Self::reg16_name(reg), imm)
            }

            // Shift/rotate with immediate or count
            0xC0..=0xC1 | 0xD0..=0xD3 => self.decode_shift_rotate(opcode),

            // RET
            0xC2 => {
                let imm = self.fetch_word();
                format!("ret 0x{:04x}", imm)
            }
            0xC3 => "ret".to_string(),

            // LES/LDS
            0xC4 => {
                let (reg, _rm, rm_str) = self.decode_modrm(true);
                format!("les {}, {}", Self::reg16_name(reg), rm_str)
            }
            0xC5 => {
                let (reg, _rm, rm_str) = self.decode_modrm(true);
                format!("lds {}, {}", Self::reg16_name(reg), rm_str)
            }

            // MOV immediate to r/m
            0xC6 => {
                let (_reg, _rm, rm_str) = self.decode_modrm(false);
                let imm = self.fetch_byte();
                format!("mov {}, 0x{:02x}", rm_str, imm)
            }
            0xC7 => {
                let (_reg, _rm, rm_str) = self.decode_modrm(true);
                let imm = self.fetch_word();
                format!("mov {}, 0x{:04x}", rm_str, imm)
            }

            // RETF
            0xCA => {
                let imm = self.fetch_word();
                format!("retf 0x{:04x}", imm)
            }
            0xCB => "retf".to_string(),

            // INT
            0xCC => "int3".to_string(),
            0xCD => {
                let int_num = self.fetch_byte();
                format!("int 0x{:02x}", int_num)
            }
            0xCE => "into".to_string(),
            0xCF => "iret".to_string(),

            // AAM/AAD
            0xD4 => {
                let base = self.fetch_byte();
                if base == 10 {
                    "aam".to_string()
                } else {
                    format!("aam 0x{:02x}", base)
                }
            }
            0xD5 => {
                let base = self.fetch_byte();
                if base == 10 {
                    "aad".to_string()
                } else {
                    format!("aad 0x{:02x}", base)
                }
            }

            // XLAT
            0xD7 => "xlat".to_string(),

            // ESC (coprocessor escape)
            0xD8..=0xDF => {
                let modrm = self.peek_byte();
                self.fetch_byte(); // consume modrm
                format!("esc 0x{:02x}, 0x{:02x}", opcode & 0x07, modrm)
            }

            // LOOP instructions
            0xE0 => {
                self.mark_reg_input("cx");
                let offset = self.fetch_byte() as i8;
                format!(
                    "loopne 0x{:04x}",
                    self.ip.wrapping_add(offset as i16 as u16)
                )
            }
            0xE1 => {
                self.mark_reg_input("cx");
                let offset = self.fetch_byte() as i8;
                format!("loope 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0xE2 => {
                self.mark_reg_input("cx");
                let offset = self.fetch_byte() as i8;
                format!("loop 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }
            0xE3 => {
                self.mark_reg_input("cx");
                let offset = self.fetch_byte() as i8;
                format!("jcxz 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }

            // IN/OUT
            0xE4 => {
                let port = self.fetch_byte();
                let name = port_name(port as u16)
                    .map(|n| format!(" ; {}", n))
                    .unwrap_or_default();
                format!("in al, 0x{:02x}{}", port, name)
            }
            0xE5 => {
                let port = self.fetch_byte();
                let name = port_name(port as u16)
                    .map(|n| format!(" ; {}", n))
                    .unwrap_or_default();
                format!("in ax, 0x{:02x}{}", port, name)
            }
            0xE6 => {
                let port = self.fetch_byte();
                self.mark_reg_input("al");
                let name = port_name(port as u16)
                    .map(|n| format!(" ; {}", n))
                    .unwrap_or_default();
                format!("out 0x{:02x}, al{}", port, name)
            }
            0xE7 => {
                let port = self.fetch_byte();
                self.mark_reg_input("ax");
                let name = port_name(port as u16)
                    .map(|n| format!(" ; {}", n))
                    .unwrap_or_default();
                format!("out 0x{:02x}, ax{}", port, name)
            }

            // CALL/JMP near/short
            0xE8 => {
                let offset = self.fetch_word() as i16;
                format!("call 0x{:04x}", self.ip.wrapping_add(offset as u16))
            }
            0xE9 => {
                let offset = self.fetch_word() as i16;
                format!("jmp 0x{:04x}", self.ip.wrapping_add(offset as u16))
            }
            0xEA => {
                let offset = self.fetch_word();
                let segment = self.fetch_word();
                format!("jmp 0x{:04x}:0x{:04x}", segment, offset)
            }
            0xEB => {
                let offset = self.fetch_byte() as i8;
                format!("jmp 0x{:04x}", self.ip.wrapping_add(offset as i16 as u16))
            }

            // IN/OUT with DX
            0xEC => "in al, dx".to_string(),
            0xED => "in ax, dx".to_string(),
            0xEE => {
                self.mark_reg_input("al");
                self.mark_reg_input("dx");
                "out dx, al".to_string()
            }
            0xEF => {
                self.mark_reg_input("ax");
                self.mark_reg_input("dx");
                "out dx, ax".to_string()
            }

            // HLT and flag instructions
            0xF4 => "hlt".to_string(),
            0xF5 => "cmc".to_string(),
            0xF6..=0xF7 => self.decode_group3((opcode & 1) == 1),
            0xF8 => "clc".to_string(),
            0xF9 => "stc".to_string(),
            0xFA => "cli".to_string(),
            0xFB => "sti".to_string(),
            0xFC => "cld".to_string(),
            0xFD => "std".to_string(),

            // INC/DEC/CALL/JMP/PUSH Group 4/5
            0xFE => {
                let modrm = self.peek_byte();
                let reg = (modrm >> 3) & 0x07;
                let (_reg, _rm, rm_str) = self.decode_modrm(false);
                match reg {
                    0 => format!("inc {}", rm_str),
                    1 => format!("dec {}", rm_str),
                    _ => format!("? {}", rm_str),
                }
            }
            0xFF => {
                let modrm = self.peek_byte();
                let reg = (modrm >> 3) & 0x07;
                let (_reg, _rm, rm_str) = self.decode_modrm(true);
                match reg {
                    0 => format!("inc {}", rm_str),
                    1 => format!("dec {}", rm_str),
                    2 => format!("call {}", rm_str),
                    3 => format!("call far {}", rm_str),
                    4 => format!("jmp {}", rm_str),
                    5 => format!("jmp far {}", rm_str),
                    6 => format!("push {}", rm_str),
                    _ => format!("? {}", rm_str),
                }
            }

            _ => format!("db 0x{:02x}", opcode),
        }
    }
}

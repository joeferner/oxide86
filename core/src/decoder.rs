use crate::cpu::Cpu;
use crate::memory::Memory;

/// Information about a decoded instruction
pub struct DecodedInstruction {
    /// Human-readable assembly string
    pub text: String,
    /// Formatted string of input register values (e.g., "AX=1234 CX=5678")
    pub reg_values: String,
}

/// Decode instruction at given CS:IP and return human-readable assembly string
pub fn decode_instruction(memory: &Memory, cs: u16, ip: u16) -> String {
    decode_instruction_with_regs(memory, cs, ip, None).text
}

/// Decode instruction at given CS:IP and return detailed information with register values
pub fn decode_instruction_with_regs(
    memory: &Memory,
    cs: u16,
    ip: u16,
    cpu: Option<&Cpu>,
) -> DecodedInstruction {
    let mut decoder = InstructionDecoder::new(memory, cs, ip);
    let text = decoder.decode();

    let reg_values = if let Some(cpu) = cpu {
        decoder.format_input_registers(cpu)
    } else {
        String::new()
    };

    DecodedInstruction { text, reg_values }
}

struct InstructionDecoder<'a> {
    memory: &'a Memory,
    cs: u16,
    ip: u16,
    segment_override: Option<&'static str>,
    repeat_prefix: Option<&'static str>,
    uses_ax: bool,
    uses_bx: bool,
    uses_cx: bool,
    uses_dx: bool,
}

impl<'a> InstructionDecoder<'a> {
    fn new(memory: &'a Memory, cs: u16, ip: u16) -> Self {
        Self {
            memory,
            cs,
            ip,
            segment_override: None,
            repeat_prefix: None,
            uses_ax: false,
            uses_bx: false,
            uses_cx: false,
            uses_dx: false,
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

    fn physical_address(segment: u16, offset: u16) -> usize {
        ((segment as usize) << 4) + (offset as usize)
    }

    fn fetch_byte(&mut self) -> u8 {
        let addr = Self::physical_address(self.cs, self.ip);
        let byte = self.memory.read_u8(addr);
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
        self.memory.read_u8(addr)
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
        if let Some(seg) = self.segment_override {
            ea.push_str(seg);
            ea.push(':');
        }

        ea.push('[');

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
                if disp >= 0 {
                    ea.push_str(&format!("+0x{:02x}", disp));
                } else {
                    ea.push_str(&format!("-0x{:02x}", -disp));
                }
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word();
                if disp > 0 {
                    ea.push_str(&format!("+0x{:04x}", disp));
                }
            }
            _ => {}
        }

        ea.push(']');
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

    pub fn decode(&mut self) -> String {
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

            // Arithmetic/logical immediate to r/m
            0x80 => self.decode_arith_imm_rm(false, false),
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
                format!("mov al, [0x{:04x}]", offset)
            }
            0xA1 => {
                let offset = self.fetch_word();
                format!("mov ax, [0x{:04x}]", offset)
            }
            0xA2 => {
                let offset = self.fetch_word();
                format!("mov [0x{:04x}], al", offset)
            }
            0xA3 => {
                let offset = self.fetch_word();
                format!("mov [0x{:04x}], ax", offset)
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
                format!("in al, 0x{:02x}", port)
            }
            0xE5 => {
                let port = self.fetch_byte();
                format!("in ax, 0x{:02x}", port)
            }
            0xE6 => {
                let port = self.fetch_byte();
                format!("out 0x{:02x}, al", port)
            }
            0xE7 => {
                let port = self.fetch_byte();
                format!("out 0x{:02x}, ax", port)
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
            0xEE => "out dx, al".to_string(),
            0xEF => "out dx, ax".to_string(),

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

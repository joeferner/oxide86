use crate::cpu::instructions::decoder::{self, Instruction, Mnemonic, Operand};
use crate::{ByteReader, Computer};

/// How control flows out of a decoded instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum FlowKind {
    /// Falls through to the next instruction.
    Continue,
    /// Unconditional near jump (target IP).
    Jump(u16),
    /// Unconditional far jump (seg, off).
    JumpFar(u16, u16),
    /// Conditional jump: may fall through OR branch to target IP.
    ConditionalJump(u16),
    /// Near call: falls through after return, also explores callee.
    Call(u16),
    /// Far call (seg, off).
    CallFar(u16, u16),
    /// ret / retf / iret — stops this path.
    Return,
    /// hlt — stops this path.
    Halt,
    /// jmp/call via register or memory — target unknown statically.
    IndirectTransfer,
}

/// Result of disassembling one instruction.
#[derive(Debug, Clone)]
pub struct DisasmResult {
    pub text: String,
    pub bytes: Vec<u8>,
    pub next_ip: u16,
    pub flow: FlowKind,
}

/// A `Computer` implementation backed by a `ByteReader` with zeroed registers.
/// Used for static disassembly where register values are not available.
struct ByteReaderComputer<'a, R> {
    reader: &'a R,
}

impl<R: ByteReader> Computer for ByteReaderComputer<'_, R> {
    fn ax(&self) -> u16 { 0 }
    fn bx(&self) -> u16 { 0 }
    fn cx(&self) -> u16 { 0 }
    fn dx(&self) -> u16 { 0 }
    fn sp(&self) -> u16 { 0 }
    fn bp(&self) -> u16 { 0 }
    fn si(&self) -> u16 { 0 }
    fn di(&self) -> u16 { 0 }
    fn cs(&self) -> u16 { 0 }
    fn ds(&self) -> u16 { 0 }
    fn ss(&self) -> u16 { 0 }
    fn es(&self) -> u16 { 0 }
    fn read_u8(&self, phys: u32) -> u8 { self.reader.read_u8(phys as usize) }
}

/// Decode a single instruction at `cs:ip` using `reader`.
pub fn disasm_one(reader: &impl ByteReader, cs: u16, ip: u16) -> DisasmResult {
    let computer = ByteReaderComputer { reader };
    let instr = decoder::decode(&computer, cs, ip);
    let next_ip = ip.wrapping_add(instr.bytes.len() as u16);
    let flow = classify_instruction_flow(&instr);
    let text = asm_text(&instr);
    DisasmResult { text, bytes: instr.bytes, next_ip, flow }
}

/// Classify the control-flow kind of a decoded instruction.
pub fn classify_instruction_flow(instr: &Instruction) -> FlowKind {
    match &instr.mnemonic {
        Mnemonic::Hlt => FlowKind::Halt,

        Mnemonic::Ret | Mnemonic::RetFar | Mnemonic::Iret => FlowKind::Return,

        Mnemonic::Jmp => match instr.operands.first() {
            Some(Operand::Imm16(target)) => FlowKind::Jump(*target),
            _ => FlowKind::IndirectTransfer,
        },

        Mnemonic::JmpFar => {
            if let (Some(Operand::Imm16(seg)), Some(Operand::Imm16(off))) =
                (instr.operands.get(0), instr.operands.get(1))
            {
                FlowKind::JumpFar(*seg, *off)
            } else {
                FlowKind::IndirectTransfer
            }
        }

        Mnemonic::Call => match instr.operands.first() {
            Some(Operand::Imm16(target)) => FlowKind::Call(*target),
            _ => FlowKind::IndirectTransfer,
        },

        Mnemonic::CallFar => {
            if let (Some(Operand::Imm16(seg)), Some(Operand::Imm16(off))) =
                (instr.operands.get(0), instr.operands.get(1))
            {
                FlowKind::CallFar(*seg, *off)
            } else {
                FlowKind::IndirectTransfer
            }
        }

        Mnemonic::Ja
        | Mnemonic::Jae
        | Mnemonic::Jb
        | Mnemonic::Jbe
        | Mnemonic::Jcxz
        | Mnemonic::Je
        | Mnemonic::Jg
        | Mnemonic::Jge
        | Mnemonic::Jl
        | Mnemonic::Jle
        | Mnemonic::Jne
        | Mnemonic::Jno
        | Mnemonic::Jns
        | Mnemonic::Jo
        | Mnemonic::Jp
        | Mnemonic::Jnp
        | Mnemonic::Js
        | Mnemonic::Loop
        | Mnemonic::Loopz
        | Mnemonic::Loopnz => match instr.operands.first() {
            Some(Operand::Imm16(target)) => FlowKind::ConditionalJump(*target),
            _ => FlowKind::Continue,
        },

        _ => FlowKind::Continue,
    }
}

/// Format just the mnemonic + operands portion of an instruction (no location/bytes/annotations).
pub fn asm_text(instr: &Instruction) -> String {
    match instr.operands.len() {
        0 => instr.mnemonic.to_string(),
        1 => format!("{} {}", instr.mnemonic, instr.operands[0].asm_str()),
        _ => format!(
            "{} {}, {}",
            instr.mnemonic,
            instr.operands[0].asm_str(),
            instr.operands[1].asm_str()
        ),
    }
}

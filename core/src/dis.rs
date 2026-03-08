use crate::ByteReader;
use crate::cpu::instructions::decoder::decode_one_raw;

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

/// Decode a single instruction at `cs:ip` using `reader`.
pub fn disasm_one(reader: &impl ByteReader, cs: u16, ip: u16) -> DisasmResult {
    let (text, bytes, next_ip) = decode_one_raw(reader, cs, ip);
    let flow = classify_flow(&text, next_ip);
    DisasmResult {
        text,
        bytes,
        next_ip,
        flow,
    }
}

fn parse_hex_u16(s: &str) -> Option<u16> {
    u16::from_str_radix(s.strip_prefix("0x").unwrap_or(s), 16).ok()
}

/// Parse a `SEG:OFF` pair from a string like `"0x1234:0x5678"`.
fn parse_seg_off(s: &str) -> Option<(u16, u16)> {
    let (seg_s, off_s) = s.split_once(':')?;
    Some((parse_hex_u16(seg_s.trim())?, parse_hex_u16(off_s.trim())?))
}

fn classify_flow(text: &str, _next_ip: u16) -> FlowKind {
    // Strip any prefix (rep, lock, cs:, es:, etc.) to get the mnemonic
    let text = text.trim();

    // HLT
    if text == "hlt" {
        return FlowKind::Halt;
    }

    // Return instructions
    if matches!(text, "ret" | "retf" | "iret")
        || text.starts_with("ret ")
        || text.starts_with("retf ")
    {
        return FlowKind::Return;
    }

    // Unconditional near jump: "jmp 0xXXXX"
    if let Some(rest) = text.strip_prefix("jmp 0x")
        && let Some(target) = parse_hex_u16(rest)
    {
        return FlowKind::Jump(target);
    }

    // Far jump: "jmp 0xSEG:0xOFF"
    if let Some(rest) = text.strip_prefix("jmp ")
        && let Some((seg, off)) = parse_seg_off(rest)
    {
        return FlowKind::JumpFar(seg, off);
    }

    // Indirect jmp: "jmp [...]" or "jmp far [...]"
    if text.starts_with("jmp ") {
        return FlowKind::IndirectTransfer;
    }

    // Near call: "call 0xXXXX"
    if let Some(rest) = text.strip_prefix("call 0x")
        && let Some(target) = parse_hex_u16(rest)
    {
        return FlowKind::Call(target);
    }

    // Far call: "call 0xSEG:0xOFF"
    if let Some(rest) = text.strip_prefix("call ")
        && let Some((seg, off)) = parse_seg_off(rest)
    {
        return FlowKind::CallFar(seg, off);
    }

    // Indirect call
    if text.starts_with("call ") {
        return FlowKind::IndirectTransfer;
    }

    // Conditional jumps: j<cc> 0xXXXX
    let cond_mnemonics = [
        "jo ", "jno ", "jb ", "jnb ", "jz ", "jnz ", "jbe ", "ja ", "js ", "jns ", "jp ", "jnp ",
        "jl ", "jge ", "jle ", "jg ", "jcxz ", "loop ", "loope ", "loopne ",
    ];
    for prefix in &cond_mnemonics {
        if let Some(rest) = text.strip_prefix(prefix)
            && let Some(target) = parse_hex_u16(rest)
        {
            return FlowKind::ConditionalJump(target);
        }
    }

    FlowKind::Continue
}

use crate::disasm::Disassembly;
use crate::loader::LoadedImage;

/// Format a sequence of bytes as space-separated hex, padded to `width` chars.
fn fmt_bytes(bytes: &[u8], width: usize) -> String {
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
    let joined = hex.join(" ");
    format!("{joined:<width$}")
}

pub fn print_disassembly(image: &LoadedImage, dis: &Disassembly) {
    let base = dis.image_base;
    let mut addr = base;

    while addr < dis.image_end {
        // Label before this address?
        if dis.labels.contains(&addr) {
            let name = label_name(addr, dis);
            println!("{name}:");
        }

        if let Some(entry) = dis.instructions.get(&addr) {
            // Decoded instruction
            let cs = entry.cs;
            let ip = entry.ip;
            let bytes_str = fmt_bytes(&entry.result.bytes, 20);
            let comment = dis
                .comments
                .get(&addr)
                .map(|c| format!("  ; {c}"))
                .unwrap_or_default();
            println!(
                "    {cs:04X}:{ip:04X}  {bytes_str}  {}{comment}",
                entry.result.text
            );
            let len = entry.result.bytes.len().max(1);
            addr += len;
        } else {
            // Uncovered byte — emit as db
            let image_offset = addr - base;
            let b = image.data.get(image_offset).copied().unwrap_or(0xFF);
            // Express as seg:off using entry_cs as the segment reference
            let seg = image.entry_cs;
            let off = addr.saturating_sub((seg as usize) << 4) as u16;
            let bytes_str = fmt_bytes(&[b], 20);
            let printable = if (0x20..0x7F).contains(&b) {
                format!(" '{}'", b as char)
            } else {
                String::new()
            };
            println!("    {seg:04X}:{off:04X}  {bytes_str}  db 0x{b:02X}    ; {b:3}{printable}");
            addr += 1;
        }
    }
}

fn label_name(addr: usize, dis: &Disassembly) -> String {
    if let Some(name) = dis.custom_labels.get(&addr) {
        return name.clone();
    }
    if let Some(idx) = dis.entry_points.iter().position(|&e| e == addr) {
        if idx == 0 {
            return "entry".to_string();
        } else {
            return format!("entry{idx}");
        }
    }
    let prefix = if dis.call_targets.contains(&addr) {
        "sub"
    } else {
        "loc"
    };
    format!("{prefix}_{addr:05X}")
}

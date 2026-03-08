use crate::disasm::{DataType, Disassembly};
use crate::loader::LoadedImage;

/// Format a sequence of bytes as space-separated hex, padded to `width` chars.
fn fmt_bytes(bytes: &[u8], width: usize) -> String {
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
    let joined = hex.join(" ");
    format!("{joined:<width$}")
}

pub fn print_disassembly(image: &LoadedImage, dis: &Disassembly, data_segment: u16) {
    let base = dis.image_base;
    let data_base = (data_segment as usize) << 4;
    let mut addr = base;

    while addr < dis.image_end {
        // Label before this address?
        if dis.labels.contains(&addr) {
            let name = label_name(addr, dis);
            println!("{name}:");
        }

        if let Some(region) = dis.data_regions.get(&addr) {
            // Data region declared in config — render it as the appropriate type
            // Display using data_segment so the address matches the runtime DS:off.
            let seg = data_segment;
            let off = addr.wrapping_sub(data_base) as u16;
            if let Some(label) = &region.label {
                println!("{label}:");
            }
            match region.data_type {
                DataType::String => {
                    // Collect bytes up to and including the null terminator
                    let mut bytes: Vec<u8> = Vec::new();
                    let mut cursor = addr;
                    loop {
                        let img_off = cursor - base;
                        let b = image.data.get(img_off).copied().unwrap_or(0xFF);
                        cursor += 1;
                        if b == 0 {
                            break;
                        }
                        bytes.push(b);
                        if cursor >= dis.image_end {
                            break;
                        }
                    }
                    let text: String = bytes
                        .iter()
                        .map(|&b| {
                            if b == b'"' {
                                "\\\"".to_string()
                            } else if !(0x20..0x7F).contains(&b) {
                                format!("\\x{b:02X}")
                            } else {
                                (b as char).to_string()
                            }
                        })
                        .collect();
                    let has_null = cursor > addr + bytes.len();
                    let null_suffix = if has_null { ",0" } else { "" };
                    let empty_bytes = fmt_bytes(&[], 20);
                    println!("    {seg:04X}:{off:04X}  {empty_bytes}  db \"{text}\"{null_suffix}");
                    addr = cursor;
                }
            }
        } else if let Some(entry) = dis.instructions.get(&addr) {
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
            // Compute seg:off using data_segment so addresses match the runtime DS.
            let (seg, off) = if addr >= data_base && addr.wrapping_sub(data_base) <= 0xFFFF {
                (data_segment, addr.wrapping_sub(data_base) as u16)
            } else {
                (image.load_segment, (addr - base) as u16)
            };
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

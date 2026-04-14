use std::collections::{HashMap, HashSet};
use std::io::Write;

use anyhow::Result;
use regex::Regex;

use crate::config::{Config, DataEntry, LabelEntry};
use crate::parse::{Key, ParseResult, Patterns};

pub fn wrap_comment(text: &str, width: usize) -> Vec<String> {
    let prefix = "; ";
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            lines.push(prefix.to_string());
            continue;
        }
        let mut current = prefix.to_string();
        for word in words {
            let sep = if current == prefix { "" } else { " " };
            let candidate = format!("{current}{sep}{word}");
            if candidate.len() > width && current != prefix {
                lines.push(current);
                current = format!("{prefix}{word}");
            } else {
                current = candidate;
            }
        }
        lines.push(current);
    }
    lines
}

/// Return the label string for a function/retf target address.
fn func_label_for(
    addr: &str,
    functions: &HashMap<String, LabelEntry>,
    retf_targets: &HashMap<String, LabelEntry>,
) -> String {
    let entry = functions.get(addr).or_else(|| retf_targets.get(addr));
    if let Some(label) = entry.and_then(|e| e.label.as_deref()) {
        return label.to_string();
    }
    let (seg, off) = addr.split_once(':').unwrap();
    format!("func_{seg}_{off}")
}

/// Return the label string for a jump target address (func_ if also a call/retf target, else lbl_).
fn jump_label_for(
    addr: &str,
    call_targets: &HashSet<String>,
    retf_targets: &std::collections::HashMap<String, LabelEntry>,
    functions: &std::collections::HashMap<String, LabelEntry>,
    labels: &std::collections::HashMap<String, LabelEntry>,
) -> String {
    if call_targets.contains(addr) || retf_targets.contains_key(addr) {
        func_label_for(addr, functions, retf_targets)
    } else {
        let (seg, off) = addr.split_once(':').unwrap();
        labels
            .get(addr)
            .and_then(|e| e.label.as_deref())
            .map(String::from)
            .unwrap_or_else(|| format!("lbl_{seg}_{off}"))
    }
}

pub fn sorted_keys(result: &ParseResult) -> Vec<Key> {
    let mut keys: Vec<Key> = result.counts.keys().cloned().collect();
    keys.sort_by(|(a_addr, a_bc), (b_addr, b_bc)| {
        let a_off = u32::from_str_radix(&a_addr[5..], 16).unwrap_or(0);
        let b_off = u32::from_str_radix(&b_addr[5..], 16).unwrap_or(0);
        a_addr[..4]
            .cmp(&b_addr[..4])
            .then(a_off.cmp(&b_off))
            .then(a_bc.cmp(b_bc))
    });
    keys
}

/// Normalise any hex port string to a 4-digit uppercase key used in the ports map.
/// `"21"` → `"0021"`, `"02F3"` → `"02F3"`, `"3da"` → `"03DA"`.
fn normalize_port(hex: &str) -> String {
    format!("{:04X}", u16::from_str_radix(hex, 16).unwrap_or(0))
}

/// Returns a port annotation for `in`/`out` instructions, or `None` for everything else.
///
/// - Immediate port in config → port name
/// - Immediate port not in config → `None` (port already visible in disasm)
/// - DX port, consistent, in config → port name
/// - DX port, consistent, not in config → `"port 0xNNNN"` (port not visible in disasm)
/// - DX port varies → `"port varies"`
fn port_comment(
    disasm: &str,
    val: Option<&Option<String>>,
    ports: &HashMap<String, String>,
    pat: &Patterns,
) -> Option<String> {
    if let Some(cap) = pat.in_imm_re.captures(disasm) {
        let port = normalize_port(&cap[1][2..]);
        return ports.get(&port).cloned();
    }
    if let Some(cap) = pat.out_imm_re.captures(disasm) {
        let port = normalize_port(&cap[1][2..]);
        return ports.get(&port).cloned();
    }
    if pat.in_dx_re.is_match(disasm) || pat.out_dx_re.is_match(disasm) {
        return match val {
            Some(None) => Some("port varies".to_string()),
            Some(Some(annotation)) => {
                pat.dx_val_re.captures(annotation).map(|cap| {
                    let port = normalize_port(&cap[1]);
                    ports
                        .get(&port)
                        .cloned()
                        .unwrap_or_else(|| format!("port 0x{port}"))
                })
            }
            None => None,
        };
    }
    None
}

/// Returns data entries whose start address falls within [gap_start, gap_end) for the given segment,
/// sorted by offset.
fn data_labels_in_range<'a>(
    seg: &str,
    gap_start: u32,
    gap_end: u32,
    data: &'a HashMap<String, DataEntry>,
) -> Vec<(u32, &'a DataEntry)> {
    let mut entries: Vec<(u32, &'a DataEntry)> = data
        .iter()
        .filter_map(|(key, entry)| {
            let (kseg, koff) = key.split_once(':')?;
            if kseg != seg {
                return None;
            }
            let off = u32::from_str_radix(koff, 16).ok()?;
            if off >= gap_start && off < gap_end {
                Some((off, entry))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by_key(|(off, _)| *off);
    entries
}

fn kind_str(entry: &DataEntry) -> String {
    match (entry.kind.as_str(), entry.length) {
        ("bytes", Some(n)) => format!("bytes[{n}]"),
        (k, _) => k.to_string(),
    }
}

pub fn generate<W: Write>(
    out: &mut W,
    result: &ParseResult,
    config: &Config,
    hot_threshold: u64,
) -> Result<()> {
    let pat = Patterns::new();
    // Matches a direct memory operand like [0x0082] or es:[0x0082]
    let mem_ref_re = Regex::new(r"\[0x([0-9a-fA-F]{1,4})\]").unwrap();
    // Matches any 0xNNNN immediate (including those inside brackets; filter below)
    let imm_re = Regex::new(r"0x([0-9a-fA-F]{1,4})").unwrap();

    let mut prev_seg: Option<String> = None;
    let mut prev_end_off: Option<u32> = None;

    for key in &sorted_keys(result) {
        let (addr, bytecode) = key;
        let disasm = &result.info[key];
        let count = result.counts[key];

        let seg = &addr[..4];
        let off_str = &addr[5..];
        let cur_off = u32::from_str_radix(off_str, 16).unwrap_or(0);
        let byte_len = bytecode.split_whitespace().count() as u32;

        // Gap detection within the same segment.
        // Gaps are split at data label boundaries so each data region gets its own label.
        if prev_seg.as_deref() == Some(seg)
            && let Some(end) = prev_end_off
            && cur_off > end
        {
            let mut gap_pos = end;
            for (data_off, entry) in data_labels_in_range(seg, end, cur_off, &config.data) {
                if data_off > gap_pos {
                    let gap_key = format!("{seg}:{gap_pos:04X}");
                    let annotation = config.gaps.get(&gap_key).map(|s| format!(" {s}")).unwrap_or_default();
                    writeln!(out, "   ; gap {seg}:{gap_pos:04X} - {seg}:{data_off:04X} ({} bytes){annotation}", data_off - gap_pos)?;
                }
                writeln!(out)?;
                if let Some(comment) = &entry.comment {
                    for line in wrap_comment(comment, 80) {
                        writeln!(out, "{line}")?;
                    }
                }
                writeln!(out, "{}:   ; {seg}:{data_off:04X}  {}", entry.label, kind_str(entry))?;
                gap_pos = data_off;
            }
            if gap_pos < cur_off {
                let gap_key = format!("{seg}:{gap_pos:04X}");
                let annotation = config.gaps.get(&gap_key).map(|s| format!(" {s}")).unwrap_or_default();
                writeln!(out, "   ; gap {seg}:{gap_pos:04X} - {seg}:{cur_off:04X} ({} bytes){annotation}", cur_off - gap_pos)?;
            }
        }
        if prev_seg.as_deref() != Some(seg) {
            prev_seg = Some(seg.to_string());
            prev_end_off = Some(cur_off + byte_len);
        } else {
            prev_end_off = Some(prev_end_off.unwrap_or(0).max(cur_off + byte_len));
        }

        // Interrupt handler labels
        if let Some(ints) = result.int_handlers.get(addr.as_str()) {
            for n in ints {
                writeln!(out, "\nint_{n:02x}h:")?;
            }
        }

        // Function / jump target labels
        if result.call_targets.contains(addr.as_str())
            || config.retf_targets.contains_key(addr.as_str())
        {
            let entry = config
                .functions
                .get(addr.as_str())
                .or_else(|| config.retf_targets.get(addr.as_str()));
            writeln!(out)?;
            if let Some(comment) = entry.and_then(|e| e.comment.as_deref()) {
                for line in wrap_comment(comment, 80) {
                    writeln!(out, "{line}")?;
                }
            }
            if let Some(label) = entry.and_then(|e| e.label.as_deref()) {
                writeln!(out, "{label}:   ; {addr}")?;
            } else {
                writeln!(out, "func_{seg}_{off_str}:")?;
            }
        } else if result.jump_targets.contains(addr.as_str()) {
            let entry = config.labels.get(addr.as_str());
            if let Some(comment) = entry.and_then(|e| e.comment.as_deref()) {
                writeln!(out)?;
                for line in wrap_comment(comment, 80) {
                    writeln!(out, "{line}")?;
                }
            }
            if let Some(label) = entry.and_then(|e| e.label.as_deref()) {
                writeln!(out, "{label}:   ; {addr}")?;
            } else {
                writeln!(out, "lbl_{seg}_{off_str}:")?;
            }
        }

        // Inline call/jump label annotation for the instruction comment
        let call_label: Option<String> = if let Some(cap) = pat.call_near_re.captures(disasm) {
            let off = u32::from_str_radix(&cap[1][2..], 16).unwrap_or(0);
            let target = format!("{seg}:{off:04X}");
            Some(
                config
                    .functions
                    .get(&target)
                    .and_then(|e| e.label.as_deref())
                    .map(String::from)
                    .unwrap_or_else(|| format!("func_{seg}_{off:04X}")),
            )
        } else if let Some(cap) = pat.call_far_re.captures(disasm) {
            let tseg = u32::from_str_radix(&cap[1][2..], 16).unwrap_or(0);
            let toff = u32::from_str_radix(&cap[2][2..], 16).unwrap_or(0);
            let target = format!("{tseg:04X}:{toff:04X}");
            Some(
                config
                    .functions
                    .get(&target)
                    .and_then(|e| e.label.as_deref())
                    .map(String::from)
                    .unwrap_or_else(|| format!("func_{tseg:04X}_{toff:04X}")),
            )
        } else if let Some(cap) = pat.jmp_near_re.captures(disasm) {
            let joff = u32::from_str_radix(&cap[2][2..], 16).unwrap_or(0);
            let jtarget = format!("{seg}:{joff:04X}");
            Some(jump_label_for(
                &jtarget,
                &result.call_targets,
                &config.retf_targets,
                &config.functions,
                &config.labels,
            ))
        } else if let Some(cap) = pat.jmp_far_re.captures(disasm) {
            let jtseg = u32::from_str_radix(&cap[1][2..], 16).unwrap_or(0);
            let jtoff = u32::from_str_radix(&cap[2][2..], 16).unwrap_or(0);
            let jtarget = format!("{jtseg:04X}:{jtoff:04X}");
            Some(jump_label_for(
                &jtarget,
                &result.call_targets,
                &config.retf_targets,
                &config.functions,
                &config.labels,
            ))
        } else {
            None
        };

        // Config line comment printed before the instruction
        if let Some(lc) = config.line_comments.get(addr.as_str())
            && !lc.is_empty()
        {
            for cline in wrap_comment(lc, 80) {
                writeln!(out, "   {cline}")?;
            }
        }

        // Look up direct memory references ([0xNNNN]) in memLabels using the
        // instruction's segment as the address qualifier.
        let mem_label = mem_ref_re.captures(disasm).and_then(|cap| {
            let off = u32::from_str_radix(&cap[1], 16).ok()?;
            let key = format!("{seg}:{off:04X}");
            config
                .mem_labels
                .get(&key)
                .filter(|s| !s.is_empty())
                .cloned()
        });

        let port_label = port_comment(disasm, result.values.get(key), &config.ports, &pat);

        // Look up data labels from plain immediates (e.g. `mov dx, 0x2962`).
        // Skip bracketed references — those are handled by memLabels.
        let data_label = imm_re
            .captures_iter(disasm)
            .filter(|cap| {
                let start = cap.get(0).unwrap().start();
                start == 0 || disasm.as_bytes()[start - 1] != b'['
            })
            .find_map(|cap| {
                let off = u32::from_str_radix(&cap[1], 16).ok()?;
                config
                    .data
                    .get(&format!("{seg}:{off:04X}"))
                    .map(|e| e.label.clone())
            });

        let annotations: Vec<&str> = [call_label.as_deref(), mem_label.as_deref(), port_label.as_deref(), data_label.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        let comment_col = if annotations.is_empty() {
            String::new()
        } else {
            format!("{}  ", annotations.join(" "))
        };
        let val_col = match result.values.get(key) {
            Some(Some(v)) if !v.is_empty() => format!("  [{v}]"),
            _ => String::new(),
        };

        let hot = if count >= hot_threshold { " [HOT]" } else { "" };
        writeln!(
            out,
            "   {disasm:<24}; {count:4}{hot} -- {addr} {bytecode:<19}{comment_col} {val_col}"
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};

    use super::*;
    use crate::config::{Config, DataEntry, LabelEntry};
    use crate::parse::ParseResult;

    /// Build a `ParseResult` from a slice of `(addr, bytes, disasm, count, val)` tuples.
    /// `val = Some("AX=0001")` → consistent value; `val = None` → value varies.
    fn make_result(
        instructions: &[(&str, &str, &str, u64, Option<&str>)],
        call_targets: Vec<&str>,
        jump_targets: Vec<&str>,
        int_handlers: Vec<(&str, u32)>,
    ) -> ParseResult {
        let mut counts: HashMap<Key, u64> = HashMap::new();
        let mut info: HashMap<Key, String> = HashMap::new();
        let mut values: HashMap<Key, Option<String>> = HashMap::new();
        for &(addr, bytes, disasm, count, val) in instructions {
            let key: Key = (addr.to_string(), bytes.to_string());
            counts.insert(key.clone(), count);
            info.insert(key.clone(), disasm.to_string());
            values.insert(key, val.map(str::to_string));
        }
        let mut ih: HashMap<String, BTreeSet<u32>> = HashMap::new();
        for (addr, n) in int_handlers {
            ih.entry(addr.to_string()).or_default().insert(n);
        }
        ParseResult {
            counts,
            info,
            values,
            call_targets: call_targets.into_iter().map(String::from).collect(),
            jump_targets: jump_targets.into_iter().map(String::from).collect(),
            int_handlers: ih,
        }
    }

    fn run(result: &ParseResult, config: &Config, threshold: u64) -> String {
        let mut buf = Vec::new();
        generate(&mut buf, result, config, threshold).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn test_basic_format() {
        let result = make_result(
            &[("0019:423F", "55", "push bp", 3, None)],
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("push bp"), "disasm");
        assert!(out.contains("0019:423F"), "addr");
        assert!(out.contains("55"), "bytecode");
        assert!(out.contains("3"), "count");
        assert!(!out.contains("[HOT]"), "should not be HOT");
    }

    #[test]
    fn test_hot_at_threshold() {
        let result = make_result(
            &[("0019:0000", "90", "nop", 1000, None)],
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("[HOT]"));
    }

    #[test]
    fn test_not_hot_below_threshold() {
        let result = make_result(
            &[("0019:0000", "90", "nop", 999, None)],
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(!out.contains("[HOT]"));
    }

    #[test]
    fn test_gap_detected() {
        let result = make_result(
            &[
                ("0019:0000", "55", "push bp", 1, None), // 1 byte → ends at 0001
                ("0019:0005", "5D", "pop bp", 1, None),  // gap: 0001..0005 = 4 bytes
            ],
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("; gap 0019:0001 - 0019:0005 (4 bytes)"));
    }

    #[test]
    fn test_no_gap_when_consecutive() {
        let result = make_result(
            &[
                ("0019:0000", "55", "push bp", 1, None), // 1 byte → ends at 0001
                ("0019:0001", "5D", "pop bp", 1, None),  // consecutive
            ],
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(!out.contains("; gap"));
    }

    #[test]
    fn test_gap_with_annotation() {
        let result = make_result(
            &[
                ("0019:0000", "55", "push bp", 1, None),
                ("0019:0005", "5D", "pop bp", 1, None),
            ],
            vec![],
            vec![],
            vec![],
        );
        let mut config = Config::default();
        config
            .gaps
            .insert("0019:0001".to_string(), "unused error path".to_string());
        let out = run(&result, &config, 1000);
        assert!(out.contains("unused error path"));
    }

    #[test]
    fn test_call_target_auto_label() {
        let result = make_result(
            &[("0019:0010", "55", "push bp", 1, None)],
            vec!["0019:0010"],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("func_0019_0010:"));
    }

    #[test]
    fn test_custom_function_label() {
        let result = make_result(
            &[("0019:0010", "55", "push bp", 1, None)],
            vec!["0019:0010"],
            vec![],
            vec![],
        );
        let mut config = Config::default();
        config.functions.insert(
            "0019:0010".to_string(),
            LabelEntry {
                label: Some("my_func".to_string()),
                comment: None,
            },
        );
        let out = run(&result, &config, 1000);
        assert!(out.contains("my_func:   ; 0019:0010"));
        assert!(!out.contains("func_0019_0010:"));
    }

    #[test]
    fn test_function_comment_block() {
        let result = make_result(
            &[("0019:0010", "55", "push bp", 1, None)],
            vec!["0019:0010"],
            vec![],
            vec![],
        );
        let mut config = Config::default();
        config.functions.insert(
            "0019:0010".to_string(),
            LabelEntry {
                label: Some("my_func".to_string()),
                comment: Some("Does something useful".to_string()),
            },
        );
        let out = run(&result, &config, 1000);
        assert!(out.contains("; Does something useful"));
    }

    #[test]
    fn test_jump_target_auto_label() {
        let result = make_result(
            &[("0019:0020", "90", "nop", 1, None)],
            vec![],
            vec!["0019:0020"],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("lbl_0019_0020:"));
    }

    #[test]
    fn test_jump_target_that_is_also_call_target_uses_func_label() {
        let result = make_result(
            &[("0019:0020", "90", "nop", 1, None)],
            vec!["0019:0020"],
            vec!["0019:0020"],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("func_0019_0020:"));
        assert!(!out.contains("lbl_0019_0020:"));
    }

    #[test]
    fn test_int_handler_label() {
        let result = make_result(
            &[("0070:1234", "50", "push ax", 1, None)],
            vec![],
            vec![],
            vec![("0070:1234", 0x21)],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("int_21h:"));
    }

    #[test]
    fn test_call_near_inline_annotation() {
        let result = make_result(
            &[
                ("0019:0000", "E8 0E 00", "call 0x0010", 1, None),
                ("0019:0010", "55", "push bp", 1, None),
            ],
            vec!["0019:0010"],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        let call_line = out.lines().find(|l| l.contains("call 0x0010")).unwrap();
        assert!(
            call_line.contains("func_0019_0010"),
            "inline label missing: {call_line}"
        );
    }

    #[test]
    fn test_jmp_near_inline_annotation() {
        let result = make_result(
            &[
                ("0019:0000", "EB 0E", "jmp 0x0010", 1, None),
                ("0019:0010", "90", "nop", 1, None),
            ],
            vec![],
            vec!["0019:0010"],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        let jmp_line = out.lines().find(|l| l.contains("jmp 0x0010")).unwrap();
        assert!(
            jmp_line.contains("lbl_0019_0010"),
            "inline label missing: {jmp_line}"
        );
    }

    #[test]
    fn test_line_comment() {
        let result = make_result(
            &[("0019:0042", "3C 6C", "cmp al, 0x6c", 1, None)],
            vec![],
            vec![],
            vec![],
        );
        let mut config = Config::default();
        config
            .line_comments
            .insert("0019:0042".to_string(), "compare end-of-line".to_string());
        let out = run(&result, &config, 1000);
        assert!(out.contains("; compare end-of-line"));
        let comment_pos = out.find("; compare end-of-line").unwrap();
        let instr_pos = out.find("cmp al, 0x6c").unwrap();
        assert!(
            comment_pos < instr_pos,
            "comment should precede instruction"
        );
    }

    #[test]
    fn test_mem_label() {
        let result = make_result(
            &[("0019:0042", "A0 82 00", "mov al, [0x0082]", 1, None)],
            vec![],
            vec![],
            vec![],
        );
        let mut config = Config::default();
        config
            .mem_labels
            .insert("0019:0082".to_string(), "cmd_code".to_string());
        let out = run(&result, &config, 1000);
        let line = out.lines().find(|l| l.contains("mov al")).unwrap();
        assert!(line.contains("cmd_code"), "mem label missing: {line}");
    }

    #[test]
    fn test_value_column_shown() {
        let result = make_result(
            &[("0019:0000", "90", "nop", 1, Some("AX=0001"))],
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        assert!(out.contains("[AX=0001]"));
    }

    #[test]
    fn test_value_varies_not_shown() {
        let result = make_result(
            &[("0019:0000", "90", "nop", 2, None)], // None = varies
            vec![],
            vec![],
            vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        let line = out.lines().find(|l| l.contains("nop")).unwrap();
        assert!(!line.contains('['), "no value column expected: {line}");
    }

    // --- port_comment tests ---

    fn make_patterns() -> Patterns {
        Patterns::new()
    }

    fn ports(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn test_port_in_immediate_named() {
        let pat = make_patterns();
        let result = port_comment("in al, 0x21", Some(&Some("AX=0000".to_string())), &ports(&[("0021", "keyboard")]), &pat);
        assert_eq!(result.as_deref(), Some("keyboard"));
    }

    #[test]
    fn test_port_in_immediate_unnamed() {
        let pat = make_patterns();
        let result = port_comment("in al, 0x21", Some(&Some("AX=0000".to_string())), &ports(&[]), &pat);
        assert_eq!(result, None);
    }

    #[test]
    fn test_port_out_immediate_named() {
        let pat = make_patterns();
        let result = port_comment("out 0x43, al", Some(&Some("AX=0036".to_string())), &ports(&[("0043", "PIT cmd")]), &pat);
        assert_eq!(result.as_deref(), Some("PIT cmd"));
    }

    #[test]
    fn test_port_out_immediate_unnamed() {
        let pat = make_patterns();
        let result = port_comment("out 0x43, al", Some(&Some("AX=0036".to_string())), &ports(&[]), &pat);
        assert_eq!(result, None);
    }

    #[test]
    fn test_port_in_dx_named() {
        let pat = make_patterns();
        let annotation = Some("DX=02F3 AX=0000".to_string());
        let result = port_comment("in al, dx", Some(&annotation), &ports(&[("02F3", "SB-CD data")]), &pat);
        assert_eq!(result.as_deref(), Some("SB-CD data"));
    }

    #[test]
    fn test_port_in_dx_unnamed_shows_number() {
        let pat = make_patterns();
        let annotation = Some("DX=02F3 AX=0000".to_string());
        let result = port_comment("in al, dx", Some(&annotation), &ports(&[]), &pat);
        assert_eq!(result.as_deref(), Some("port 0x02F3"));
    }

    #[test]
    fn test_port_out_dx_named() {
        let pat = make_patterns();
        let annotation = Some("DX=0230 AX=0042".to_string());
        let result = port_comment("out dx, al", Some(&annotation), &ports(&[("0230", "SB-CD cmd")]), &pat);
        assert_eq!(result.as_deref(), Some("SB-CD cmd"));
    }

    #[test]
    fn test_port_in_dx_varies() {
        let pat = make_patterns();
        let result = port_comment("in al, dx", Some(&None), &ports(&[("02F3", "SB-CD data")]), &pat);
        assert_eq!(result.as_deref(), Some("port varies"));
    }

    #[test]
    fn test_port_out_dx_varies() {
        let pat = make_patterns();
        let result = port_comment("out dx, al", Some(&None), &ports(&[]), &pat);
        assert_eq!(result.as_deref(), Some("port varies"));
    }

    #[test]
    fn test_builtin_port_no_config() {
        // PIC1 command (0x20) should be annotated with no user config at all
        let pat = make_patterns();
        let result = port_comment(
            "out 0x20, al",
            Some(&Some("AX=0020".to_string())),
            &Config::default().ports,
            &pat,
        );
        assert_eq!(result.as_deref(), Some("PIC1 command"));
    }

    #[test]
    fn test_user_port_overrides_builtin() {
        let pat = make_patterns();
        // 0x20 is built-in as "PIC1 command"; user renames it
        let mut p = Config::default().ports;
        p.insert("0020".to_string(), "my PIC".to_string());
        let result = port_comment("out 0x20, al", Some(&Some("AX=0020".to_string())), &p, &pat);
        assert_eq!(result.as_deref(), Some("my PIC"));
    }

    #[test]
    fn test_port_non_io_instruction() {
        let pat = make_patterns();
        let result = port_comment("mov ax, bx", Some(&Some("AX=0001".to_string())), &ports(&[]), &pat);
        assert_eq!(result, None);
    }

    #[test]
    fn test_port_in_generate_output() {
        let result = make_result(
            &[("0C45:2183", "EC", "in al, dx", 7, Some("DX=0230 AX=00FF"))],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        config.ports.insert("0230".to_string(), "SB-CD cmd".to_string());
        let out = run(&result, &config, 1000);
        let line = out.lines().find(|l| l.contains("in al, dx")).unwrap();
        assert!(line.contains("SB-CD cmd"), "port name missing: {line}");
    }

    #[test]
    fn test_port_varies_in_generate_output() {
        let result = make_result(
            &[("0C45:229F", "EC", "in al, dx", 2, None)], // None = port varies
            vec![], vec![], vec![],
        );
        let out = run(&result, &Config::default(), 1000);
        let line = out.lines().find(|l| l.contains("in al, dx")).unwrap();
        assert!(line.contains("port varies"), "port varies missing: {line}");
    }

    // --- data section annotation tests ---

    fn make_data(entries: &[(&str, &str, &str, Option<u32>, Option<&str>)]) -> HashMap<String, DataEntry> {
        // (addr, label, kind, length, comment)
        entries.iter().map(|(addr, label, kind, length, comment)| {
            (addr.to_string(), DataEntry {
                label: label.to_string(),
                comment: comment.map(str::to_string),
                kind: kind.to_string(),
                length: *length,
            })
        }).collect()
    }

    #[test]
    fn test_data_label_in_gap() {
        // Two instructions with a gap; data entry at the gap start
        let result = make_result(
            &[
                ("0C45:0000", "55", "push bp", 1, None),   // ends at 0001
                ("0C45:000A", "5D", "pop bp", 1, None),    // gap 0001..000A
            ],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        config.data = make_data(&[("0C45:0001", "my_data", "bytes", Some(4), None)]);
        let out = run(&result, &config, 1000);
        assert!(out.contains("my_data:   ; 0C45:0001  bytes[4]"), "label missing:\n{out}");
    }

    #[test]
    fn test_data_label_splits_gap() {
        // Gap from 0001 to 000A; data at 0005 — should produce two gap lines
        let result = make_result(
            &[
                ("0C45:0000", "55", "push bp", 1, None),
                ("0C45:000A", "5D", "pop bp", 1, None),
            ],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        config.data = make_data(&[("0C45:0005", "mid_data", "string", None, None)]);
        let out = run(&result, &config, 1000);
        // First sub-gap: 0001..0005
        assert!(out.contains("; gap 0C45:0001 - 0C45:0005 (4 bytes)"), "first sub-gap:\n{out}");
        // Data label
        assert!(out.contains("mid_data:   ; 0C45:0005  string"), "label:\n{out}");
        // Second sub-gap: 0005..000A
        assert!(out.contains("; gap 0C45:0005 - 0C45:000A (5 bytes)"), "second sub-gap:\n{out}");
    }

    #[test]
    fn test_data_label_with_comment() {
        let result = make_result(
            &[
                ("0019:0000", "55", "push bp", 1, None),
                ("0019:0010", "5D", "pop bp", 1, None),
            ],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        config.data = make_data(&[("0019:0001", "oem_id", "bytes", Some(8), Some("OEM identifier string"))]);
        let out = run(&result, &config, 1000);
        assert!(out.contains("; OEM identifier string"), "comment:\n{out}");
        // Comment should appear before label
        let comment_pos = out.find("; OEM identifier string").unwrap();
        let label_pos = out.find("oem_id:").unwrap();
        assert!(comment_pos < label_pos);
    }

    #[test]
    fn test_data_imm_annotation() {
        // Instruction with an immediate that matches a data label address
        let result = make_result(
            &[("0019:0000", "BA 10 00", "mov dx, 0x0010", 1, None)],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        config.data = make_data(&[("0019:0010", "str_hello", "string", None, None)]);
        let out = run(&result, &config, 1000);
        let line = out.lines().find(|l| l.contains("mov dx")).unwrap();
        assert!(line.contains("str_hello"), "imm annotation missing: {line}");
    }

    #[test]
    fn test_data_imm_no_annotation_for_bracket_ref() {
        // [0x0010] is a memory reference (memLabels territory), not a data imm
        let result = make_result(
            &[("0019:0000", "8B 16 10 00", "mov dx, [0x0010]", 1, None)],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        config.data = make_data(&[("0019:0010", "str_hello", "string", None, None)]);
        let out = run(&result, &config, 1000);
        let line = out.lines().find(|l| l.contains("mov dx")).unwrap();
        // data_label should NOT fire for bracket refs (memLabels handles those)
        assert!(!line.contains("str_hello"), "should not annotate bracket ref: {line}");
    }

    #[test]
    fn test_data_label_not_in_gap_of_different_segment() {
        let result = make_result(
            &[
                ("0019:0000", "55", "push bp", 1, None),
                ("0019:000A", "5D", "pop bp", 1, None),
            ],
            vec![], vec![], vec![],
        );
        let mut config = Config::default();
        // Data entry is in a different segment — should not appear
        config.data = make_data(&[("001A:0005", "other_seg_data", "bytes", None, None)]);
        let out = run(&result, &config, 1000);
        assert!(!out.contains("other_seg_data"));
    }
}

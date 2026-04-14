use std::collections::{HashMap, HashSet};
use std::io::Write;

use anyhow::Result;
use regex::Regex;

use crate::config::{Config, LabelEntry};
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

pub fn generate<W: Write>(
    out: &mut W,
    result: &ParseResult,
    config: &Config,
    hot_threshold: u64,
) -> Result<()> {
    let pat = Patterns::new();
    // Matches a direct memory operand like [0x0082] or es:[0x0082]
    let mem_ref_re = Regex::new(r"\[0x([0-9a-fA-F]{1,4})\]").unwrap();

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

        // Gap detection within the same segment
        if prev_seg.as_deref() == Some(seg)
            && let Some(end) = prev_end_off
            && cur_off > end
        {
            let gap_key = format!("{seg}:{end:04X}");
            let annotation = config
                .gaps
                .get(&gap_key)
                .map(|s| format!(" {s}"))
                .unwrap_or_default();
            writeln!(
                out,
                "   ; gap {seg}:{end:04X} - {seg}:{cur_off:04X} ({} bytes){annotation}",
                cur_off - end
            )?;
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

        let comment_col = match (call_label.as_deref(), mem_label.as_deref()) {
            (Some(c), Some(m)) => format!("{c} {m}  "),
            (Some(c), None) => format!("{c}  "),
            (None, Some(m)) => format!("{m}  "),
            (None, None) => String::new(),
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
    use crate::config::{Config, LabelEntry};
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
}

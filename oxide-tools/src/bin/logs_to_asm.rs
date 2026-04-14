use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use serde_json::Value;

#[derive(Parser)]
#[command(
    name = "logs-to-asm",
    about = "Convert oxide86 execution log to annotated assembly listing"
)]
struct Args {
    /// Path to the log file
    #[arg(long, short, default_value = "oxide86.log")]
    log_file: PathBuf,

    /// Path to optional JSON config file
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Output file path
    #[arg(long, short)]
    out: PathBuf,
}

/// A named label with optional comment block, from the config file.
#[derive(Default)]
struct LabelEntry {
    label: Option<String>,
    comment: Option<String>,
}

#[derive(Default)]
struct Config {
    functions: HashMap<String, LabelEntry>,
    labels: HashMap<String, LabelEntry>,
    line_comments: HashMap<String, String>, // addr -> comment text
    retf_targets: HashMap<String, LabelEntry>,
}

/// (addr "SSSS:OOOO", bytecode "BB BB ...")
type Key = (String, String);

struct ParseResult {
    counts: HashMap<Key, u64>,
    info: HashMap<Key, String>,           // key -> disasm string
    values: HashMap<Key, Option<String>>, // None = varies across executions
    call_targets: HashSet<String>,
    jump_targets: HashSet<String>,
    int_handlers: HashMap<String, BTreeSet<u32>>, // addr -> set of int numbers
}

struct Patterns {
    log_re: Regex,
    call_near_re: Regex,
    call_far_re: Regex,
    int_re: Regex,
    jmp_near_re: Regex,
    jmp_far_re: Regex,
}

impl Patterns {
    fn new() -> Self {
        Self {
            log_re: Regex::new(
                r"\] ([0-9A-Fa-f]{4}:[0-9A-Fa-f]{4}) ((?:[0-9A-Fa-f]{2} )*[0-9A-Fa-f]{2})\s+(.*?)(?:\s{2,}(.*))?$"
            ).unwrap(),
            call_near_re: Regex::new(r"^call\s+(0x[0-9a-fA-F]+)$").unwrap(),
            call_far_re: Regex::new(
                r"^call\s+far\s+(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)$"
            ).unwrap(),
            int_re: Regex::new(r"^int\s+(0x[0-9a-fA-F]+)$").unwrap(),
            jmp_near_re: Regex::new(
                r"^(jmp(?:\s+(?:short|near))?|j[a-z]+|loop[a-z]*|jcxz)\s+(0x[0-9a-fA-F]+)$"
            ).unwrap(),
            jmp_far_re: Regex::new(
                r"^jmp\s+far\s+(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)$"
            ).unwrap(),
        }
    }
}

fn parse_log(path: &PathBuf) -> Result<ParseResult> {
    let pat = Patterns::new();

    let mut counts: HashMap<Key, u64> = HashMap::new();
    let mut info: HashMap<Key, String> = HashMap::new();
    let mut values: HashMap<Key, Option<String>> = HashMap::new();
    let mut call_targets: HashSet<String> = HashSet::new();
    let mut jump_targets: HashSet<String> = HashSet::new();
    let mut int_handlers: HashMap<String, BTreeSet<u32>> = HashMap::new();
    let mut pending_int: Option<(u32, String)> = None; // (int_num, caller_seg)

    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        let caps = match pat.log_re.captures(&line) {
            Some(c) => c,
            None => continue,
        };

        let addr = caps[1].to_uppercase();
        let bytecode = caps[2].trim_end().to_string();
        let disasm = caps[3].trim().to_string();
        let val = caps
            .get(4)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let key: Key = (addr.clone(), bytecode.clone());

        *counts.entry(key.clone()).or_insert(0) += 1;
        info.entry(key.clone()).or_insert_with(|| disasm.clone());

        // Track whether the register-annotation value is consistent across runs.
        match values.get(&key) {
            None => {
                values.insert(key.clone(), Some(val.clone()));
            }
            Some(Some(existing)) if existing != &val => {
                values.insert(key.clone(), None); // varies
            }
            _ => {}
        }

        // If the previous instruction was `int NN`, the next instruction logged
        // from a different segment is the handler entry point.
        let old_pending = pending_int.take();
        if let Some((int_num, int_seg)) = old_pending {
            if &addr[..4] != int_seg.as_str() {
                int_handlers.entry(addr.clone()).or_default().insert(int_num);
            }
        }

        let seg = addr[..4].to_string();

        if let Some(cap) = pat.call_near_re.captures(&disasm) {
            let off = u32::from_str_radix(&cap[1][2..], 16)?;
            call_targets.insert(format!("{seg}:{off:04X}"));
            continue;
        }
        if let Some(cap) = pat.call_far_re.captures(&disasm) {
            let tseg = u32::from_str_radix(&cap[1][2..], 16)?;
            let toff = u32::from_str_radix(&cap[2][2..], 16)?;
            call_targets.insert(format!("{tseg:04X}:{toff:04X}"));
            continue;
        }
        if let Some(cap) = pat.int_re.captures(&disasm) {
            let int_num = u32::from_str_radix(&cap[1][2..], 16)?;
            pending_int = Some((int_num, seg));
            continue;
        }
        if let Some(cap) = pat.jmp_near_re.captures(&disasm) {
            let off = u32::from_str_radix(&cap[2][2..], 16)?;
            jump_targets.insert(format!("{seg}:{off:04X}"));
            continue;
        }
        if let Some(cap) = pat.jmp_far_re.captures(&disasm) {
            let tseg = u32::from_str_radix(&cap[1][2..], 16)?;
            let toff = u32::from_str_radix(&cap[2][2..], 16)?;
            jump_targets.insert(format!("{tseg:04X}:{toff:04X}"));
        }
    }

    Ok(ParseResult {
        counts,
        info,
        values,
        call_targets,
        jump_targets,
        int_handlers,
    })
}

fn parse_label_entry(v: &Value) -> LabelEntry {
    LabelEntry {
        label: v.get("label").and_then(Value::as_str).map(String::from),
        comment: v.get("comment").and_then(Value::as_str).map(String::from),
    }
}

fn load_config(path: &PathBuf) -> Result<Config> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => {
            return Err(e).with_context(|| format!("opening config {}", path.display()));
        }
    };
    let data: Value = serde_json::from_reader(file)
        .with_context(|| format!("parsing config {}", path.display()))?;

    let parse_entry_map = |key: &str| -> HashMap<String, LabelEntry> {
        data.get(key)
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.to_uppercase(), parse_label_entry(v)))
                    .collect()
            })
            .unwrap_or_default()
    };

    let line_comments = data
        .get("lineComments")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.to_uppercase(), v.as_str().unwrap_or("").to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Config {
        functions: parse_entry_map("functions"),
        labels: parse_entry_map("labels"),
        line_comments,
        retf_targets: parse_entry_map("retf_targets"),
    })
}

fn wrap_comment(text: &str, width: usize) -> Vec<String> {
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
    retf_targets: &HashMap<String, LabelEntry>,
    functions: &HashMap<String, LabelEntry>,
    labels: &HashMap<String, LabelEntry>,
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

fn main() -> Result<()> {
    let args = Args::parse();

    let ParseResult {
        counts,
        info,
        values,
        call_targets,
        jump_targets,
        int_handlers,
    } = parse_log(&args.log_file)?;

    let Config {
        functions,
        labels,
        line_comments,
        retf_targets,
    } = match &args.config {
        Some(p) => load_config(p)?,
        None => Config::default(),
    };

    // Sort by (seg_str, off_u32, bytecode_str) — same order as the Python script.
    let mut keys: Vec<Key> = counts.keys().cloned().collect();
    keys.sort_by(|(a_addr, a_bc), (b_addr, b_bc)| {
        let a_off = u32::from_str_radix(&a_addr[5..], 16).unwrap_or(0);
        let b_off = u32::from_str_radix(&b_addr[5..], 16).unwrap_or(0);
        a_addr[..4]
            .cmp(&b_addr[..4])
            .then(a_off.cmp(&b_off))
            .then(a_bc.cmp(b_bc))
    });

    let mut out = BufWriter::new(
        File::create(&args.out)
            .with_context(|| format!("creating output file {}", args.out.display()))?,
    );

    writeln!(out, "; Generated by oxide86-tools logs-to-asm")?;
    writeln!(out, "; Additional information can be found in scripts/logs_to_asm.md")?;
    writeln!(out, "; Log: {}", args.log_file.display())?;
    if let Some(cfg) = &args.config {
        writeln!(out, "; Config: {}", cfg.display())?;
    }
    writeln!(out)?;

    let pat = Patterns::new();
    let mut prev_seg: Option<String> = None;
    let mut prev_end_off: Option<u32> = None;

    for key in &keys {
        let (addr, bytecode) = key;
        let disasm = &info[key];
        let count = counts[key];

        let seg = &addr[..4];
        let off_str = &addr[5..];
        let cur_off = u32::from_str_radix(off_str, 16).unwrap_or(0);
        let byte_len = bytecode.split_whitespace().count() as u32;

        // Gap detection within the same segment
        if prev_seg.as_deref() == Some(seg) {
            if let Some(end) = prev_end_off {
                if cur_off > end {
                    writeln!(
                        out,
                        "   ; gap {seg}:{end:04X} - {seg}:{cur_off:04X} ({} bytes)",
                        cur_off - end
                    )?;
                }
            }
        }
        if prev_seg.as_deref() != Some(seg) {
            prev_seg = Some(seg.to_string());
            prev_end_off = Some(cur_off + byte_len);
        } else {
            prev_end_off = Some(prev_end_off.unwrap_or(0).max(cur_off + byte_len));
        }

        // Interrupt handler labels
        if let Some(ints) = int_handlers.get(addr.as_str()) {
            for n in ints {
                writeln!(out, "\nint_{n:02x}h:")?;
            }
        }

        // Function / jump target labels
        if call_targets.contains(addr.as_str()) || retf_targets.contains_key(addr.as_str()) {
            let entry = functions
                .get(addr.as_str())
                .or_else(|| retf_targets.get(addr.as_str()));
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
        } else if jump_targets.contains(addr.as_str()) {
            let entry = labels.get(addr.as_str());
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
                functions
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
                functions
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
                &call_targets,
                &retf_targets,
                &functions,
                &labels,
            ))
        } else if let Some(cap) = pat.jmp_far_re.captures(disasm) {
            let jtseg = u32::from_str_radix(&cap[1][2..], 16).unwrap_or(0);
            let jtoff = u32::from_str_radix(&cap[2][2..], 16).unwrap_or(0);
            let jtarget = format!("{jtseg:04X}:{jtoff:04X}");
            Some(jump_label_for(
                &jtarget,
                &call_targets,
                &retf_targets,
                &functions,
                &labels,
            ))
        } else {
            None
        };

        // Config line comment printed before the instruction
        if let Some(lc) = line_comments.get(addr.as_str()) {
            if !lc.is_empty() {
                for cline in wrap_comment(lc, 80) {
                    writeln!(out, "   {cline}")?;
                }
            }
        }

        let comment_col = call_label
            .as_deref()
            .map(|l| format!("{l}  "))
            .unwrap_or_default();
        let val_col = match values.get(key) {
            Some(Some(v)) if !v.is_empty() => format!("  [{v}]"),
            _ => String::new(),
        };

        writeln!(
            out,
            "   {disasm:<24}; {count:4} -- {addr} {bytecode:<19}{comment_col} {val_col}"
        )?;
    }

    Ok(())
}

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::{Context, Result};
use regex::Regex;

/// (addr "SSSS:OOOO", bytecode "BB BB ...")
pub type Key = (String, String);

pub struct ParseResult {
    pub counts: HashMap<Key, u64>,
    pub info: HashMap<Key, String>,           // key -> disasm string
    pub values: HashMap<Key, Option<String>>, // None = varies across executions
    pub call_targets: HashSet<String>,
    pub jump_targets: HashSet<String>,
    pub int_handlers: HashMap<String, BTreeSet<u32>>, // addr -> set of int numbers
}

pub struct Patterns {
    pub log_re: Regex,
    pub call_near_re: Regex,
    pub call_far_re: Regex,
    pub int_re: Regex,
    pub jmp_near_re: Regex,
    pub jmp_far_re: Regex,
}

impl Patterns {
    pub fn new() -> Self {
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

struct PendingInt {
    int_num: u32,
    caller_seg: String,
}

struct ParseState {
    pat: Patterns,
    counts: HashMap<Key, u64>,
    info: HashMap<Key, String>,
    values: HashMap<Key, Option<String>>,
    call_targets: HashSet<String>,
    jump_targets: HashSet<String>,
    int_handlers: HashMap<String, BTreeSet<u32>>,
    pending_int: Option<PendingInt>,
}

impl ParseState {
    fn new() -> Self {
        Self {
            pat: Patterns::new(),
            counts: HashMap::new(),
            info: HashMap::new(),
            values: HashMap::new(),
            call_targets: HashSet::new(),
            jump_targets: HashSet::new(),
            int_handlers: HashMap::new(),
            pending_int: None,
        }
    }

    fn add_line(&mut self, line: &str) -> Result<()> {
        let caps = match self.pat.log_re.captures(line) {
            Some(c) => c,
            None => return Ok(()),
        };

        let addr = caps[1].to_uppercase();
        let bytecode = caps[2].trim_end().to_string();
        let disasm = caps[3].trim().to_string();
        let val = caps
            .get(4)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        let key: Key = (addr.clone(), bytecode.clone());

        *self.counts.entry(key.clone()).or_insert(0) += 1;
        self.info
            .entry(key.clone())
            .or_insert_with(|| disasm.clone());

        // Track whether the register-annotation value is consistent across runs.
        match self.values.get(&key) {
            None => {
                self.values.insert(key.clone(), Some(val.clone()));
            }
            Some(Some(existing)) if existing != &val => {
                self.values.insert(key.clone(), None); // varies
            }
            _ => {}
        }

        // If the previous instruction was `int NN`, the next instruction logged
        // from a different segment is the handler entry point.
        if let Some(PendingInt {
            int_num,
            caller_seg,
        }) = self.pending_int.take()
            && &addr[..4] != caller_seg.as_str()
        {
            self.int_handlers
                .entry(addr.clone())
                .or_default()
                .insert(int_num);
        }

        let seg = addr[..4].to_string();

        if let Some(cap) = self.pat.call_near_re.captures(&disasm) {
            let off = u32::from_str_radix(&cap[1][2..], 16)?;
            self.call_targets.insert(format!("{seg}:{off:04X}"));
            return Ok(());
        }
        if let Some(cap) = self.pat.call_far_re.captures(&disasm) {
            let tseg = u32::from_str_radix(&cap[1][2..], 16)?;
            let toff = u32::from_str_radix(&cap[2][2..], 16)?;
            self.call_targets.insert(format!("{tseg:04X}:{toff:04X}"));
            return Ok(());
        }
        if let Some(cap) = self.pat.int_re.captures(&disasm) {
            let int_num = u32::from_str_radix(&cap[1][2..], 16)?;
            self.pending_int = Some(PendingInt {
                int_num,
                caller_seg: seg,
            });
            return Ok(());
        }
        if let Some(cap) = self.pat.jmp_near_re.captures(&disasm) {
            let off = u32::from_str_radix(&cap[2][2..], 16)?;
            self.jump_targets.insert(format!("{seg}:{off:04X}"));
            return Ok(());
        }
        if let Some(cap) = self.pat.jmp_far_re.captures(&disasm) {
            let tseg = u32::from_str_radix(&cap[1][2..], 16)?;
            let toff = u32::from_str_radix(&cap[2][2..], 16)?;
            self.jump_targets.insert(format!("{tseg:04X}:{toff:04X}"));
        }

        Ok(())
    }

    fn into_parse_result(self) -> ParseResult {
        ParseResult {
            counts: self.counts,
            info: self.info,
            values: self.values,
            call_targets: self.call_targets,
            jump_targets: self.jump_targets,
            int_handlers: self.int_handlers,
        }
    }
}

pub fn parse_log(path: &PathBuf) -> Result<ParseResult> {
    let mut state = ParseState::new();

    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    for line in BufReader::new(file).lines() {
        state.add_line(&line?)?;
    }

    Ok(state.into_parse_result())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal log line that `log_re` will accept.
    fn make_line(addr: &str, bytes: &str, disasm: &str, val: Option<&str>) -> String {
        match val {
            Some(v) => format!("] {addr} {bytes}  {disasm}  {v}"),
            None => format!("] {addr} {bytes}  {disasm}"),
        }
    }

    #[test]
    fn test_non_matching_line() {
        let mut state = ParseState::new();
        state.add_line("this is not a log line").unwrap();
        assert!(state.counts.is_empty());
        assert!(state.call_targets.is_empty());
        assert!(state.jump_targets.is_empty());
    }

    #[test]
    fn test_plain_instruction_count_and_info() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:423F", "55", "push bp", None))
            .unwrap();
        let key = ("0019:423F".to_string(), "55".to_string());
        assert_eq!(state.counts[&key], 1);
        assert_eq!(state.info[&key], "push bp");
    }

    #[test]
    fn test_count_increments() {
        let mut state = ParseState::new();
        let line = make_line("0019:423F", "55", "push bp", None);
        state.add_line(&line).unwrap();
        state.add_line(&line).unwrap();
        let key = ("0019:423F".to_string(), "55".to_string());
        assert_eq!(state.counts[&key], 2);
    }

    #[test]
    fn test_value_consistent() {
        let mut state = ParseState::new();
        let line = make_line("0019:423F", "55", "push bp", Some("AX=0001"));
        state.add_line(&line).unwrap();
        state.add_line(&line).unwrap();
        let key = ("0019:423F".to_string(), "55".to_string());
        assert_eq!(state.values[&key], Some("AX=0001".to_string()));
    }

    #[test]
    fn test_value_varies() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:423F", "55", "push bp", Some("AX=0001")))
            .unwrap();
        state
            .add_line(&make_line("0019:423F", "55", "push bp", Some("AX=0002")))
            .unwrap();
        let key = ("0019:423F".to_string(), "55".to_string());
        assert_eq!(state.values[&key], None);
    }

    #[test]
    fn test_call_near() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:43E4", "E8 8E FE", "call 0x423f", None))
            .unwrap();
        assert!(state.call_targets.contains("0019:423F"));
        assert!(state.jump_targets.is_empty());
    }

    #[test]
    fn test_call_far() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line(
                "0019:43E4",
                "9A 00 00 50 00",
                "call far 0x0050, 0x0000",
                None,
            ))
            .unwrap();
        assert!(state.call_targets.contains("0050:0000"));
        assert!(state.jump_targets.is_empty());
    }

    #[test]
    fn test_int_sets_pending() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:40EC", "CD 21", "int 0x21", None))
            .unwrap();
        let p = state
            .pending_int
            .as_ref()
            .expect("pending_int should be set");
        assert_eq!(p.int_num, 0x21);
        assert_eq!(p.caller_seg, "0019");
    }

    #[test]
    fn test_int_handler_detected() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:40EC", "CD 21", "int 0x21", None))
            .unwrap();
        state
            .add_line(&make_line("0070:1234", "50", "push ax", None))
            .unwrap();
        assert!(state.int_handlers["0070:1234"].contains(&0x21));
    }

    #[test]
    fn test_int_same_segment_no_handler() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:40EC", "CD 21", "int 0x21", None))
            .unwrap();
        state
            .add_line(&make_line("0019:40EE", "90", "nop", None))
            .unwrap();
        assert!(state.int_handlers.is_empty());
    }

    #[test]
    fn test_jmp_near_conditional() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:42FA", "75 04", "jne 0x4300", None))
            .unwrap();
        assert!(state.jump_targets.contains("0019:4300"));
        assert!(state.call_targets.is_empty());
    }

    #[test]
    fn test_jmp_near_short() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:42FA", "EB 04", "jmp short 0x4300", None))
            .unwrap();
        assert!(state.jump_targets.contains("0019:4300"));
    }

    #[test]
    fn test_jmp_near_loop() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:42FA", "E2 FC", "loop 0x42f8", None))
            .unwrap();
        assert!(state.jump_targets.contains("0019:42F8"));
    }

    #[test]
    fn test_jmp_near_jcxz() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line("0019:42FA", "E3 04", "jcxz 0x4300", None))
            .unwrap();
        assert!(state.jump_targets.contains("0019:4300"));
    }

    #[test]
    fn test_jmp_far() {
        let mut state = ParseState::new();
        state
            .add_line(&make_line(
                "0019:43E4",
                "EA 00 10 20 00",
                "jmp far 0x0020, 0x1000",
                None,
            ))
            .unwrap();
        assert!(state.jump_targets.contains("0020:1000"));
        assert!(state.call_targets.is_empty());
    }
}

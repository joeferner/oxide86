use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;

/// A named label with optional comment block, from the config file.
#[derive(Default)]
pub struct LabelEntry {
    pub label: Option<String>,
    pub comment: Option<String>,
}

#[derive(Default)]
pub struct Config {
    pub functions: HashMap<String, LabelEntry>,
    pub labels: HashMap<String, LabelEntry>,
    pub line_comments: HashMap<String, String>, // addr -> comment text
    pub retf_targets: HashMap<String, LabelEntry>,
    pub gaps: HashMap<String, String>, // gap-start addr -> annotation text
    pub mem_labels: HashMap<String, String>, // addr -> label name
}

fn parse_label_entry(v: &Value) -> LabelEntry {
    LabelEntry {
        label: v.get("label").and_then(Value::as_str).map(String::from),
        comment: v.get("comment").and_then(Value::as_str).map(String::from),
    }
}

pub fn load_config(path: &PathBuf) -> Result<Config> {
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

    let str_map = |key: &str| -> HashMap<String, String> {
        data.get(key)
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.to_uppercase(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };

    Ok(Config {
        functions: parse_entry_map("functions"),
        labels: parse_entry_map("labels"),
        line_comments: str_map("lineComments"),
        retf_targets: parse_entry_map("retf_targets"),
        gaps: str_map("gaps"),
        mem_labels: str_map("memLabels"),
    })
}

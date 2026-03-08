mod disasm;
mod loader;
mod output;

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "oxide86-disasm",
    about = "286 disassembler for COM and EXE files"
)]
struct Args {
    /// File to disassemble (.com or .exe)
    file: PathBuf,

    /// JSON config file with entry points and labels
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    /// Map of "SEG:OFF" → label name
    #[serde(default)]
    entry_points: HashMap<String, String>,
    /// Map of "SEG:OFF" → comment string shown next to the instruction
    #[serde(default)]
    comments: HashMap<String, String>,
    /// Segment at which the EXE was loaded (hex string, e.g. "0EEC").
    /// Defaults to "0000". Set this to match the emulator's load address
    /// so that CS values in the output align with execution logs.
    #[serde(default)]
    load_segment: Option<String>,
}

fn parse_seg_off(s: &str) -> Result<(u16, u16)> {
    let (seg_str, off_str) = s
        .split_once(':')
        .with_context(|| format!("expected SEG:OFF format, got '{s}'"))?;
    let seg = u16::from_str_radix(seg_str.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid segment '{seg_str}'"))?;
    let off = u16::from_str_radix(off_str.trim_start_matches("0x"), 16)
        .with_context(|| format!("invalid offset '{off_str}'"))?;
    Ok((seg, off))
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // Parse config first so we have load_segment before loading the image
    let mut load_segment: u16 = 0x0000;
    let mut extra_entries: Vec<(u16, u16, Option<String>)> = Vec::new();
    let mut comments: HashMap<usize, String> = HashMap::new();

    if let Some(config_path) = &args.config {
        let raw = std::fs::read_to_string(config_path)
            .with_context(|| format!("failed to read config '{}'", config_path.display()))?;
        let config: Config = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse config '{}'", config_path.display()))?;
        if let Some(seg_str) = config.load_segment {
            load_segment = u16::from_str_radix(seg_str.trim_start_matches("0x"), 16)
                .with_context(|| format!("invalid loadSegment '{seg_str}'"))?;
        }
        for (addr_str, label) in config.entry_points {
            let (seg, off) = parse_seg_off(&addr_str)
                .with_context(|| format!("invalid config entry point '{addr_str}'"))?;
            extra_entries.push((seg, off, Some(label)));
        }
        for (addr_str, comment) in config.comments {
            let (seg, off) = parse_seg_off(&addr_str)
                .with_context(|| format!("invalid config comment address '{addr_str}'"))?;
            let linear = (seg as usize) << 4 | off as usize;
            comments.insert(linear, comment);
        }
    }

    let image = loader::load(&args.file, load_segment)
        .with_context(|| format!("failed to load '{}'", args.file.display()))?;

    let mut entries: Vec<(u16, u16, Option<String>)> = vec![(image.entry_cs, image.entry_ip, None)];
    entries.extend(extra_entries);

    let disassembly = disasm::disassemble(&image, &entries, comments);
    output::print_disassembly(&image, &disassembly);

    Ok(())
}

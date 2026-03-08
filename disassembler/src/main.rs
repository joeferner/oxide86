mod disasm;
mod loader;
mod output;

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use disasm::{DataRegion, DataType};

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
struct DataSpec {
    /// Data type: "string" (null-terminated ASCII), "bytes" (raw bytes), etc.
    #[serde(rename = "type")]
    data_type: String,
    /// Optional label to emit before this data region.
    label: Option<String>,
    /// Number of bytes for "bytes" type. Required when type is "bytes".
    count: Option<usize>,
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
    /// Map of "SEG:OFF" → data region declaration
    #[serde(default)]
    data: HashMap<String, DataSpec>,
    /// Segment at which the EXE was loaded (hex string, e.g. "0EEC").
    /// Defaults to "0000". Set this to match the emulator's load address
    /// so that CS values in the output align with execution logs.
    #[serde(default)]
    load_segment: Option<String>,
    /// Segment to use for displaying data addresses (hex string, e.g. "0EFC").
    /// Defaults to loadSegment. Set this to the runtime DS value so that
    /// data offsets in the output match what the emulator sees.
    #[serde(default)]
    data_segment: Option<String>,
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
    let mut data_segment: Option<u16> = None;
    let mut extra_entries: Vec<(u16, u16, Option<String>)> = Vec::new();
    let mut comments: HashMap<usize, String> = HashMap::new();
    let mut data_regions: HashMap<usize, DataRegion> = HashMap::new();
    // Keep original address strings for unused-key warnings
    let mut entry_point_keys: Vec<(usize, String)> = Vec::new();
    let mut comment_keys: Vec<(usize, String)> = Vec::new();
    let mut data_keys: Vec<(usize, String)> = Vec::new();

    if let Some(config_path) = &args.config {
        let raw = std::fs::read_to_string(config_path)
            .with_context(|| format!("failed to read config '{}'", config_path.display()))?;
        let config: Config = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse config '{}'", config_path.display()))?;
        if let Some(seg_str) = config.load_segment {
            load_segment = u16::from_str_radix(seg_str.trim_start_matches("0x"), 16)
                .with_context(|| format!("invalid loadSegment '{seg_str}'"))?;
        }
        if let Some(seg_str) = config.data_segment {
            let ds = u16::from_str_radix(seg_str.trim_start_matches("0x"), 16)
                .with_context(|| format!("invalid dataSegment '{seg_str}'"))?;
            data_segment = Some(ds);
        }
        for (addr_str, label) in config.entry_points {
            let (seg, off) = parse_seg_off(&addr_str)
                .with_context(|| format!("invalid config entry point '{addr_str}'"))?;
            let linear = ((seg as usize) << 4) + off as usize;
            entry_point_keys.push((linear, addr_str.clone()));
            extra_entries.push((seg, off, Some(label)));
        }
        for (addr_str, comment) in config.comments {
            let (seg, off) = parse_seg_off(&addr_str)
                .with_context(|| format!("invalid config comment address '{addr_str}'"))?;
            let linear = ((seg as usize) << 4) + off as usize;
            comment_keys.push((linear, addr_str.clone()));
            comments.insert(linear, comment);
        }
        for (addr_str, spec) in config.data {
            let (seg, off) = parse_seg_off(&addr_str)
                .with_context(|| format!("invalid config data address '{addr_str}'"))?;
            let linear = ((seg as usize) << 4) + off as usize;
            let data_type = match spec.data_type.as_str() {
                "string" => DataType::String,
                "bytes" => {
                    let count = spec.count.with_context(|| {
                        format!("'bytes' type at '{addr_str}' requires a 'count' field")
                    })?;
                    DataType::Bytes(count)
                }
                other => anyhow::bail!("unknown data type '{other}' at '{addr_str}'"),
            };
            let _ = (seg, off); // physical address used only for map key
            data_keys.push((linear, addr_str.clone()));
            data_regions.insert(
                linear,
                DataRegion {
                    data_type,
                    label: spec.label,
                },
            );
        }
    }

    let image = loader::load(&args.file, load_segment)
        .with_context(|| format!("failed to load '{}'", args.file.display()))?;

    let mut entries: Vec<(u16, u16, Option<String>)> = vec![(image.entry_cs, image.entry_ip, None)];
    entries.extend(extra_entries);

    let disassembly = disasm::disassemble(&image, &entries, comments, data_regions);
    let effective_data_segment = data_segment.unwrap_or(load_segment);

    // Warn about config keys that didn't match any disassembled content.
    for (linear, addr_str) in &entry_point_keys {
        if !disassembly.instructions.contains_key(linear) {
            eprintln!(
                "warning: entryPoint '{addr_str}' did not produce any disassembled instructions"
            );
        }
    }
    for (linear, addr_str) in &comment_keys {
        if !disassembly.instructions.contains_key(linear) {
            eprintln!("warning: comment at '{addr_str}' does not match any instruction");
        }
    }
    for (linear, addr_str) in &data_keys {
        if *linear < disassembly.image_base || *linear >= disassembly.image_end {
            eprintln!("warning: data entry '{addr_str}' is outside the image range");
        }
    }

    output::print_disassembly(&image, &disassembly, effective_data_segment);

    Ok(())
}

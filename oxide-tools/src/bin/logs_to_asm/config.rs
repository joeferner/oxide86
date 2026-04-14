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

pub struct Config {
    pub functions: HashMap<String, LabelEntry>,
    pub labels: HashMap<String, LabelEntry>,
    pub line_comments: HashMap<String, String>, // addr -> comment text
    pub retf_targets: HashMap<String, LabelEntry>,
    pub gaps: HashMap<String, String>,       // gap-start addr -> annotation text
    pub mem_labels: HashMap<String, String>, // addr -> label name
    pub ports: HashMap<String, String>,      // port number (4 hex digits, uppercase) -> name
}

impl Default for Config {
    fn default() -> Self {
        Self {
            functions: HashMap::new(),
            labels: HashMap::new(),
            line_comments: HashMap::new(),
            retf_targets: HashMap::new(),
            gaps: HashMap::new(),
            mem_labels: HashMap::new(),
            ports: builtin_ports(),
        }
    }
}

/// Well-known IBM PC I/O ports. Keys are 4-digit uppercase hex strings.
/// These are pre-populated so common ports are annotated without any config file.
/// User-supplied `ports` entries override these on a per-port basis.
fn builtin_ports() -> HashMap<String, String> {
    HashMap::from([
        // PIC — Programmable Interrupt Controller (8259A)
        ("0020".into(), "PIC1 command".into()),
        ("0021".into(), "PIC1 data (IMR)".into()),
        ("00A0".into(), "PIC2 command".into()),
        ("00A1".into(), "PIC2 data (IMR)".into()),
        // PIT — Programmable Interval Timer (8253/8254)
        ("0040".into(), "PIT ch0 (system timer)".into()),
        ("0041".into(), "PIT ch1 (DRAM refresh)".into()),
        ("0042".into(), "PIT ch2 (speaker tone)".into()),
        ("0043".into(), "PIT mode/command".into()),
        // Keyboard controller (8042) / system control
        ("0060".into(), "keyboard data / PS2 port 1".into()),
        ("0061".into(), "speaker / system ctrl B".into()),
        ("0064".into(), "keyboard status/command".into()),
        // CMOS / RTC (MC146818)
        ("0070".into(), "CMOS index / NMI mask".into()),
        ("0071".into(), "CMOS data".into()),
        // PS/2 fast A20 gate
        ("0092".into(), "PS2 fast A20 / reset".into()),
        // DMA controller 1 (8237A, channels 0–3, byte transfers)
        ("0000".into(), "DMA1 ch0 base addr".into()),
        ("0001".into(), "DMA1 ch0 count".into()),
        ("0002".into(), "DMA1 ch1 base addr".into()),
        ("0003".into(), "DMA1 ch1 count".into()),
        ("0004".into(), "DMA1 ch2 base addr".into()),
        ("0005".into(), "DMA1 ch2 count".into()),
        ("0006".into(), "DMA1 ch3 base addr".into()),
        ("0007".into(), "DMA1 ch3 count".into()),
        ("0008".into(), "DMA1 status/command".into()),
        ("0009".into(), "DMA1 request".into()),
        ("000A".into(), "DMA1 single mask".into()),
        ("000B".into(), "DMA1 mode".into()),
        ("000C".into(), "DMA1 clear flip-flop".into()),
        ("000D".into(), "DMA1 master clear".into()),
        ("000E".into(), "DMA1 clear mask".into()),
        ("000F".into(), "DMA1 write mask".into()),
        // DMA page registers
        ("0081".into(), "DMA page ch2".into()),
        ("0082".into(), "DMA page ch3".into()),
        ("0083".into(), "DMA page ch1".into()),
        ("0087".into(), "DMA page ch0".into()),
        ("0089".into(), "DMA page ch6".into()),
        ("008A".into(), "DMA page ch7".into()),
        ("008B".into(), "DMA page ch5".into()),
        ("008F".into(), "DMA page ch4".into()),
        // DMA controller 2 (8237A, channels 4–7, word transfers)
        ("00C0".into(), "DMA2 ch4 base addr".into()),
        ("00C2".into(), "DMA2 ch4 count".into()),
        ("00C4".into(), "DMA2 ch5 base addr".into()),
        ("00C6".into(), "DMA2 ch5 count".into()),
        ("00C8".into(), "DMA2 ch6 base addr".into()),
        ("00CA".into(), "DMA2 ch6 count".into()),
        ("00CC".into(), "DMA2 ch7 base addr".into()),
        ("00CE".into(), "DMA2 ch7 count".into()),
        ("00D0".into(), "DMA2 status/command".into()),
        ("00D2".into(), "DMA2 request".into()),
        ("00D4".into(), "DMA2 single mask".into()),
        ("00D6".into(), "DMA2 mode".into()),
        ("00D8".into(), "DMA2 clear flip-flop".into()),
        ("00DA".into(), "DMA2 master clear".into()),
        ("00DC".into(), "DMA2 clear mask".into()),
        ("00DE".into(), "DMA2 write mask".into()),
        // VGA / EGA
        ("03C0".into(), "VGA attr ctrl index/data".into()),
        ("03C1".into(), "VGA attr ctrl data (read)".into()),
        ("03C2".into(), "VGA misc output (w) / input status 0 (r)".into()),
        ("03C4".into(), "VGA sequencer index".into()),
        ("03C5".into(), "VGA sequencer data".into()),
        ("03C6".into(), "VGA PEL mask".into()),
        ("03C7".into(), "VGA PEL read addr / DAC state".into()),
        ("03C8".into(), "VGA PEL write addr".into()),
        ("03C9".into(), "VGA PEL data".into()),
        ("03CA".into(), "VGA feature ctrl (read)".into()),
        ("03CC".into(), "VGA misc output (read)".into()),
        ("03CE".into(), "VGA graphics ctrl index".into()),
        ("03CF".into(), "VGA graphics ctrl data".into()),
        ("03D4".into(), "VGA CRTC index (color)".into()),
        ("03D5".into(), "VGA CRTC data (color)".into()),
        ("03DA".into(), "VGA input status 1 (color)".into()),
        ("03B4".into(), "VGA CRTC index (mono)".into()),
        ("03B5".into(), "VGA CRTC data (mono)".into()),
        ("03BA".into(), "VGA input status 1 (mono)".into()),
        // Floppy disk controller (NEC 765 / 8272A)
        ("03F2".into(), "FDC digital output register".into()),
        ("03F4".into(), "FDC main status register".into()),
        ("03F5".into(), "FDC data (FIFO)".into()),
        ("03F7".into(), "FDC digital input / config ctrl".into()),
        // Serial — COM1
        ("03F8".into(), "COM1 data / baud LSB".into()),
        ("03F9".into(), "COM1 IER / baud MSB".into()),
        ("03FA".into(), "COM1 IIR / FCR".into()),
        ("03FB".into(), "COM1 LCR".into()),
        ("03FC".into(), "COM1 MCR".into()),
        ("03FD".into(), "COM1 LSR".into()),
        ("03FE".into(), "COM1 MSR".into()),
        // Serial — COM2
        ("02F8".into(), "COM2 data / baud LSB".into()),
        ("02F9".into(), "COM2 IER / baud MSB".into()),
        ("02FA".into(), "COM2 IIR / FCR".into()),
        ("02FB".into(), "COM2 LCR".into()),
        ("02FC".into(), "COM2 MCR".into()),
        ("02FD".into(), "COM2 LSR".into()),
        ("02FE".into(), "COM2 MSR".into()),
        // Parallel — LPT1
        ("0378".into(), "LPT1 data".into()),
        ("0379".into(), "LPT1 status".into()),
        ("037A".into(), "LPT1 control".into()),
        // IDE — primary channel
        ("01F0".into(), "IDE0 data".into()),
        ("01F1".into(), "IDE0 error / features".into()),
        ("01F2".into(), "IDE0 sector count".into()),
        ("01F3".into(), "IDE0 LBA low".into()),
        ("01F4".into(), "IDE0 LBA mid".into()),
        ("01F5".into(), "IDE0 LBA high".into()),
        ("01F6".into(), "IDE0 drive/head".into()),
        ("01F7".into(), "IDE0 status/command".into()),
        ("03F6".into(), "IDE0 alt status / device ctrl".into()),
        // x87 FPU
        ("00F0".into(), "FPU clear busy latch".into()),
        ("00F1".into(), "FPU reset".into()),
    ])
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

    // Start from defaults (which include built-in port names), then override each field.
    // For ports specifically, user entries are merged in so they win over built-ins.
    let mut config = Config::default();
    config.functions = parse_entry_map("functions");
    config.labels = parse_entry_map("labels");
    config.line_comments = str_map("lineComments");
    config.retf_targets = parse_entry_map("retf_targets");
    config.gaps = str_map("gaps");
    config.mem_labels = str_map("memLabels");
    config.ports.extend(str_map("ports")); // user wins on conflict
    Ok(config)
}

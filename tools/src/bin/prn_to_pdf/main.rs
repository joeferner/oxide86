mod parser;
mod render;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use render::RenderOptions;

#[derive(Parser)]
#[command(
    name = "prn-to-pdf",
    about = "Convert a .prn printer-output file to PDF",
    long_about = "\
Convert the raw byte stream captured from an emulated parallel port to a PDF.

The tool handles plain text with CR/LF/FF control characters and a subset of
ESC/P commands (bold, italic, underline, double-strike).  Output is rendered
in Courier, preserving column alignment.

Examples:
  prn-to-pdf output.prn
  prn-to-pdf output.prn result.pdf --font-size 10 --paper a4"
)]
struct Args {
    /// Input .prn file
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output .pdf file (default: input path with .pdf extension)
    #[arg(value_name = "OUTPUT")]
    output: Option<PathBuf>,

    /// Font size in points (12 pt Courier = 10 CPI / 6 LPI, the DOS default)
    #[arg(long, default_value_t = 12.0)]
    font_size: f64,

    /// Paper size: letter (8.5×11 in) or a4 (210×297 mm)
    #[arg(long, default_value = "letter")]
    paper: String,

    /// Page margin in inches (applied to top, bottom, and left)
    #[arg(long, default_value_t = 0.25)]
    margin: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let output = args
        .output
        .unwrap_or_else(|| args.input.with_extension("pdf"));

    let data =
        std::fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;

    let doc = parser::parse(&data);

    let margin_pt = args.margin * 72.0;
    let mut opts = RenderOptions {
        font_size: args.font_size,
        line_height: args.font_size * 1.2,
        margin_top: margin_pt,
        margin_bottom: margin_pt,
        margin_left: margin_pt,
        ..RenderOptions::default()
    };

    match args.paper.to_lowercase().as_str() {
        "a4" => {
            opts.paper_width = 210.0 / 25.4 * 72.0;
            opts.paper_height = 297.0 / 25.4 * 72.0;
        }
        _ => {
            // letter and any unrecognized value: keep defaults from RenderOptions::default()
        }
    }

    render::render(&doc, &opts, &output)
        .with_context(|| format!("writing {}", output.display()))?;

    println!("{}", output.display());
    Ok(())
}

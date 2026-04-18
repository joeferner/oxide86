//! Render a parsed [`Doc`] to a PDF file using printpdf.
//!
//! Each page in the [`Doc`] becomes one PDF page.  Text is rendered in
//! Courier (or one of its bold/italic variants) at a configurable point size.
//!
//! Coordinate system note: printpdf places the origin at the **bottom-left**
//! corner of the page, with y increasing upward.  All measurements are first
//! computed in points then converted to mm (as f32) for the printpdf API.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Result;
use printpdf::*;

use crate::parser::{Doc, Style};

const PT_PER_INCH: f64 = 72.0;
const MM_PER_INCH: f64 = 25.4;

fn pt_to_mm(pt: f64) -> f32 {
    (pt / PT_PER_INCH * MM_PER_INCH) as f32
}

/// Options that control how the document is laid out on the page.
pub struct RenderOptions {
    /// Font size in points (controls character size and CPI).
    /// Courier advance = 0.6 × font_size, so 12 pt ≈ 10 CPI.
    pub font_size: f64,
    /// Distance between baselines in points (0 → 1.2 × font_size).
    pub line_height: f64,
    /// Page margins in points (top / bottom / left).
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub margin_left: f64,
    /// Paper dimensions in points.
    pub paper_width: f64,
    pub paper_height: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        let font_size = 12.0_f64; // 12 pt Courier → 10 CPI, 6 LPI
        let margin = 0.5 * PT_PER_INCH; // 0.5-inch margins
        Self {
            font_size,
            line_height: font_size * 1.2,
            margin_top: margin,
            margin_bottom: margin,
            margin_left: margin,
            paper_width: 8.5 * PT_PER_INCH,
            paper_height: 11.0 * PT_PER_INCH,
        }
    }
}

/// Render `doc` to a PDF file at `output`.
pub fn render(doc: &Doc, opts: &RenderOptions, output: &Path) -> Result<()> {
    let width_mm = pt_to_mm(opts.paper_width);
    let height_mm = pt_to_mm(opts.paper_height);
    let line_h = if opts.line_height > 0.0 {
        opts.line_height
    } else {
        opts.font_size * 1.2
    };

    // Courier advance width = 600 units / 1000 units per em = 0.6 × font_size
    let char_w_pt = opts.font_size * 0.6;
    let margin_left_mm = pt_to_mm(opts.margin_left);
    let margin_bottom_mm = pt_to_mm(opts.margin_bottom);

    let (pdf, first_page, first_layer) =
        PdfDocument::new("PRN Output", Mm(width_mm), Mm(height_mm), "Layer 1");

    let font_normal = pdf.add_builtin_font(BuiltinFont::Courier)?;
    let font_bold = pdf.add_builtin_font(BuiltinFont::CourierBold)?;
    let font_italic = pdf.add_builtin_font(BuiltinFont::CourierOblique)?;
    let font_bold_italic = pdf.add_builtin_font(BuiltinFont::CourierBoldOblique)?;

    let mut first = true;

    for page in &doc.pages {
        let layer = if first {
            first = false;
            pdf.get_page(first_page).get_layer(first_layer)
        } else {
            let (pg, ly) = pdf.add_page(Mm(width_mm), Mm(height_mm), "Layer 1");
            pdf.get_page(pg).get_layer(ly)
        };

        for (row_idx, line) in page.iter().enumerate() {
            if line.cells.is_empty() {
                continue;
            }

            // Baseline y from top: margin_top + (row + 1) * line_height
            // Subtract from page height because PDF y-axis is bottom-up.
            let y_mm =
                pt_to_mm(opts.paper_height - opts.margin_top - (row_idx as f64 + 1.0) * line_h);

            if y_mm < margin_bottom_mm {
                // Past the bottom margin – remaining rows would overflow the page.
                break;
            }

            // Group consecutive cells with the same style into text runs.
            let runs = build_runs(line);

            for (run_col, text, style) in &runs {
                if text.trim().is_empty() {
                    continue;
                }

                let x_mm = margin_left_mm + (*run_col as f32) * pt_to_mm(char_w_pt);
                let font = choose_font(
                    style,
                    &font_normal,
                    &font_bold,
                    &font_italic,
                    &font_bold_italic,
                );
                layer.use_text(
                    text.as_str(),
                    opts.font_size as f32,
                    Mm(x_mm),
                    Mm(y_mm),
                    font,
                );

                if style.underline {
                    draw_underline(
                        &layer,
                        x_mm,
                        y_mm,
                        text.chars().count(),
                        pt_to_mm(char_w_pt),
                        opts,
                    );
                }
            }
        }
    }

    let file = File::create(output)?;
    pdf.save(&mut BufWriter::new(file))?;
    Ok(())
}

// ─── helpers ────────────────────────────────────────────────────────────────

type Run = (usize, String, Style);

/// Collapse a line's cells into (start_col, text, style) runs.
fn build_runs(line: &crate::parser::Line) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let ncols = line.cells.len();
    let mut run_col = 0usize;
    let mut run_text = String::new();
    let mut run_style: Option<Style> = None;

    for col in 0..ncols {
        let (ch, st) = match &line.cells[col] {
            Some(c) => (c.ch, c.style),
            None => (' ', Style::default()),
        };

        match run_style {
            Some(s) if s == st => {
                run_text.push(ch);
            }
            _ => {
                if !run_text.is_empty() {
                    runs.push((run_col, run_text.clone(), run_style.unwrap()));
                }
                run_col = col;
                run_text = String::from(ch);
                run_style = Some(st);
            }
        }
    }
    if !run_text.is_empty()
        && let Some(st) = run_style
    {
        runs.push((run_col, run_text, st));
    }

    // Trim trailing whitespace from the last run
    if let Some(last) = runs.last_mut() {
        last.1 = last.1.trim_end().to_string();
    }

    runs
}

fn choose_font<'a>(
    style: &Style,
    normal: &'a IndirectFontRef,
    bold: &'a IndirectFontRef,
    italic: &'a IndirectFontRef,
    bold_italic: &'a IndirectFontRef,
) -> &'a IndirectFontRef {
    match (style.bold || style.double_strike, style.italic) {
        (true, true) => bold_italic,
        (true, false) => bold,
        (false, true) => italic,
        (false, false) => normal,
    }
}

fn draw_underline(
    layer: &PdfLayerReference,
    x_mm: f32,
    baseline_y_mm: f32,
    char_count: usize,
    char_w_mm: f32,
    opts: &RenderOptions,
) {
    let width_mm = char_w_mm * char_count as f32;
    // Underline sits ~10% of the font size below the baseline
    let ul_y_mm = baseline_y_mm - pt_to_mm(opts.font_size * 0.1);
    let line = Line {
        points: vec![
            (Point::new(Mm(x_mm), Mm(ul_y_mm)), false),
            (Point::new(Mm(x_mm + width_mm), Mm(ul_y_mm)), false),
        ],
        is_closed: false,
    };
    layer.set_outline_thickness(0.5);
    layer.add_line(line);
}

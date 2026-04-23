//! Parse raw `.prn` printer-output bytes into a structured page/line/cell grid.
//!
//! Control characters handled:
//! - `CR` (0x0D) – move to column 0, no line advance (for overprinting)
//! - `LF` (0x0A) – advance one line
//! - `FF` (0x0C) – form feed (start new page)
//! - `BS` (0x08) – backspace (column − 1)
//! - `HT` (0x09) – horizontal tab to the next 8-column stop
//!
//! ESC/P commands handled (subset):
//! - `ESC @` – reset all attributes
//! - `ESC E` / `ESC F` – bold on/off
//! - `ESC G` / `ESC H` – double-strike on/off
//! - `ESC 4` / `ESC 5` – italic on/off
//! - `ESC -` *n* – underline on (*n* ≠ 0) / off (*n* = 0)
//! - One-parameter commands skipped cleanly
//! - Graphics data commands (K/L/Y/Z/*) skipped cleanly

/// Per-cell text style.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub double_strike: bool,
}

/// A single printed character at a grid position.
#[derive(Clone, Debug)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

/// One row (physical printer line) of cells, indexed by column.
///
/// Gaps (columns with no printout) are stored as `None`.
#[derive(Clone, Debug, Default)]
pub struct Line {
    pub cells: Vec<Option<Cell>>,
}

impl Line {
    /// Place `cell` at `col`, growing the vec as needed.
    /// Later writes to the same column overwrite earlier ones (overprint semantics).
    pub fn put(&mut self, col: usize, cell: Cell) {
        if col >= self.cells.len() {
            self.cells.resize(col + 1, None);
        }
        self.cells[col] = Some(cell);
    }
}

/// Parsed printer output: a list of pages, each a list of lines.
pub struct Doc {
    pub pages: Vec<Vec<Line>>,
}

/// Parse raw `.prn` bytes into a [`Doc`].
pub fn parse(data: &[u8]) -> Doc {
    let mut pages: Vec<Vec<Line>> = vec![vec![Line::default()]];
    let mut row: usize = 0;
    let mut col: usize = 0;
    let mut style = Style::default();

    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        match b {
            0x0D => {
                // CR: return to column 0, no line advance
                col = 0;
                i += 1;
            }
            0x0A => {
                // LF: advance one line
                row += 1;
                let page = pages.last_mut().unwrap();
                while page.len() <= row {
                    page.push(Line::default());
                }
                i += 1;
            }
            0x0C => {
                // FF: new page
                pages.push(vec![Line::default()]);
                row = 0;
                col = 0;
                i += 1;
            }
            0x08 => {
                // BS: backspace
                col = col.saturating_sub(1);
                i += 1;
            }
            0x09 => {
                // HT: next 8-column tab stop
                col = (col / 8 + 1) * 8;
                i += 1;
            }
            0x1B => {
                // ESC sequence
                let next = parse_esc(data, i + 1, &mut style);
                i = next;
            }
            0x0E | 0x0F | 0x12 | 0x14 => {
                // SO/SI/DC2/DC4: expanded/condensed mode – no grid effect, skip
                i += 1;
            }
            _ if b >= 0x20 => {
                // Printable character
                let page = pages.last_mut().unwrap();
                // Ensure the current row exists
                while page.len() <= row {
                    page.push(Line::default());
                }
                page[row].put(
                    col,
                    Cell {
                        ch: b as char,
                        style,
                    },
                );
                col += 1;
                i += 1;
            }
            _ => {
                // Other control bytes: ignore
                i += 1;
            }
        }
    }

    // Drop trailing completely-empty pages
    while pages.len() > 1 {
        let is_empty = pages
            .last()
            .is_none_or(|p| p.iter().all(|l| l.cells.is_empty()));
        if is_empty {
            pages.pop();
        } else {
            break;
        }
    }

    Doc { pages }
}

/// Parse one ESC sequence starting at `start` (the byte immediately after the ESC).
///
/// Returns the index of the **next unprocessed** byte.
fn parse_esc(data: &[u8], start: usize, style: &mut Style) -> usize {
    if start >= data.len() {
        return start;
    }
    let cmd = data[start];
    match cmd {
        // ── Attribute commands (no parameters) ─────────────────────────────
        b'@' => {
            *style = Style::default();
            start + 1
        }
        b'E' => {
            style.bold = true;
            start + 1
        }
        b'F' => {
            style.bold = false;
            start + 1
        }
        b'G' => {
            style.double_strike = true;
            start + 1
        }
        b'H' => {
            style.double_strike = false;
            start + 1
        }
        b'4' => {
            style.italic = true;
            start + 1
        }
        b'5' => {
            style.italic = false;
            start + 1
        }
        b'2' => {
            // Set line spacing to 1/6 inch (default) – no parameter
            start + 1
        }

        // ── Commands with one parameter byte ───────────────────────────────
        b'-' => {
            // ESC - n : underline on/off
            if start + 1 < data.len() {
                style.underline = data[start + 1] != 0;
                start + 2
            } else {
                start + 1
            }
        }
        b'M' | b'P' | b'l' | b'Q' | b'C' | b'N' | b'3' | b'A' | b'J' | b'j' | b'!' | b'k'
        | b'x' | b'r' | b't' | b'a' | b'S' | b'T' | b'W' | b'p' | b'1' | b'6' | b'7' | b'8'
        | b'9' | b'0' => {
            // One-byte parameter commands
            start + 2
        }

        // ── Graphics / bit-image commands (variable length) ────────────────
        b'K' | b'L' | b'Y' | b'Z' => {
            // ESC K/L/Y/Z n1 n2 [n1 + n2*256 bytes of data]
            if start + 2 < data.len() {
                let n1 = data[start + 1] as usize;
                let n2 = data[start + 2] as usize;
                let count = n1 + n2 * 256;
                start + 3 + count
            } else {
                start + 1
            }
        }
        b'*' => {
            // ESC * m n1 n2 [n1 + n2*256 bytes]
            if start + 3 < data.len() {
                let n1 = data[start + 2] as usize;
                let n2 = data[start + 3] as usize;
                let count = n1 + n2 * 256;
                start + 4 + count
            } else {
                start + 1
            }
        }
        b'(' => {
            // Extended ESC ( x n1 n2 [data]
            if start + 3 < data.len() {
                let n1 = data[start + 2] as usize;
                let n2 = data[start + 3] as usize;
                let count = n1 + n2 * 256;
                start + 4 + count
            } else {
                start + 1
            }
        }

        _ => {
            // Unknown command: skip just the command byte
            start + 1
        }
    }
}

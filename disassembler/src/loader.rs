use anyhow::{bail, Context, Result};
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageKind {
    Com,
    Exe,
}

/// A flat byte image loaded from a COM or EXE file, with its entry point.
pub struct LoadedImage {
    /// Flat byte image. For COM files this is the raw file content;
    /// for EXE files this is the load image (header stripped).
    pub data: Vec<u8>,
    /// Physical base address (load_segment << 4). image[0] maps to this linear address.
    pub base_linear: usize,
    /// Entry segment (CS at entry).
    pub entry_cs: u16,
    /// Entry offset (IP at entry).
    pub entry_ip: u16,
    #[allow(dead_code)]
    pub kind: ImageKind,
}

pub fn load(path: &Path, load_segment: u16) -> Result<LoadedImage> {
    let data = fs::read(path).with_context(|| format!("reading '{}'", path.display()))?;

    if data.len() >= 2 && data[0] == 0x4D && data[1] == 0x5A {
        load_exe(data, load_segment)
    } else {
        load_com(data)
    }
}

fn load_com(data: Vec<u8>) -> Result<LoadedImage> {
    // COM: raw binary, entry at 0x0000:0x0100
    // The file content is loaded starting at linear address 0x100 (offset 0x100 in segment 0x0000)
    // We prepend 0x100 zero bytes so that linear address 0x100 maps to data[0x100]
    let mut image = vec![0u8; 0x100];
    image.extend_from_slice(&data);
    Ok(LoadedImage {
        data: image,
        base_linear: 0,
        entry_cs: 0x0000,
        entry_ip: 0x0100,
        kind: ImageKind::Com,
    })
}

/// MZ EXE header fields we care about (all little-endian u16).
/// Offsets per the MZ spec:
///   0x00 magic (MZ)
///   0x02 last_page_bytes
///   0x04 page_count
///   0x06 reloc_count
///   0x08 header_paragraphs
///   0x0A min_extra_paragraphs
///   0x0C max_extra_paragraphs
///   0x0E initial_ss
///   0x10 initial_sp
///   0x12 checksum
///   0x14 initial_ip
///   0x16 initial_cs
///   0x18 reloc_table_offset
///   0x1A overlay_number
fn load_exe(data: Vec<u8>, load_segment: u16) -> Result<LoadedImage> {
    if data.len() < 0x1C {
        bail!("file too small to be a valid MZ EXE");
    }

    let read_u16 = |offset: usize| -> u16 { u16::from_le_bytes([data[offset], data[offset + 1]]) };

    let last_page_bytes = read_u16(0x02);
    let page_count = read_u16(0x04) as usize;
    let header_paragraphs = read_u16(0x08) as usize;
    let initial_ip = read_u16(0x14);
    let initial_cs = read_u16(0x16) as i16 as i32; // signed relative to load segment

    let header_size = header_paragraphs * 16;

    // Total image size from MZ header
    let image_size = if last_page_bytes == 0 {
        page_count * 512
    } else {
        (page_count - 1) * 512 + last_page_bytes as usize
    };
    let image_size = image_size.saturating_sub(header_size);

    if data.len() < header_size {
        bail!(
            "EXE header claims {header_size} bytes but file is only {} bytes",
            data.len()
        );
    }

    let base_linear = (load_segment as usize) << 4;
    let entry_cs = (load_segment as i32 + initial_cs) as u16;
    let entry_ip = initial_ip;

    // Slice out the load image, padded with zeros if necessary for addressing
    let available = data
        .len()
        .saturating_sub(header_size)
        .min(image_size.max(data.len() - header_size));
    let mut image = data[header_size..header_size + available].to_vec();

    // Pad so that the entry point offset within the image is in bounds.
    // Entry linear address relative to image start = (entry_cs - load_segment) * 16 + entry_ip
    let entry_image_offset = ((entry_cs as usize) << 4) + entry_ip as usize - base_linear;
    if entry_image_offset >= image.len() {
        image.resize(entry_image_offset + 1, 0xFF);
    }

    Ok(LoadedImage {
        data: image,
        base_linear,
        entry_cs,
        entry_ip,
        kind: ImageKind::Exe,
    })
}

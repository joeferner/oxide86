use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    println!("cargo:rerun-if-changed=assets/logo.png");

    let png_data = fs::read(manifest_dir.join("assets/logo.png"))
        .expect("failed to read core/assets/logo.png");

    let decoder = png::Decoder::new(std::io::Cursor::new(png_data));
    let mut reader = decoder.read_info().expect("invalid logo.png");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .expect("failed to read logo.png frame");

    let w = frame.width as usize;
    let h = frame.height as usize;
    let size = frame.buffer_size();

    let rgba: Vec<u8> = match frame.color_type {
        png::ColorType::Rgba => buf[..size].to_vec(),
        png::ColorType::Rgb => buf[..size]
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255u8])
            .collect(),
        other => panic!("unsupported PNG color type: {:?}", other),
    };

    fs::write(out_dir.join("logo_rgba.bin"), &rgba).expect("failed to write logo_rgba.bin");

    fs::write(
        out_dir.join("logo_dims.rs"),
        format!(
            "pub const LOGO_PNG_WIDTH: usize = {};\npub const LOGO_PNG_HEIGHT: usize = {};\n",
            w, h
        ),
    )
    .expect("failed to write logo_dims.rs");
}

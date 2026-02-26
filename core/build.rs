use std::process::Command;
use walkdir::WalkDir;

fn main() {
    let asm_dir = "src/test_data";

    for entry in WalkDir::new(asm_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("asm") {
            println!("cargo:rerun-if-changed={}", path.display());

            let output_obj = path.with_extension("com");
            let status = Command::new("nasm")
                .arg("-f")
                .arg("bin")
                .arg(path)
                .arg("-o")
                .arg(&output_obj)
                .status()
                .expect("Failed to run nasm");

            if !status.success() {
                panic!("NASM failed to assemble {:?}", path);
            }
        }
    }
}

use anyhow::{Context, Result};
use clap::Args;

use crate::disk::{normalise_disk_path, open_disk};

#[derive(Args)]
pub struct DirArgs {
    /// Path to disk image file
    disk: String,

    /// Directory path to list (default: root). Use forward or backslashes.
    #[arg(default_value = "/")]
    path: String,

    /// Show all entries including those with hidden/system attributes
    #[arg(short, long)]
    all: bool,
}

pub fn run(args: DirArgs) -> Result<()> {
    let mut cursor = open_disk(&args.disk)?;
    let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
        .with_context(|| format!("opening FAT filesystem in '{}'", args.disk))?;

    let norm_path = normalise_disk_path(&args.path);
    let root = fs.root_dir();

    // Navigate to the target directory
    let dir = if norm_path == "/" {
        root
    } else {
        root.open_dir(&norm_path[1..]) // strip leading /
            .with_context(|| format!("opening directory '{norm_path}'"))?
    };

    // Print header
    let label = fs.volume_label();
    println!(" Volume: {}", label.trim());
    println!(" Directory of {}", norm_path);
    println!();
    println!("{:<13} {:>10}  {:<20} Attr", "Name", "Size", "Modified");
    println!("{}", "-".repeat(62));

    let mut file_count = 0usize;
    let mut dir_count = 0usize;
    let mut total_bytes = 0u64;

    for entry in dir.iter() {
        let entry = entry.with_context(|| "reading directory entry")?;
        let name = entry.file_name();

        // fatfs returns "." and ".." - skip them for cleaner output
        if name == "." || name == ".." {
            continue;
        }

        let is_dir = entry.is_dir();
        let attrs = entry.attributes();
        let hidden = attrs.contains(fatfs::FileAttributes::HIDDEN)
            || attrs.contains(fatfs::FileAttributes::SYSTEM);

        if hidden && !args.all {
            continue;
        }

        let size: u64 = if is_dir {
            0
        } else {
            let mut f = entry.to_file();
            use std::io::Seek;
            f.seek(std::io::SeekFrom::End(0)).unwrap_or(0)
        };

        let modified = entry.modified();
        let d = modified.date;
        let t = modified.time;
        let date_str = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            d.year, d.month, d.day, t.hour, t.min
        );

        let attr_str = format!(
            "{}{}{}{}{}",
            if attrs.contains(fatfs::FileAttributes::READ_ONLY) {
                "R"
            } else {
                "-"
            },
            if hidden { "H" } else { "-" },
            if attrs.contains(fatfs::FileAttributes::SYSTEM) {
                "S"
            } else {
                "-"
            },
            if is_dir { "D" } else { "-" },
            if attrs.contains(fatfs::FileAttributes::ARCHIVE) {
                "A"
            } else {
                "-"
            },
        );

        if is_dir {
            println!(
                "{:<13} {:>10}  {:<20} {}",
                format!("{name}/"),
                "<DIR>",
                date_str,
                attr_str
            );
            dir_count += 1;
        } else {
            println!("{:<13} {:>10}  {:<20} {}", name, size, date_str, attr_str);
            file_count += 1;
            total_bytes += size;
        }
    }

    println!("{}", "-".repeat(62));
    println!(
        "{} file(s), {} bytes  |  {} dir(s)",
        file_count, total_bytes, dir_count
    );

    Ok(())
}

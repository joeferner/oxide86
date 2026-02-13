use anyhow::{Context, Result, bail};
use clap::Args;
use std::io::{Read, Write};

use crate::disk::{is_disk_path, normalise_disk_path, open_disk, with_disk_mut};

#[derive(Args)]
pub struct CopyArgs {
    /// Path to disk image file
    disk: String,

    /// Source path. Prefix with '::' for disk paths (e.g. ::FILE.TXT or ::SUBDIR/FILE.TXT)
    src: String,

    /// Destination path. Prefix with '::' for disk paths (e.g. ::FILE.TXT or ::SUBDIR/FILE.TXT)
    dst: String,
}

pub fn run(args: CopyArgs) -> Result<()> {
    let src_is_disk = is_disk_path(&args.src);
    let dst_is_disk = is_disk_path(&args.dst);

    match (src_is_disk, dst_is_disk) {
        (true, false) => copy_from_disk(&args.disk, &args.src, &args.dst),
        (false, true) => copy_to_disk(&args.disk, &args.src, &args.dst),
        (true, true) => bail!("Both src and dst are disk paths. Use host paths for at least one."),
        (false, false) => bail!("Neither src nor dst is a disk path. Prefix disk paths with '::'"),
    }
}

/// Copy a file from the disk image to the host filesystem.
fn copy_from_disk(disk: &str, src: &str, dst: &str) -> Result<()> {
    let disk_path = normalise_disk_path(src);

    let mut cursor = open_disk(disk)?;
    let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
        .with_context(|| format!("opening FAT filesystem in '{disk}'"))?;

    let root = fs.root_dir();
    let mut file = root
        .open_file(&disk_path[1..]) // strip leading /
        .with_context(|| format!("opening disk file '{disk_path}'"))?;

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .with_context(|| format!("reading disk file '{disk_path}'"))?;

    // If dst is a directory, append the filename from the disk path
    let dst_path = resolve_dst_host_path(dst, &disk_path);

    std::fs::write(&dst_path, &buf).with_context(|| format!("writing host file '{dst_path}'"))?;

    println!("Copied {} ({} bytes) -> {}", disk_path, buf.len(), dst_path);
    Ok(())
}

/// Copy a file from the host filesystem to the disk image.
fn copy_to_disk(disk: &str, src: &str, dst: &str) -> Result<()> {
    let disk_path = normalise_disk_path(dst);

    let host_data = std::fs::read(src).with_context(|| format!("reading host file '{src}'"))?;
    let size = host_data.len();

    with_disk_mut(disk, |fs| {
        // Ensure intermediate directories exist
        let disk_path_trimmed = &disk_path[1..]; // strip leading /
        create_dirs_for(fs.root_dir(), disk_path_trimmed)?;

        let mut file = fs
            .root_dir()
            .create_file(disk_path_trimmed)
            .with_context(|| format!("creating disk file '{disk_path}'"))?;

        file.write_all(&host_data)
            .with_context(|| format!("writing disk file '{disk_path}'"))?;
        Ok(())
    })?;

    println!("Copied {} ({} bytes) -> {disk_path}", src, size);
    Ok(())
}

/// Create all parent directories for the given path if they don't exist.
fn create_dirs_for<T: fatfs::ReadWriteSeek>(root: fatfs::Dir<'_, T>, path: &str) -> Result<()> {
    let parent = match path.rfind('/') {
        Some(idx) => &path[..idx],
        None => return Ok(()), // file is in root, no dirs to create
    };
    if parent.is_empty() {
        return Ok(());
    }
    // Create each component
    let mut current = root;
    for part in parent.split('/') {
        if part.is_empty() {
            continue;
        }
        current = match current.create_dir(part) {
            Ok(d) => d,
            Err(_) => current
                .open_dir(part)
                .with_context(|| format!("opening directory '{part}'"))?,
        };
    }
    Ok(())
}

/// If `dst` is an existing host directory, append the filename from `disk_path`.
fn resolve_dst_host_path(dst: &str, disk_path: &str) -> String {
    let dst_meta = std::fs::metadata(dst);
    let is_dir = dst_meta.map(|m| m.is_dir()).unwrap_or(false);
    if is_dir {
        let filename = disk_path.rsplit('/').next().unwrap_or("file");
        format!("{}/{}", dst.trim_end_matches('/'), filename)
    } else {
        dst.to_string()
    }
}

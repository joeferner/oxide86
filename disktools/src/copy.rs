use anyhow::{Context, Result, bail};
use clap::Args;
use std::io::{Read, Write};
use std::path::Path;

use crate::disk::{is_disk_path, normalise_disk_path, open_disk, with_disk_mut};

#[derive(Args)]
pub struct CopyArgs {
    /// Path to disk image file
    #[arg(short = 'i', long = "image")]
    disk: String,

    /// Source and destination paths (last argument is destination).
    ///
    /// Prefix with '::' to refer to disk paths, e.g.:
    ///   copy -i disk.img file.txt        ::file.txt   (host -> disk)
    ///   copy -i disk.img file1.txt file2.txt ::dir/   (multiple host -> disk dir)
    ///   copy -i disk.img ::file.txt       ./output/   (disk -> host)
    ///   copy -i disk.img ::a.txt ::b.txt  ./output/   (multiple disk -> host dir)
    #[arg(required = true, num_args = 2..)]
    files: Vec<String>,
}

pub fn run(args: CopyArgs) -> Result<()> {
    let (dst, srcs) = args.files.split_last().unwrap();

    // Validate: all sources must be the same kind (all disk or all host)
    let any_src_disk = srcs.iter().any(|s| is_disk_path(s));
    let any_src_host = srcs.iter().any(|s| !is_disk_path(s));
    if any_src_disk && any_src_host {
        bail!(
            "Mixed source paths: all sources must be either disk paths ('::') or host paths, not both"
        );
    }

    let dst_is_disk = is_disk_path(dst);

    match (any_src_disk, dst_is_disk) {
        (true, false) => copy_from_disk(&args.disk, srcs, dst),
        (false, true) => copy_to_disk(&args.disk, srcs, dst),
        (true, true) => bail!("Both src and dst are disk paths. Use a host path for at least one."),
        (false, false) => bail!("Neither src nor dst is a disk path. Prefix disk paths with '::'"),
    }
}

/// Copy one or more files from the disk image to the host filesystem.
fn copy_from_disk(disk: &str, srcs: &[String], dst: &str) -> Result<()> {
    // Determine if dst must be a directory
    let multi = srcs.len() > 1;
    if multi {
        // Destination must be an existing host directory
        let meta = std::fs::metadata(dst).with_context(|| {
            format!("destination '{dst}' must be an existing directory for multi-file copy")
        })?;
        if !meta.is_dir() {
            bail!("destination '{dst}' must be a directory when copying multiple files");
        }
    }

    let mut cursor = open_disk(disk)?;
    let fs = fatfs::FileSystem::new(&mut cursor, fatfs::FsOptions::new())
        .with_context(|| format!("opening FAT filesystem in '{disk}'"))?;
    let root = fs.root_dir();

    for src in srcs {
        let disk_path = normalise_disk_path(src);

        let mut file = root
            .open_file(&disk_path[1..]) // strip leading /
            .with_context(|| format!("opening disk file '{disk_path}'"))?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .with_context(|| format!("reading disk file '{disk_path}'"))?;

        let dst_path = resolve_dst_host_path(dst, &disk_path);
        std::fs::write(&dst_path, &buf)
            .with_context(|| format!("writing host file '{dst_path}'"))?;

        println!("Copied {} ({} bytes) -> {}", disk_path, buf.len(), dst_path);
    }

    Ok(())
}

/// Copy one or more files from the host filesystem to the disk image.
fn copy_to_disk(disk: &str, srcs: &[String], dst: &str) -> Result<()> {
    let disk_dst = normalise_disk_path(dst);

    // Collect host file data before opening the disk (keeps borrow checker happy)
    let host_files: Vec<(String, Vec<u8>)> = srcs
        .iter()
        .map(|src| {
            let data = std::fs::read(src).with_context(|| format!("reading host file '{src}'"))?;
            Ok((src.clone(), data))
        })
        .collect::<Result<_>>()?;

    let multi = host_files.len() > 1;

    with_disk_mut(disk, |fs| {
        for (src, host_data) in &host_files {
            // For multi-file copies, treat dst as a directory and append the filename.
            // For single-file copies, use dst as-is (may be a file or directory).
            let disk_path = if multi {
                let filename = Path::new(src)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(src.as_str());
                format!("{}/{}", disk_dst.trim_end_matches('/'), filename)
            } else {
                disk_dst.clone()
            };

            let disk_path_trimmed = &disk_path[1..]; // strip leading /
            create_dirs_for(fs.root_dir(), disk_path_trimmed)?;

            let mut file = fs
                .root_dir()
                .create_file(disk_path_trimmed)
                .with_context(|| format!("creating disk file '{disk_path}'"))?;

            file.write_all(host_data)
                .with_context(|| format!("writing disk file '{disk_path}'"))?;

            println!("Copied {} ({} bytes) -> {disk_path}", src, host_data.len());
        }
        Ok(())
    })
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

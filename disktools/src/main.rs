mod copy;
mod dir;
mod disk;
mod format;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "oxide86-disktools", about = "Disk image tools for oxide86")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a blank FAT-formatted disk image
    Format(format::FormatArgs),
    /// List files and directories on a disk image
    Dir(dir::DirArgs),
    /// Copy files to or from a disk image (last argument is destination).
    ///
    /// Prefix paths with '::' to refer to disk paths, e.g.:
    ///   copy -i disk.img file.txt         ::file.txt   (host -> disk)
    ///   copy -i disk.img a.txt b.txt      ::dir/       (multiple host -> disk dir)
    ///   copy -i disk.img ::file.txt       ./output/    (disk -> host)
    ///   copy -i disk.img ::a.txt ::b.txt  ./output/    (multiple disk -> host dir)
    Copy(copy::CopyArgs),
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    match cli.command {
        Command::Format(args) => format::run(args),
        Command::Dir(args) => dir::run(args),
        Command::Copy(args) => copy::run(args),
    }
}

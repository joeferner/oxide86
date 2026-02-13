mod copy;
mod dir;
mod disk;
mod format;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "emu86-disktools", about = "Disk image tools for emu86")]
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
    /// Copy files to or from a disk image.
    ///
    /// Prefix paths with '::' to refer to disk paths, e.g.:
    ///   copy disk.img file.txt ::file.txt   (host -> disk)
    ///   copy disk.img ::file.txt file.txt   (disk -> host)
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

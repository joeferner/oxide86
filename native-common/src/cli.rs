use clap::Parser;

#[derive(Parser)]
pub struct CommonCli {
    /// Path to the program binary to load and execute (not used with --boot)
    #[arg(required_unless_present = "boot")]
    pub program: Option<String>,

    /// Boot from disk image instead of loading a program
    #[arg(long)]
    pub boot: bool,

    /// Boot drive number (0x00 for floppy A:, 0x01 for floppy B:, 0x80 for hard disk C:)
    #[arg(long, default_value = "0x00")]
    pub boot_drive: String,

    /// Starting segment address (default: 0x0000)
    #[arg(long, default_value = "0x0000")]
    pub segment: String,

    /// Starting offset address (default: 0x0100, like .COM files)
    #[arg(long, default_value = "0x0100")]
    pub offset: String,

    /// Path to disk image file for floppy A:
    #[arg(long = "floppy-a")]
    pub floppy_a: Option<String>,

    /// Path to disk image file for floppy B:
    #[arg(long = "floppy-b")]
    pub floppy_b: Option<String>,

    /// Path to hard disk image file(s) - can be specified multiple times for C:, D:, etc.
    #[arg(long = "hdd", action = clap::ArgAction::Append)]
    pub hard_disks: Vec<String>,

    /// Device to attach to COM1 (e.g., "mouse")
    #[arg(long = "com1", value_name = "DEVICE")]
    pub com1_device: Option<String>,

    /// Device to attach to COM2 (e.g., "mouse")
    #[arg(long = "com2", value_name = "DEVICE")]
    pub com2_device: Option<String>,

    /// Enable execution logging (logs each instruction to emu86.log)
    #[arg(long = "exec-log")]
    pub exec_log: bool,

    /// Enable interrupt logging (logs INT calls to emu86.log)
    #[arg(long = "int-log")]
    pub int_log: bool,

    /// CPU type to emulate (8086, 286, 386, 486)
    #[arg(long = "cpu", default_value = "8086")]
    pub cpu_type: String,

    /// CPU clock speed in MHz (default: 4.77 for original 8086)
    #[arg(long, default_value = "4.77")]
    pub speed: f64,
}

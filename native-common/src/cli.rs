use clap::Parser;

#[derive(Parser)]
pub struct CommonCli {

    /// Path to disk image file for floppy B:
    #[arg(long = "floppy-b")]
    pub floppy_b: Option<String>,

    /// Path to hard disk image file(s) - can be specified multiple times for C:, D:, etc.
    #[arg(long = "hdd", action = clap::ArgAction::Append)]
    pub hard_disks: Vec<String>,

    /// Mount host directory as DOS drive (format: /path:E: or /path/to/dir:D:)
    #[arg(long = "mount-dir", action = clap::ArgAction::Append)]
    pub mount_dirs: Vec<String>,

    /// Device to attach to COM1 (e.g., "mouse", "logger")
    #[arg(long = "com1", value_name = "DEVICE")]
    pub com1_device: Option<String>,

    /// Device to attach to COM2 (e.g., "mouse", "logger")
    #[arg(long = "com2", value_name = "DEVICE")]
    pub com2_device: Option<String>,

    /// Enable interrupt logging (logs INT calls to oxide86.log)
    #[arg(long = "int-log")]
    pub int_log: bool,

    /// CPU clock speed in MHz (default: 8 for a standard 286)
    #[arg(long, default_value = "8")]
    pub speed: f64,

    /// Memory size in KB (default: 1024; conventional memory capped at 640 KB,
    /// extended memory = memory - 1024 KB on 286+ CPUs)
    #[arg(long, default_value = "1024", value_name = "KB")]
    pub memory: u32,

    /// Video card type to emulate (cga, ega, vga)
    #[arg(long = "video-card", default_value = "vga")]
    pub video_card: String,

    /// Enable joystick A (port 0x201)
    #[arg(long = "joystick-a")]
    pub joystick_a: bool,

    /// Enable joystick B (port 0x201)
    #[arg(long = "joystick-b")]
    pub joystick_b: bool,

    /// Disable PC speaker / audio output
    #[arg(long = "disable-pc-speaker")]
    pub disable_pc_speaker: bool,

    /// Sound card to emulate (none, adlib)
    #[arg(long = "sound-card", default_value = "adlib")]
    pub sound_card: String,

    /// CD-ROM ISO image(s) - can be specified multiple times (up to 4 slots)
    #[arg(long = "cdrom", action = clap::ArgAction::Append)]
    pub cdroms: Vec<String>,
}

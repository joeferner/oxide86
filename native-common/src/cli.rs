use clap::Parser;

#[derive(Parser)]
pub struct CommonCli {
    /// Mount host directory as DOS drive (format: /path:E: or /path/to/dir:D:)
    #[arg(long = "mount-dir", action = clap::ArgAction::Append)]
    pub mount_dirs: Vec<String>,

    /// Enable interrupt logging (logs INT calls to oxide86.log)
    #[arg(long = "int-log")]
    pub int_log: bool,

    /// Enable joystick A (port 0x201)
    #[arg(long = "joystick-a")]
    pub joystick_a: bool,

    /// Enable joystick B (port 0x201)
    #[arg(long = "joystick-b")]
    pub joystick_b: bool,

    /// Sound card to emulate (none, adlib)
    #[arg(long = "sound-card", default_value = "adlib")]
    pub sound_card: String,

    /// CD-ROM ISO image(s) - can be specified multiple times (up to 4 slots)
    #[arg(long = "cdrom", action = clap::ArgAction::Append)]
    pub cdroms: Vec<String>,
}

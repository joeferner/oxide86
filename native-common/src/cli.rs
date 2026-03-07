use clap::Parser;

#[derive(Parser)]
pub struct CommonCli {
    /// Path to the program binary to load and execute
    #[arg()]
    pub program: Option<String>,

    /// Starting segment address (default: 0x1000)
    #[arg(long, default_value = "0x1000")]
    pub segment: String,

    /// Starting offset address (default: 0x0100, like .COM files)
    #[arg(long, default_value = "0x0100")]
    pub offset: String,

    /// CPU type to emulate (8086, 286, 386, 486)
    #[arg(long = "cpu", default_value = "286")]
    pub cpu_type: String,

    /// CPU clock speed in MHz (default: 8 for a standard 286)
    #[arg(long, default_value = "8")]
    pub speed: f64,

    /// Memory size in KB (default: 1024; conventional memory capped at 640 KB,
    /// extended memory = memory - 1024 KB on 286+ CPUs)
    #[arg(long, default_value = "2048", value_name = "KB")]
    pub memory: String,

    /// Video card type to emulate (cga, ega, vga)
    #[arg(long = "video-card", default_value = "vga")]
    pub video_card: String,

    /// Enable execution logging (logs each instruction to oxide86.log)
    #[arg(long = "exec-log")]
    pub exec_log: bool,

    /// Boot drive number (0x00 for floppy A:, 0x01 for floppy B:, 0x80 for hard disk C:)
    #[arg(long, default_value = "0x00")]
    pub boot_drive: String,

    /// Path to disk image file for floppy A: (append :r for read-only, e.g. disk.img:r)
    #[arg(long = "floppy-a")]
    pub floppy_a: Option<String>,

    /// Path to disk image file for floppy B: (append :r for read-only, e.g. disk.img:r)
    #[arg(long = "floppy-b")]
    pub floppy_b: Option<String>,

    /// Path to hard disk image file(s) - can be specified multiple times for C:, D:, etc.
    #[arg(long = "hdd", action = clap::ArgAction::Append)]
    pub hard_disks: Vec<String>,

    /// Disable PC speaker / audio output
    #[arg(long = "disable-pc-speaker")]
    pub disable_pc_speaker: bool,

    /// Route host mouse events to the PS/2 auxiliary port (INT 15h AH=C2h / IRQ12)
    /// instead of a serial mouse on a COM port
    #[arg(long = "ps2-mouse")]
    pub ps2_mouse: bool,

    /// Device to attach to COM1 (e.g., "mouse")
    #[arg(long = "com1", value_name = "DEVICE")]
    pub com1_device: Option<String>,

    /// Device to attach to COM2 (e.g., "mouse")
    #[arg(long = "com2", value_name = "DEVICE")]
    pub com2_device: Option<String>,

    /// Device to attach to COM3 (e.g., "mouse")
    #[arg(long = "com3", value_name = "DEVICE")]
    pub com3_device: Option<String>,

    /// Device to attach to COM4 (e.g., "mouse")
    #[arg(long = "com4", value_name = "DEVICE")]
    pub com4_device: Option<String>,
}

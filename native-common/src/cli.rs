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

    /// Sound card type to emulate (none, adlib, sb16)
    #[arg(long = "sound-card", default_value = "sb16")]
    pub sound_card: String,

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

    /// Device to attach to COM1 (e.g., "mouse", "loopback")
    #[arg(long = "com1", value_name = "DEVICE")]
    pub com1_device: Option<String>,

    /// Device to attach to COM2 (e.g., "mouse", "loopback")
    #[arg(long = "com2", value_name = "DEVICE")]
    pub com2_device: Option<String>,

    /// Device to attach to COM3 (e.g., "mouse", "loopback")
    #[arg(long = "com3", value_name = "DEVICE")]
    pub com3_device: Option<String>,

    /// Device to attach to COM4 (e.g., "mouse", "loopback")
    #[arg(long = "com4", value_name = "DEVICE")]
    pub com4_device: Option<String>,

    /// Device to attach to LPT1 (e.g., "printer", "loopback")
    #[arg(long = "lpt1", value_name = "DEVICE")]
    pub lpt1_device: Option<String>,

    /// File to write raw LPT1 printer output to (e.g. lpt1.prn). Only used when --lpt1 printer is set.
    #[arg(long = "lpt1-output", value_name = "FILE")]
    pub lpt1_output: Option<String>,

    /// Device to attach to LPT2 (e.g., "printer", "loopback")
    #[arg(long = "lpt2", value_name = "DEVICE")]
    pub lpt2_device: Option<String>,

    /// File to write raw LPT2 printer output to (e.g. lpt2.prn). Only used when --lpt2 printer is set.
    #[arg(long = "lpt2-output", value_name = "FILE")]
    pub lpt2_output: Option<String>,

    /// Device to attach to LPT3 (e.g., "printer", "loopback")
    #[arg(long = "lpt3", value_name = "DEVICE")]
    pub lpt3_device: Option<String>,

    /// File to write raw LPT3 printer output to (e.g. lpt3.prn). Only used when --lpt3 printer is set.
    #[arg(long = "lpt3-output", value_name = "FILE")]
    pub lpt3_output: Option<String>,

    /// Enable joystick/gamepad input on the game port (0x201). Uses the first connected gamepad.
    #[arg(long = "joystick")]
    pub joystick: bool,

    /// Use the host wall-clock for RTC time instead of deriving it from CPU cycles.
    #[arg(long = "native-clock")]
    pub native_clock: bool,

    /// Disable the 8087 math coprocessor
    #[arg(long = "no-fpu")]
    pub no_fpu: bool,

    /// Physical memory addresses to watch for writes (hex, e.g. 0x4064E).
    /// Each write to a watched address is logged: [WATCH] 0xADDR written: 0xVAL by CS:IP
    /// Can be specified multiple times.
    #[arg(long = "watch", value_name = "ADDR", action = clap::ArgAction::Append)]
    pub watch: Vec<String>,

    /// Start the MCP debug server on the given TCP port (e.g. 7777).
    /// Allows Claude Code to inspect registers, memory, and set breakpoints.
    #[arg(long = "debug-mcp", value_name = "PORT")]
    pub debug_mcp_port: Option<u16>,

    /// Pause emulation immediately on start (requires --debug-mcp).
    #[arg(long = "debug-mcp-pause-on-start", requires = "debug_mcp_port")]
    pub debug_mcp_pause_on_start: bool,

    /// Sound Blaster DSP/Mixer/OPL base port (SB16 only)
    #[arg(
        long = "sound-blaster-port",
        value_name = "PORT",
        default_value = "0x220"
    )]
    pub sound_blaster_port: String,

    /// Sound Blaster CD-ROM interface base port (SB16 only)
    #[arg(
        long = "sound-blaster-cd-port",
        value_name = "PORT",
        default_value = "0x230"
    )]
    pub sound_blaster_cd_port: String,

    /// Disable the Sound Blaster CD-ROM interface
    #[arg(long = "disable-sound-blaster-cd")]
    pub disable_sound_blaster_cd: bool,

    /// Sound Blaster IRQ line (default: 5; valid: 2, 5, 7, 10)
    #[arg(long = "sound-blaster-irq", value_name = "IRQ", default_value = "5")]
    pub sound_blaster_irq: u8,

    /// Sound Blaster 8-bit DMA channel (default: 1; valid: 0, 1, 3)
    #[arg(
        long = "sound-blaster-dma8",
        value_name = "CHANNEL",
        default_value = "1"
    )]
    pub sound_blaster_dma8: u8,

    /// Sound Blaster 16-bit DMA channel (default: 5; valid: 5, 6, 7)
    #[arg(
        long = "sound-blaster-dma16",
        value_name = "CHANNEL",
        default_value = "5"
    )]
    pub sound_blaster_dma16: u8,

    /// ISO image to mount as CD-ROM at startup
    #[arg(long = "cdrom", value_name = "FILE")]
    pub cdrom: Option<std::path::PathBuf>,
}

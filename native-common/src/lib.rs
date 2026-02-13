mod cli;
mod clock;
mod setup;

pub use cli::CommonCli;
pub use clock::NativeClock;
pub use setup::{
    apply_logging_flags, attach_serial_device, create_speaker, load_disks, load_program_or_boot,
};

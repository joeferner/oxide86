use std::fmt::{Debug, Display};

pub use crate::audio::adlib::{ADLIB_SAMPLE_RATE, Adlib};
pub use crate::audio::{
    NullSoundCard, SoundCard, SoundCardType,
    speaker::{NullSpeaker, SpeakerOutput},
};
pub use crate::bus::Bus;
pub use crate::cdrom::{CD_SECTOR_SIZE, CdRomImage};
pub use crate::clock::{Clock, LocalDate, LocalTime};
pub use crate::cpu::bios::{Bios, DosDevice, DriveParams, KeyPress, SharedBiosState};
pub use crate::cpu_type::CpuType;
pub use crate::decoder::{DecodedInstruction, decode_instruction, decode_instruction_with_regs};
pub use crate::disk::{
    BackedDisk, DiskBackend, DiskController, DiskGeometry, DiskImage, MemoryDiskBackend,
    PartitionedDisk, SECTOR_SIZE, create_formatted_disk, parse_mbr,
};
pub use crate::drive_manager::{DiskAdapter, DriveManager};
pub use crate::io::Pit;
pub use crate::joystick::{JoystickInput, JoystickState, NullJoystick};
pub use crate::keyboard::KeyboardInput;
pub use crate::memory::{MEMORY_SIZE, Memory};
pub use crate::memory_allocator::MemoryAllocator;
pub use crate::mouse::{MouseInput, MouseState, NullMouse};
pub use crate::serial_logger::SerialLogger;
pub use crate::serial_mouse::SerialMouse;
pub use crate::serial_port::{SerialDevice, SerialParams, SerialPortController, SerialStatus};
pub use crate::video::{
    CGA_MEMORY_END, CGA_MEMORY_SIZE, CGA_MEMORY_START, CursorPosition, NullVideoController, Video,
    VideoController, VideoMode, colors,
};
pub use crate::video_card_type::VideoCardType;
pub use computer::{Computer, ComputerConfig};
pub use font::Cp437Font;
pub use palette::TextModePalette;

pub mod audio;
pub mod bus;
pub mod cdrom;
pub mod clock;
pub mod computer;
pub mod cpu;
pub mod cpu_type;
pub mod decoder;
pub mod disk;
pub mod drive_manager;
pub mod font;
pub mod io;
pub mod joystick;
pub mod keyboard;
pub mod memory;
pub mod memory_allocator;
pub mod mouse;
pub mod palette;
pub mod peripheral;
pub mod serial_logger;
pub mod serial_mouse;
pub mod serial_port;
pub mod utils;
pub mod video;
pub mod video_card_type;

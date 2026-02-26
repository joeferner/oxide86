pub mod video_buffer;
pub mod video_card;

pub const MAX_VIDEO_WIDTH: usize = 800;
pub const MAX_VIDEO_HEIGHT: usize = 600;

// CGA video memory constants
pub const CGA_MEMORY_START: usize = 0xB8000;
pub const CGA_MEMORY_END: usize = 0xBFFFF;
pub const CGA_MEMORY_SIZE: usize = CGA_MEMORY_END - CGA_MEMORY_START + 1; // 32KB

pub use video_buffer::VideoBuffer;
pub use video_card::VideoCard;

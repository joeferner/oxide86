pub mod computer;
pub mod cpu;
pub mod logging;
pub mod memory;
pub mod memory_bus;
pub mod video;

// Calculate physical address from segment:offset
pub fn physical_address(segment: u16, offset: u16) -> usize {
    ((segment as usize) << 4) + (offset as usize)
}

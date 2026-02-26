pub mod computer;
pub mod cpu;
pub mod logging;
pub mod memory;

// Calculate physical address from segment:offset
pub fn physical_address(segment: u16, offset: u16) -> usize {
    ((segment as usize) << 4) + (offset as usize)
}

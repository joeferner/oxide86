pub mod computer;
pub mod cpu;
pub mod memory;
pub mod memory_bus;
pub mod video;

#[cfg(test)]
pub mod tests;

// Calculate physical address from segment:offset
pub fn physical_address(segment: u16, offset: u16) -> usize {
    ((segment as usize) << 4) + (offset as usize)
}

pub trait Device {
    fn read_u8(&self, addr: usize) -> Option<u8>;
    fn write_u8(&mut self, addr: usize, val: u8) -> bool;
}

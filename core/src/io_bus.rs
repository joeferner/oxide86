pub struct IoBus {}

impl IoBus {
    pub fn new() -> Self {
        Self {}
    }

    pub fn write_u8(&mut self, addr: u16, val: u8) {
        todo!("IoBus write_u8 addr: {addr}, val: {val}");
    }
}

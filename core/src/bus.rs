use std::{
    any::Any,
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use anyhow::Result;

use crate::{
    Device, DeviceRef,
    cpu::bios::bios_reset,
    devices::{keyboard_controller::KeyboardController, pic::PIC, pit::PIT},
    disk::{DiskController, DriveNumber},
    memory::Memory,
    video::VideoCard,
};

const MEMORY_MAPPED_IO_START: usize = 0xA0000;
const MEMORY_MAPPED_IO_END: usize = 0xF0000;

pub struct Bus {
    memory: Memory,
    devices: Vec<DeviceRef>,
    disk_controllers: Vec<Rc<RefCell<DiskController>>>,
    pic: Rc<RefCell<PIC>>,
    pit: Rc<RefCell<PIT>>,
    keyboard_controller: Rc<RefCell<KeyboardController>>,
    video_card: Option<Rc<RefCell<VideoCard>>>,

    /// Cycle count to accurately track CPU cycles
    cycle_count: u32,
}

impl Bus {
    pub fn new(memory: Memory) -> Self {
        let keyboard_controller = Rc::new(RefCell::new(KeyboardController::new()));
        let pit = Rc::new(RefCell::new(PIT::new()));
        let pic = Rc::new(RefCell::new(PIC::new(
            pit.clone(),
            keyboard_controller.clone(),
        )));
        Self {
            memory,
            devices: vec![pic.clone(), pit.clone(), keyboard_controller.clone()],
            disk_controllers: vec![],
            pic,
            pit,
            keyboard_controller,
            video_card: None,
            cycle_count: 0,
        }
    }

    pub fn increment_cycle_count(&mut self, cycles: u32) {
        self.cycle_count = self.cycle_count.wrapping_add(cycles);
    }

    pub fn cycle_count(&self) -> u32 {
        self.cycle_count
    }

    pub fn pic(&self) -> Ref<'_, PIC> {
        self.pic.borrow()
    }

    pub fn pic_mut(&self) -> RefMut<'_, PIC> {
        self.pic.borrow_mut()
    }

    pub fn pit(&self) -> Ref<'_, PIT> {
        self.pit.borrow()
    }

    pub fn pit_mut(&self) -> RefMut<'_, PIT> {
        self.pit.borrow_mut()
    }

    pub fn keyboard_controller(&self) -> Ref<'_, KeyboardController> {
        self.keyboard_controller.borrow()
    }

    pub fn keyboard_controller_mut(&self) -> RefMut<'_, KeyboardController> {
        self.keyboard_controller.borrow_mut()
    }

    pub fn video_card_mut(&self) -> Option<RefMut<'_, VideoCard>> {
        self.video_card
            .as_ref()
            .map(|video_card| video_card.borrow_mut())
    }

    pub fn add_device<T: Device + 'static>(&mut self, device: T) {
        let rc = Rc::new(RefCell::new(device));

        let rc_any: Rc<dyn Any> = rc.clone();
        if let Ok(dc) = Rc::downcast::<RefCell<DiskController>>(rc_any) {
            self.disk_controllers.push(dc);
        }

        let rc_any: Rc<dyn Any> = rc.clone();
        if let Ok(dc) = Rc::downcast::<RefCell<VideoCard>>(rc_any) {
            if self.video_card.is_some() {
                panic!("video card already added");
            }
            self.video_card = Some(dc);
        }

        self.devices.push(rc);
    }

    pub fn find_disk_controller(&self, drive: DriveNumber) -> Option<Rc<RefCell<DiskController>>> {
        self.disk_controllers
            .iter()
            .find(|c| c.borrow().drive_number() == drive)
            .cloned()
    }

    pub fn memory_read_u8(&self, addr: usize) -> u8 {
        if (MEMORY_MAPPED_IO_START..MEMORY_MAPPED_IO_END).contains(&addr) {
            for device in &self.devices {
                if let Some(val) = device.borrow().memory_read_u8(addr) {
                    return val;
                }
            }
        }

        self.memory.read_u8(addr)
    }

    pub fn memory_write_u8(&mut self, addr: usize, val: u8) {
        if (MEMORY_MAPPED_IO_START..MEMORY_MAPPED_IO_END).contains(&addr) {
            for device in &self.devices {
                if device.borrow_mut().memory_write_u8(addr, val) {
                    return;
                }
            }
        }

        self.memory.write_u8(addr, val);
    }

    /// Read a 16-bit word (little-endian)
    pub fn memory_read_u16(&self, address: usize) -> u16 {
        let low = self.memory_read_u8(address) as u16;
        let high = self.memory_read_u8(address + 1) as u16;
        (high << 8) | low
    }

    /// Write a 16-bit word (little-endian)
    pub fn memory_write_u16(&mut self, addr: usize, val: u16) {
        self.memory_write_u8(addr, (val & 0xFF) as u8);
        self.memory_write_u8(addr + 1, (val >> 8) as u8);
    }

    /// Read 32-bit dword from memory or memory-mapped device
    pub fn memory_read_u32(&self, address: usize) -> u32 {
        let w1 = self.memory_read_u16(address) as u32;
        let w2 = self.memory_read_u16(address + 2) as u32;
        (w2 << 16) | w1
    }

    /// Write 32-bit dword to memory or memory-mapped device
    pub fn memory_write_u32(&mut self, address: usize, value: u32) {
        self.memory_write_u16(address, (value & 0xFFFF) as u16);
        self.memory_write_u16(address + 2, (value >> 16) as u16);
    }

    pub fn io_read_u8(&self, port: u16) -> u8 {
        for device in &self.devices {
            if let Some(val) = device.borrow().io_read_u8(port) {
                return val;
            }
        }

        log::warn!("No device responded to io read port: 0x{port:04X}");
        0xff
    }

    pub fn io_read_u16(&self, port: u16) -> u16 {
        todo!("IoBus read_u16 {port}");
    }

    pub fn io_write_u8(&mut self, port: u16, val: u8) {
        for device in &self.devices {
            if device.borrow_mut().io_write_u8(port, val) {
                return;
            }
        }

        log::warn!("No device responded to io write port: 0x{port:04X}, val: 0x{val:02X}");
    }

    /// Load binary data at a specific address
    pub fn load_at(&mut self, addr: usize, data: &[u8]) -> Result<()> {
        self.memory.load_at(addr, data)
    }

    pub fn reset(&mut self) {
        self.cycle_count = 0;
        bios_reset(self);
        for device in &self.devices {
            device.borrow_mut().reset();
        }
    }
}

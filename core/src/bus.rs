use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use anyhow::Result;

use crate::{
    Device, DeviceRef,
    cpu::bios::bios_reset,
    devices::{
        SoundCard, SoundCardRef,
        floppy_disk_controller::FloppyDiskController,
        game_port::GamePortDevice,
        hard_disk_controller::HardDiskController,
        keyboard_controller::KeyboardController,
        pc_speaker::PcSpeaker,
        pic::Pic,
        pit::Pit,
        rtc::{Clock, Rtc},
        uart::Uart,
    },
    disk::Disk,
    memory::Memory,
    video::VideoCard,
    wrapping_ge,
};

const MEMORY_MAPPED_IO_START: usize = 0xA0000;
const MEMORY_MAPPED_IO_END: usize = 0xF0000;

pub(crate) struct BusConfig {
    pub memory: Memory,
    pub cpu_clock_speed: u32,
    pub clock: Option<Box<dyn Clock>>,
    pub hard_disks: Vec<Box<dyn Disk>>,
    pub video_card: Rc<RefCell<VideoCard>>,
    pub pc_speaker: Box<dyn PcSpeaker>,
}

pub(crate) struct Bus {
    memory: Memory,
    devices: Vec<DeviceRef>,
    floppy_controller: Rc<RefCell<FloppyDiskController>>,
    hard_disk_controller: Rc<RefCell<HardDiskController>>,
    game_port: Rc<RefCell<GamePortDevice>>,
    pic: Rc<RefCell<Pic>>,
    keyboard_controller: Rc<RefCell<KeyboardController>>,
    uart: Rc<RefCell<Uart>>,
    rtc: Option<Rc<RefCell<Rtc>>>,
    video_card: Rc<RefCell<VideoCard>>,
    sound_card: Option<SoundCardRef>,

    /// Cycle count to accurately track CPU cycles
    cycle_count: u32,
}

impl Bus {
    pub(crate) fn new(config: BusConfig) -> Self {
        let keyboard_controller = Rc::new(RefCell::new(KeyboardController::new()));
        let pit = Rc::new(RefCell::new(Pit::new(
            config.cpu_clock_speed,
            config.pc_speaker,
        )));
        let uart = Rc::new(RefCell::new(Uart::new()));
        let pic = Rc::new(RefCell::new(Pic::new(
            pit.clone(),
            keyboard_controller.clone(),
            uart.clone(),
        )));
        let floppy_controller = Rc::new(RefCell::new(FloppyDiskController::new()));
        let hard_disk_controller =
            Rc::new(RefCell::new(HardDiskController::new(config.hard_disks)));
        let game_port = Rc::new(RefCell::new(GamePortDevice::new(config.cpu_clock_speed)));
        let mut devices: Vec<DeviceRef> = vec![
            pic.clone(),
            pit,
            keyboard_controller.clone(),
            uart.clone(),
            floppy_controller.clone(),
            hard_disk_controller.clone(),
            config.video_card.clone(),
            game_port.clone(),
        ];
        let rtc = if let Some(clock) = config.clock {
            let rtc = Rc::new(RefCell::new(Rtc::new(clock)));
            devices.push(rtc.clone());
            Some(rtc)
        } else {
            None
        };
        Self {
            memory: config.memory,
            devices,
            floppy_controller,
            hard_disk_controller,
            game_port,
            pic,
            keyboard_controller,
            uart,
            video_card: config.video_card,
            sound_card: None,
            cycle_count: 0,
            rtc,
        }
    }

    pub(crate) fn has_rtc(&self) -> bool {
        self.rtc.is_some()
    }

    pub(crate) fn rtc(&self) -> Option<Ref<'_, Rtc>> {
        self.rtc.as_ref().map(|rtc| rtc.borrow())
    }

    pub(crate) fn increment_cycle_count(&mut self, cycles: u32) {
        self.cycle_count = self.cycle_count.wrapping_add(cycles);
        if let Some(sc) = &self.sound_card
            && wrapping_ge(self.cycle_count, sc.borrow().next_sample_cycle())
        {
            sc.borrow_mut().advance_to_cycle(self.cycle_count);
        }
    }

    pub(crate) fn cycle_count(&self) -> u32 {
        self.cycle_count
    }

    pub(crate) fn pic_mut(&self) -> RefMut<'_, Pic> {
        self.pic.borrow_mut()
    }

    /// Notify the PIC that a non-PIT device has a pending IRQ, so it will be
    /// checked on the next CPU step rather than after the polling interval.
    pub(crate) fn notify_irq_pending(&self) {
        self.pic.borrow_mut().notify_pending();
    }

    pub(crate) fn uart_mut(&self) -> RefMut<'_, Uart> {
        self.uart.borrow_mut()
    }

    pub(crate) fn is_keyboard_irq_in_service(&self) -> bool {
        self.pic.borrow().is_keyboard_irq_in_service()
    }

    pub(crate) fn keyboard_controller(&self) -> Ref<'_, KeyboardController> {
        self.keyboard_controller.borrow()
    }

    pub(crate) fn keyboard_controller_mut(&self) -> RefMut<'_, KeyboardController> {
        self.keyboard_controller.borrow_mut()
    }

    pub(crate) fn video_card(&self) -> Ref<'_, VideoCard> {
        self.video_card.borrow()
    }

    pub(crate) fn video_card_mut(&self) -> RefMut<'_, VideoCard> {
        self.video_card.borrow_mut()
    }

    pub(crate) fn hard_disk_controller(&self) -> Ref<'_, HardDiskController> {
        self.hard_disk_controller.borrow()
    }

    pub(crate) fn floppy_controller(&self) -> Ref<'_, FloppyDiskController> {
        self.floppy_controller.borrow()
    }

    pub(crate) fn floppy_controller_mut(&self) -> RefMut<'_, FloppyDiskController> {
        self.floppy_controller.borrow_mut()
    }

    pub(crate) fn game_port_mut(&self) -> RefMut<'_, GamePortDevice> {
        self.game_port.borrow_mut()
    }

    pub(crate) fn add_device<T: Device + 'static>(&mut self, device: T) {
        let rc = Rc::new(RefCell::new(device));
        self.devices.push(rc);
    }

    pub(crate) fn add_sound_card<T: Device + SoundCard + 'static>(&mut self, device: T) {
        let rc = Rc::new(RefCell::new(device));
        self.devices.push(rc.clone());
        self.sound_card = Some(rc);
    }

    pub(crate) fn memory_read_u8(&self, addr: usize) -> u8 {
        if (MEMORY_MAPPED_IO_START..MEMORY_MAPPED_IO_END).contains(&addr) {
            for device in &self.devices {
                if let Some(val) = device.borrow_mut().memory_read_u8(addr, self.cycle_count) {
                    return val;
                }
            }
        }

        self.memory.read_u8(addr)
    }

    pub(crate) fn memory_write_u8(&mut self, addr: usize, val: u8) {
        let cycle_count = self.cycle_count;
        if (MEMORY_MAPPED_IO_START..MEMORY_MAPPED_IO_END).contains(&addr) {
            for device in &self.devices {
                if device.borrow_mut().memory_write_u8(addr, val, cycle_count) {
                    return;
                }
            }
        }

        self.memory.write_u8(addr, val);
    }

    /// Read a 16-bit word (little-endian)
    pub(crate) fn memory_read_u16(&self, address: usize) -> u16 {
        let low = self.memory_read_u8(address) as u16;
        let high = self.memory_read_u8(address + 1) as u16;
        (high << 8) | low
    }

    /// Write a 16-bit word (little-endian)
    pub(crate) fn memory_write_u16(&mut self, addr: usize, val: u16) {
        self.memory_write_u8(addr, (val & 0xFF) as u8);
        self.memory_write_u8(addr + 1, (val >> 8) as u8);
    }

    /// Write 32-bit dword to memory or memory-mapped device
    pub(crate) fn memory_write_u32(&mut self, address: usize, value: u32) {
        self.memory_write_u16(address, (value & 0xFFFF) as u16);
        self.memory_write_u16(address + 2, (value >> 16) as u16);
    }

    pub(crate) fn io_read_u8(&self, port: u16) -> u8 {
        for device in &self.devices {
            if let Some(val) = device.borrow_mut().io_read_u8(port, self.cycle_count) {
                return val;
            }
        }

        if let Some(hint) = unimplemented_port_hint(port) {
            log::info!("Unhandled io read port: 0x{port:04X} ({hint})");
        } else {
            log::warn!("No device responded to io read port: 0x{port:04X}");
        }
        0xff
    }

    pub(crate) fn io_read_u16(&self, port: u16) -> u16 {
        let lo = self.io_read_u8(port);
        let hi = self.io_read_u8(port + 1);
        u16::from_le_bytes([lo, hi])
    }

    pub(crate) fn io_write_u16(&mut self, port: u16, val: u16) {
        let [lo, hi] = val.to_le_bytes();
        self.io_write_u8(port, lo);
        self.io_write_u8(port + 1, hi);
    }

    pub(crate) fn io_write_u8(&mut self, port: u16, val: u8) {
        let cycle_count = self.cycle_count;
        for device in &self.devices {
            if device.borrow_mut().io_write_u8(port, val, cycle_count) {
                return;
            }
        }

        if let Some(hint) = unimplemented_port_hint(port) {
            log::info!("Unhandled io write port: 0x{port:04X} val: 0x{val:02X} ({hint})");
        } else {
            log::warn!("No device responded to io write port: 0x{port:04X}, val: 0x{val:02X}");
        }
    }

    /// Load binary data at a specific address
    pub(crate) fn load_at(&mut self, addr: usize, data: &[u8]) -> Result<()> {
        self.memory.load_at(addr, data)
    }

    pub(crate) fn reset(&mut self) {
        self.cycle_count = 0;
        bios_reset(self);
        for device in &self.devices {
            device.borrow_mut().reset();
        }
    }

    /// Get extended memory size in KB
    pub(crate) fn extended_memory_kb(&self) -> u16 {
        self.memory.extended_memory_kb()
    }

    /// Encode a PS/2 mouse event as a 3-byte packet and queue it into the
    /// keyboard controller's auxiliary output buffer.  IRQ12 will fire on the
    /// next CPU step (if interrupts are enabled and the aux port is enabled).
    ///
    /// `buttons`: bit 0 = left, bit 1 = right, bit 2 = middle.
    pub(crate) fn push_ps2_mouse_event(&mut self, dx: i8, dy: i8, buttons: u8) {
        let sign_x: u8 = if dx < 0 { 0x10 } else { 0 };
        let sign_y: u8 = if dy < 0 { 0x20 } else { 0 };
        let byte0 = 0x08 | (buttons & 0x07) | sign_x | sign_y;
        let byte1 = dx as u8;
        let byte2 = dy as u8;
        self.keyboard_controller
            .borrow_mut()
            .push_mouse_bytes(&[byte0, byte1, byte2]);
        self.notify_irq_pending();
    }
}

impl crate::ByteReader for Bus {
    fn read_u8(&self, addr: usize) -> u8 {
        self.memory_read_u8(addr)
    }

    fn read_u16(&self, addr: usize) -> u16 {
        self.memory_read_u16(addr)
    }
}

/// Returns a hint string for ports that are known to be probed but not implemented,
/// so they can be logged at a lower level than truly unrecognised ports.
fn unimplemented_port_hint(port: u16) -> Option<&'static str> {
    match port {
        0x02F2..=0x02F7 => Some("possible secondary FDC/serial probe"),
        0x06F0..=0x06FF => Some("DOS 4.01 hardware probe"),
        0x31A0..=0x31AF => Some("unknown ISA card detection probe"),
        _ => None,
    }
}

use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    sync::Arc,
    sync::atomic::Ordering,
};

use crate::debugger::DebugShared;
use anyhow::Result;

use crate::{
    Device, DeviceRef,
    cpu::{CpuType, bios::bios_reset},
    devices::{
        CdromController, CdromControllerRef, SoundCard, SoundCardRef,
        clock::Clock,
        dma::{DmaController, DmaTransfer},
        floppy_disk_controller::FloppyDiskController,
        game_port::GamePortDevice,
        hard_disk_controller::HardDiskController,
        keyboard_controller::KeyboardController,
        parallel_port::ParallelPort,
        pc_speaker::PcSpeaker,
        pic::Pic,
        pit::Pit,
        rtc::Rtc,
        uart::Uart,
    },
    disk::Disk,
    memory::Memory,
    video::VideoCard,
    wrapping_ge,
};

const MEMORY_MAPPED_IO_START: usize = 0xA0000;
const MEMORY_MAPPED_IO_END: usize = 0xF0000;

/// System Control Port A — Fast A20 gate and system reset (PS/2 systems).
/// Bit 1 = A20 gate (1 = enabled); bit 0 = system reset (pulse to reset).
const PORT_SYSTEM_CONTROL_A: u16 = 0x0092;

pub(crate) struct BusConfig {
    pub memory: Memory,
    pub cpu_type: CpuType,
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
    parallel_port: Rc<RefCell<ParallelPort>>,
    rtc: Option<Rc<RefCell<Rtc>>>,
    video_card: Rc<RefCell<VideoCard>>,
    sound_card: Option<SoundCardRef>,
    cdrom_controller: Option<CdromControllerRef>,
    dma: Rc<RefCell<DmaController>>,
    /// Devices connected to DMA channels, indexed by global channel number (0–7).
    /// The Bus calls `dma_read_u8` / `dma_write_u8` on these when executing transfers.
    dma_devices: [Option<DeviceRef>; 8],

    /// Cycle count to accurately track CPU cycles
    cycle_count: u32,

    /// A20 address line gate. When false, bit 20 of every memory address is
    /// masked out, causing the region 0x100000–0x10FFEF to alias 0x00000–0x0FFEF
    /// (classic 8086 wrap-around behaviour). Starts disabled, matching real
    /// hardware: the BIOS or OS (e.g. HIMEM.SYS) must explicitly enable it.
    a20_enabled: bool,

    /// Physical addresses to watch for writes. When any write hits one of these
    /// addresses, a [WATCH] log line is emitted with the value and CS:IP.
    watchpoints: Vec<usize>,

    /// CS:IP of the instruction currently executing (updated by the CPU before
    /// each instruction so watch hit logs can report the originating address).
    watch_cs: u16,
    watch_ip: u16,

    /// Optional debugger shared state. When set, write watchpoints are checked
    /// in memory_write_u8 and can trigger a pause.
    debug: Option<Arc<DebugShared>>,

    /// CPU reset requested by keyboard controller command 0xFE.
    reset_requested: bool,
}

impl Bus {
    pub(crate) fn new(config: BusConfig) -> Self {
        let keyboard_controller = Rc::new(RefCell::new(KeyboardController::new()));
        let pit = Rc::new(RefCell::new(Pit::new(
            config.cpu_clock_speed,
            config.pc_speaker,
        )));
        let uart = Rc::new(RefCell::new(Uart::new()));
        let floppy_controller = Rc::new(RefCell::new(FloppyDiskController::new()));
        let hard_disk_controller =
            Rc::new(RefCell::new(HardDiskController::new(config.hard_disks)));
        let game_port = Rc::new(RefCell::new(GamePortDevice::new(config.cpu_clock_speed)));
        let dma = Rc::new(RefCell::new(DmaController::new(config.cpu_type)));
        let parallel_port = Rc::new(RefCell::new(ParallelPort::new()));
        let rtc = config
            .clock
            .map(|clock| Rc::new(RefCell::new(Rtc::new(clock))));
        let pic = Rc::new(RefCell::new(Pic::new(
            pit.clone(),
            keyboard_controller.clone(),
            uart.clone(),
            floppy_controller.clone(),
            rtc.clone(),
        )));
        let mut devices: Vec<DeviceRef> = vec![
            pic.clone(),
            pit,
            keyboard_controller.clone(),
            uart.clone(),
            floppy_controller.clone(),
            hard_disk_controller.clone(),
            config.video_card.clone(),
            game_port.clone(),
            dma.clone(),
            parallel_port.clone(),
        ];
        if let Some(ref rtc) = rtc {
            devices.push(rtc.clone());
        }
        let mut dma_devices: [Option<DeviceRef>; 8] = Default::default();
        dma_devices[2] = Some(floppy_controller.clone());

        Self {
            memory: config.memory,
            devices,
            floppy_controller,
            hard_disk_controller,
            game_port,
            pic,
            keyboard_controller,
            uart,
            parallel_port,
            video_card: config.video_card,
            sound_card: None,
            cdrom_controller: None,
            dma,
            dma_devices,
            cycle_count: 0,
            rtc,
            a20_enabled: false,
            watchpoints: Vec::new(),
            watch_cs: 0,
            watch_ip: 0,
            debug: None,
            reset_requested: false,
        }
    }

    /// Check and clear the CPU reset request flag (set by keyboard controller 0xFE).
    pub(crate) fn take_reset_request(&mut self) -> bool {
        let r = self.reset_requested;
        self.reset_requested = false;
        r
    }

    pub(crate) fn set_debug(&mut self, debug: Arc<DebugShared>) {
        self.debug = Some(debug);
    }

    pub(crate) fn has_rtc(&self) -> bool {
        self.rtc.is_some()
    }

    pub(crate) fn rtc(&self) -> Option<Ref<'_, Rtc>> {
        self.rtc.as_ref().map(|rtc| rtc.borrow())
    }

    pub(crate) fn increment_cycle_count(&mut self, cycles: u32) {
        self.cycle_count = self.cycle_count.wrapping_add(cycles);
        let transfers = self.dma.borrow_mut().tick(self.cycle_count);
        for transfer in transfers {
            self.execute_dma_transfer(transfer);
        }
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

    pub(crate) fn parallel_port_mut(&self) -> RefMut<'_, ParallelPort> {
        self.parallel_port.borrow_mut()
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

    /// Assert a DMA request on the given global channel (0–7).
    #[allow(dead_code)]
    pub(crate) fn dma_request(&mut self, channel: u8) {
        self.dma.borrow_mut().set_dreq(channel, true);
    }

    /// Deassert a DMA request on the given global channel (0–7).
    #[allow(dead_code)]
    pub(crate) fn dma_release(&mut self, channel: u8) {
        self.dma.borrow_mut().set_dreq(channel, false);
    }

    /// Execute one DMA data transfer op produced by `DmaController::tick`.
    fn execute_dma_transfer(&mut self, transfer: DmaTransfer) {
        let ch = transfer.channel as usize;
        let device = self.dma_devices[ch].clone();
        if transfer.write_to_memory {
            // Device → memory: ask device for a byte, write it to memory.
            if let Some(dev) = device
                && let Some(byte) = dev.borrow_mut().dma_read_u8()
            {
                self.memory_write_u8(transfer.phys_addr as usize, byte);
            }
        } else {
            // Memory → device: read from memory, push byte to device.
            if let Some(dev) = device {
                let byte = self.memory_read_u8(transfer.phys_addr as usize);
                dev.borrow_mut().dma_write_u8(byte);
            }
        }
        // Drain any DREQ state change the FDC signalled during dma_read_u8
        // (e.g. deassert on last byte when FDC transitions to Result phase).
        if let Some(dreq) = self.floppy_controller.borrow_mut().take_dreq_request() {
            self.dma.borrow_mut().set_dreq(2, dreq);
            if !dreq {
                // DMA transfer complete — FDC has raised IRQ 6; fast-track the PIC scan.
                self.notify_irq_pending();
            }
        }
        // Channel 1 is the SB16 8-bit DMA channel; wake the PIC so it sees the IRQ
        // immediately after the block completes rather than waiting up to 100 instructions.
        if transfer.channel == 1 {
            self.notify_irq_pending();
        }
    }

    pub(crate) fn add_device<T: Device + 'static>(&mut self, device: T) {
        let rc = Rc::new(RefCell::new(device));
        self.devices.push(rc);
    }

    pub(crate) fn add_sound_card<T: Device + SoundCard + 'static>(&mut self, device: T) {
        debug_assert!(self.sound_card.is_none(), "sound card already registered");
        let rc = Rc::new(RefCell::new(device));
        self.devices.push(rc.clone());
        self.sound_card = Some(rc);
    }

    pub(crate) fn add_cdrom_controller<T: Device + CdromController + 'static>(
        &mut self,
        device: T,
    ) {
        debug_assert!(
            self.cdrom_controller.is_none(),
            "CD-ROM controller already registered"
        );
        let rc = Rc::new(RefCell::new(device));
        self.devices.push(rc.clone());
        self.cdrom_controller = Some(rc.clone());
        self.pic.borrow_mut().set_cdrom(rc);
    }

    pub(crate) fn add_sound_blaster<T: Device + SoundCard + CdromController + 'static>(
        &mut self,
        device: T,
    ) {
        debug_assert!(self.sound_card.is_none(), "sound card already registered");
        debug_assert!(
            self.cdrom_controller.is_none(),
            "CD-ROM controller already registered"
        );
        let rc = Rc::new(RefCell::new(device));
        self.devices.push(rc.clone());
        self.sound_card = Some(rc.clone());
        self.cdrom_controller = Some(rc.clone());
        // Channel 1: 8-bit DMA for SB16 PCM playback.
        self.dma_devices[1] = Some(rc.clone());
        self.pic.borrow_mut().set_cdrom(rc);
    }

    pub(crate) fn cdrom_controller(&self) -> Option<&CdromControllerRef> {
        self.cdrom_controller.as_ref()
    }

    pub(crate) fn memory_read_u8(&self, addr: usize) -> u8 {
        let addr = self.apply_a20(addr);
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
        let addr = self.apply_a20(addr);
        let cycle_count = self.cycle_count;
        if (MEMORY_MAPPED_IO_START..MEMORY_MAPPED_IO_END).contains(&addr) {
            for device in &self.devices {
                if device.borrow_mut().memory_write_u8(addr, val, cycle_count) {
                    return;
                }
            }
        }

        if !self.watchpoints.is_empty() && self.watchpoints.contains(&addr) {
            log::info!(
                "[WATCH] 0x{addr:05X} written: 0x{val:02X} by {:04X}:{:04X}",
                self.watch_cs,
                self.watch_ip,
            );
        }

        if let Some(ref dbg) = self.debug
            && dbg.has_write_watchpoints.load(Ordering::Relaxed)
        {
            let hit = dbg
                .write_watchpoints
                .lock()
                .unwrap()
                .contains(&(addr as u32));
            if hit {
                *dbg.watchpoint_hit.lock().unwrap() =
                    Some((addr as u32, val, self.watch_cs, self.watch_ip));
                dbg.pause_requested.store(true, Ordering::SeqCst);
            }
        }

        self.memory.write_u8(addr, val);
    }

    /// Set the physical addresses to watch for writes.
    pub(crate) fn set_watch_addresses(&mut self, addrs: Vec<usize>) {
        self.watchpoints = addrs;
    }

    /// Return any watched addresses that fall within [base, base+len).
    pub(crate) fn watchpoints_in_range(&self, base: usize, len: usize) -> Vec<usize> {
        self.watchpoints
            .iter()
            .copied()
            .filter(|&addr| addr >= base && addr < base + len)
            .collect()
    }

    /// Called by the CPU before executing each instruction so that watch hits
    /// can report the CS:IP of the responsible instruction.
    #[inline(always)]
    pub(crate) fn set_current_ip(&mut self, cs: u16, ip: u16) {
        self.watch_cs = cs;
        self.watch_ip = ip;
    }

    /// Read a 16-bit word (little-endian)
    pub(crate) fn memory_read_u16(&self, address: usize) -> u16 {
        let low = self.memory_read_u8(address) as u16;
        let high = self.memory_read_u8(address + 1) as u16;
        (high << 8) | low
    }

    /// Read a null-terminated C string from guest memory, stopping at NUL or 260 bytes.
    pub(crate) fn read_c_string(&self, addr: usize) -> String {
        let mut bytes = Vec::new();
        let mut a = addr;
        loop {
            let b = self.memory_read_u8(a);
            if b == 0 || bytes.len() >= 260 {
                break;
            }
            bytes.push(b);
            a += 1;
        }
        String::from_utf8_lossy(&bytes).into_owned()
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
        // Port 0x92: System Control Port A (Fast A20 gate).
        // Bit 1 = A20 gate state; bit 0 = system reset (always 0 on read).
        if port == PORT_SYSTEM_CONTROL_A {
            return if self.a20_enabled { 0x02 } else { 0x00 };
        }

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
        // Port 0x92: System Control Port A (Fast A20 gate).
        // Bit 1 = A20 gate; bit 0 = system reset pulse (ignored).
        if port == PORT_SYSTEM_CONTROL_A {
            self.set_a20_enabled((val & 0x02) != 0);
            return;
        }

        let cycle_count = self.cycle_count;
        for device in &self.devices {
            if device.borrow_mut().io_write_u8(port, val, cycle_count) {
                // After each device write, drain any A20 gate change from the keyboard controller.
                let a20_request = self.keyboard_controller.borrow_mut().take_a20_request();
                if let Some(enabled) = a20_request {
                    self.set_a20_enabled(enabled);
                }
                // Drain any CPU reset request from the keyboard controller.
                if self.keyboard_controller.borrow_mut().take_reset_request() {
                    self.reset_requested = true;
                }
                // Drain any DMA channel 2 request state change from the FDC.
                // This is how the FDC asserts DREQ when it enters DMA execution phase.
                if let Some(dreq) = self.floppy_controller.borrow_mut().take_dreq_request() {
                    self.dma.borrow_mut().set_dreq(2, dreq);
                }
                // Drain any DREQ state change from the sound card (e.g. SB16 DMA start/stop).
                if let Some(sc) = &self.sound_card
                    && let Some((channel, asserted)) = sc.borrow_mut().take_dreq_request()
                {
                    self.dma.borrow_mut().set_dreq(channel, asserted);
                }
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
        for device in &self.devices {
            device.borrow_mut().reset();
        }
        bios_reset(self);
    }

    /// Warm reset: like reset() but skips the video card so the screen is preserved.
    /// On real hardware, a KBC-triggered CPU reset does not reinitialize the CGA/EGA/VGA
    /// hardware — video memory and registers survive the warm restart.
    pub(crate) fn warm_reset(&mut self) {
        self.cycle_count = 0;
        for device in &self.devices {
            if device.borrow().as_any().type_id() == std::any::TypeId::of::<VideoCard>() {
                continue;
            }
            device.borrow_mut().reset();
        }
        bios_reset(self);
    }

    /// Get extended memory size in KB
    pub(crate) fn extended_memory_kb(&self) -> u16 {
        self.memory.extended_memory_kb()
    }

    pub(crate) fn a20_enabled(&self) -> bool {
        self.a20_enabled
    }

    pub(crate) fn set_a20_enabled(&mut self, enabled: bool) {
        if self.a20_enabled != enabled {
            log::debug!("A20 gate {}", if enabled { "enabled" } else { "disabled" });
            self.a20_enabled = enabled;
        }
    }

    /// Calculate physical address from segment:offset, applying A20 gate masking.
    /// When A20 is disabled, bit 20 is cleared, aliasing the HMA
    /// (0x100000–0x10FFEF) back to 0x000000–0x0FFEF.
    pub(crate) fn physical_address(&self, segment: u16, offset: u16) -> usize {
        let addr = ((segment as usize) << 4) + (offset as usize);
        self.apply_a20(addr)
    }

    /// Apply A20 gate masking to a physical address. When A20 is disabled, bit 20
    /// is cleared, aliasing the HMA (0x100000–0x10FFEF) back to 0x000000–0x0FFEF.
    #[inline(always)]
    pub(crate) fn apply_a20_pub(&self, addr: usize) -> usize {
        self.apply_a20(addr)
    }

    #[inline(always)]
    fn apply_a20(&self, addr: usize) -> usize {
        if self.a20_enabled {
            addr
        } else {
            addr & !(1 << 20)
        }
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
        0x0070 | 0x0071 => Some("CMOS RTC (not present on 8086)"),
        0x0050..=0x005F => Some("possible EMS/extended BIOS hardware probe"),
        0x0066 => Some("keyboard controller range probe"),
        0x02F2..=0x02F7 => Some("possible secondary FDC/serial probe"),
        0x06F0..=0x06FF => Some("DOS 4.01 hardware probe"),
        0x31A0..=0x31AF => Some("unknown ISA card detection probe"),
        _ => None,
    }
}

use std::any::Any;

use crate::{
    Device,
    devices::{CdromController, PcmRingBuffer, SoundCard},
    disk::cdrom::CdromBackend,
};

mod cdrom;
use cdrom::SoundBlasterCdromInner;
mod dsp;
use dsp::SoundBlasterDsp;
mod mixer;
use mixer::SoundBlasterMixer;
mod midi;
use midi::SoundBlasterMidi;
mod mpu;
use mpu::SoundBlasterMpu;
mod opl;
use opl::SoundBlasterOpl;

/// Which Sound Blaster model to emulate. Controls the DSP version reported by command 0xE1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundBlasterModel {
    /// SB 2.0 (DSP v2.01). Compatible with SB2-era installers and most DOS games.
    Sb2,
    /// SB Pro 2 (DSP v3.02). Adds stereo OPL2 and stereo PCM.
    SbPro,
    /// SB16 (DSP v4.05). Adds 16-bit DMA, SB16 mixer, and ASP.
    Sb16,
}

/// Sound Blaster emulation.
pub struct SoundBlaster {
    base_port: u16,
    cdrom: SoundBlasterCdromInner,
    dsp: SoundBlasterDsp,
    midi: SoundBlasterMidi,
    mixer: SoundBlasterMixer,
    mpu: SoundBlasterMpu,
    opl: SoundBlasterOpl,
    pcm_out: crate::devices::PcmRingBuffer,
    cpu_freq: u64,
    /// Last Direct DAC sample value (pending, not yet pushed to pcm_out).
    last_dac_sample: f32,
    /// Cycle count when the last Direct DAC sample arrived.
    last_dac_cycle: Option<u32>,
}

impl SoundBlaster {
    /// Create with default SB base 0x220, CD-ROM port 0x230, no disc, IRQ 5. Used by tests.
    pub fn new(model: SoundBlasterModel, cpu_freq: u64) -> Self {
        Self {
            base_port: 0x220,
            cdrom: SoundBlasterCdromInner::new(0x230, None, 5),
            dsp: SoundBlasterDsp::new(model),
            midi: SoundBlasterMidi::new(cpu_freq),
            mixer: SoundBlasterMixer::new(),
            mpu: SoundBlasterMpu::new(),
            opl: SoundBlasterOpl::new(cpu_freq),
            pcm_out: PcmRingBuffer::new_with_hold(44100 * 2, 44100),
            cpu_freq,
            last_dac_sample: 0.0,
            last_dac_cycle: None,
        }
    }

    /// Create with explicit CD-ROM configuration for native/CLI use.
    pub fn with_cdrom(
        model: SoundBlasterModel,
        cdrom_base_port: u16,
        disc: Option<Box<dyn CdromBackend>>,
        irq_line: u8,
        cpu_freq: u64,
    ) -> Self {
        Self {
            base_port: 0x220,
            cdrom: SoundBlasterCdromInner::new(cdrom_base_port, disc, irq_line),
            dsp: SoundBlasterDsp::new(model),
            midi: SoundBlasterMidi::new(cpu_freq),
            mixer: SoundBlasterMixer::new(),
            mpu: SoundBlasterMpu::new(),
            opl: SoundBlasterOpl::new(cpu_freq),
            pcm_out: PcmRingBuffer::new_with_hold(44100 * 2, 44100),
            cpu_freq,
            last_dac_sample: 0.0,
            last_dac_cycle: None,
        }
    }

    pub fn midi_consumer(&self) -> PcmRingBuffer {
        self.midi.consumer()
    }

    pub fn opl_consumer(&self) -> PcmRingBuffer {
        self.opl.consumer()
    }

    pub fn pcm_consumer(&self) -> PcmRingBuffer {
        self.pcm_out.clone()
    }
}

impl Device for SoundBlaster {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.cdrom.reset();
        self.dsp.hardware_reset();
        self.midi.reset();
        self.mixer.reset();
        self.mpu.reset();
        self.opl.reset();
        self.pcm_out.clear();
        self.last_dac_sample = 0.0;
        self.last_dac_cycle = None;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn dma_read_u8(&mut self) -> Option<u8> {
        // ADC (device→memory) transfer: tick DSP byte counter with silence so IRQ fires on block complete.
        self.dsp.dma_receive_byte(0x80);
        Some(0x80)
    }

    fn dma_write_u8(&mut self, val: u8) -> bool {
        let (sample, _done) = self.dsp.dma_receive_byte(val);
        // Upsample from DSP rate to 44100 Hz by repeating each sample.
        let dsp_rate = self.dsp.sample_rate();
        let factor = (44100 / dsp_rate).max(1) as usize;
        for _ in 0..factor {
            self.pcm_out.push_sample(sample);
        }
        true
    }

    fn io_read_u8(&mut self, port: u16, cycle_count: u32) -> Option<u8> {
        let sb_off = port.wrapping_sub(self.base_port);
        match sb_off {
            // OPL status (chip 0: base+0, base+1, base+8, base+9; chip 1: base+2, base+3)
            0x00 | 0x01 | 0x02 | 0x03 | 0x08 | 0x09 => {
                return Some(self.opl.read_status(cycle_count));
            }
            // Mixer ports
            0x04 => return Some(0xFF), // index port reads back 0xFF on real HW
            0x05 => {
                // Mixer reg 0x82 (IRQ status) reflects live DSP interrupt state.
                // Bit 0 = 8-bit DMA IRQ, bit 1 = 16-bit DMA IRQ.
                let val = if self.mixer.current_index() == 0x82 {
                    let mut v = self.mixer.read_data();
                    if self.dsp.irq_status_8 {
                        v |= 0x01;
                    }
                    if self.dsp.irq_status_16 {
                        v |= 0x02;
                    }
                    v
                } else {
                    self.mixer.read_data()
                };
                return Some(val);
            }
            // DSP reset port (write-only on HW); reading returns write-buffer-not-busy (0x7F)
            // so SBMIDI.EXE's presence/write-ready poll sees the hardware as available.
            0x06 => return Some(0x7F),
            // DSP ports
            0x0A => return Some(self.dsp.read_data()),
            0x0C => return Some(0x00), // write-buffer status: never busy
            0x0E => return Some(self.dsp.read_status()),
            0x0F => return Some(self.dsp.read_ack16()),
            _ => {}
        }
        // AdLib-compat OPL ports
        if let 0x388..=0x38B = port {
            return Some(self.opl.read_status(cycle_count));
        }
        // MPU-401 ports
        match port {
            0x330 => return Some(self.mpu.read_data()),
            0x331 => return Some(self.mpu.read_status()),
            _ => {}
        }
        self.cdrom.io_read_u8(port, cycle_count)
    }

    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool {
        let sb_off = port.wrapping_sub(self.base_port);
        match sb_off {
            // OPL address ports (chip 0: base+0, base+8)
            0x00 | 0x08 => {
                self.opl.write_address(0, val, cycle_count);
                return true;
            }
            // OPL data ports (chip 0: base+1, base+9)
            0x01 | 0x09 => {
                self.opl.write_data(0, val, cycle_count);
                return true;
            }
            // OPL address port (chip 1: base+2)
            0x02 => {
                self.opl.write_address(1, val, cycle_count);
                return true;
            }
            // OPL data port (chip 1: base+3)
            0x03 => {
                self.opl.write_data(1, val, cycle_count);
                return true;
            }
            // Mixer ports
            0x04 => {
                self.mixer.write_index(val);
                return true;
            }
            0x05 => {
                self.mixer.write_data(val);
                return true;
            }
            // DSP ports
            0x06 => {
                self.dsp.write_reset_port(val);
                return true;
            }
            0x0C => {
                self.dsp.write_command_port(val);
                if let Some(byte) = self.dsp.take_direct_dac_byte() {
                    let sample = (byte as f32 - 128.0) / 128.0;
                    // Resample: push previous sample for the elapsed time, then record new one.
                    let n_hold = if let Some(prev) = self.last_dac_cycle {
                        let delta = cycle_count.wrapping_sub(prev);
                        ((delta as u64 * 44100) / self.cpu_freq).max(1) as usize
                    } else {
                        1
                    };
                    for _ in 0..n_hold {
                        self.pcm_out.push_sample(self.last_dac_sample);
                    }
                    self.last_dac_sample = sample;
                    self.last_dac_cycle = Some(cycle_count);
                }
                return true;
            }
            _ => {}
        }
        // AdLib-compat OPL ports
        match port {
            0x388 => {
                self.opl.write_address(0, val, cycle_count);
                return true;
            }
            0x389 => {
                self.opl.write_data(0, val, cycle_count);
                return true;
            }
            0x38A => {
                self.opl.write_address(1, val, cycle_count);
                return true;
            }
            0x38B => {
                self.opl.write_data(1, val, cycle_count);
                return true;
            }
            // MPU-401 ports
            0x330 => {
                self.mpu.write_data(val);
                self.midi.push_byte(val);
                return true;
            }
            0x331 => {
                self.mpu.write_command(val);
                return true;
            }
            _ => {}
        }
        self.cdrom.io_write_u8(port, val, cycle_count)
    }
}

impl SoundCard for SoundBlaster {
    fn advance_to_cycle(&mut self, cycle_count: u32) {
        self.midi.advance_to_cycle(cycle_count);
        self.opl.advance_to_cycle(cycle_count);
    }

    fn next_sample_cycle(&self) -> u32 {
        self.opl.next_sample_cycle()
    }

    fn take_dreq_request(&mut self) -> Option<(u8, bool)> {
        let channel = if self.dsp.dma_16bit { 5u8 } else { 1u8 };
        self.dsp
            .take_dreq_request()
            .map(|asserted| (channel, asserted))
    }
}

impl CdromController for SoundBlaster {
    fn load_disc(&mut self, disc: Box<dyn CdromBackend>) {
        self.cdrom.load_disc(disc);
    }

    fn eject_disc(&mut self) {
        self.cdrom.eject_disc();
    }

    fn take_pending_irq(&mut self) -> bool {
        if self.dsp.take_pending_irq() {
            return true;
        }
        self.cdrom.take_pending_irq()
    }

    fn irq_line(&self) -> u8 {
        self.cdrom.irq_line()
    }
}

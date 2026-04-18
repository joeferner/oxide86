use crate::devices::{
    PcmRingBuffer,
    nuked_opl3::{self, Opl3Chip},
};

const OPL_SAMPLE_RATE: u32 = 44100;
const DEFAULT_CAPACITY: usize = OPL_SAMPLE_RATE as usize / 2;
const FLUSH_SIZE: usize = 128;

pub(super) struct SoundBlasterOpl {
    chip: Opl3Chip,
    pending_address: [u8; 2],
    timer1_value: u8,
    timer2_value: u8,
    timer_control: u8,
    timer1_counter: u32,
    timer2_counter: u32,
    status: u8,
    cycle_acc: u64,
    last_cycle_count: u32,
    next_sample_cycle: u32,
    pending_flush: Vec<f32>,
    cpu_freq: u64,
    timer1_cycles_per_tick: u32,
    timer2_cycles_per_tick: u32,
    consumer: PcmRingBuffer,
}

impl SoundBlasterOpl {
    pub(super) fn new(cpu_freq: u64) -> Self {
        let mut chip = Opl3Chip::default();
        nuked_opl3::reset(&mut chip, OPL_SAMPLE_RATE);
        let timer1_cycles_per_tick = (80e-6 * cpu_freq as f64).round() as u32;
        let timer2_cycles_per_tick = (320e-6 * cpu_freq as f64).round() as u32;
        Self {
            chip,
            pending_address: [0; 2],
            timer1_value: 0,
            timer2_value: 0,
            timer_control: 0,
            timer1_counter: 0,
            timer2_counter: 0,
            status: 0,
            cycle_acc: 0,
            last_cycle_count: 0,
            next_sample_cycle: (cpu_freq / OPL_SAMPLE_RATE as u64) as u32,
            pending_flush: Vec::with_capacity(FLUSH_SIZE * 2),
            cpu_freq,
            timer1_cycles_per_tick,
            timer2_cycles_per_tick,
            consumer: PcmRingBuffer::new(DEFAULT_CAPACITY, OPL_SAMPLE_RATE),
        }
    }

    pub(super) fn consumer(&self) -> PcmRingBuffer {
        self.consumer.clone()
    }

    pub(super) fn advance_to_cycle(&mut self, cycle_count: u32) {
        let elapsed = cycle_count.wrapping_sub(self.last_cycle_count) as u64;
        self.last_cycle_count = cycle_count;

        if elapsed > 0 {
            let cycles = elapsed as u32;
            if self.timer_control & 0x01 != 0 {
                self.timer1_counter += cycles;
                let ticks = (256 - self.timer1_value as u32).max(1);
                let threshold = ticks * self.timer1_cycles_per_tick;
                if self.timer1_counter >= threshold {
                    self.timer1_counter = 0;
                    if self.timer_control & 0x40 == 0 {
                        self.status |= 0xC0;
                    }
                }
            }
            if self.timer_control & 0x02 != 0 {
                self.timer2_counter += cycles;
                let ticks = (256 - self.timer2_value as u32).max(1);
                let threshold = ticks * self.timer2_cycles_per_tick;
                if self.timer2_counter >= threshold {
                    self.timer2_counter = 0;
                    if self.timer_control & 0x20 == 0 {
                        self.status |= 0xA0;
                    }
                }
            }
        }

        self.cycle_acc += elapsed * OPL_SAMPLE_RATE as u64;
        let n_out = self.cycle_acc / self.cpu_freq;
        self.cycle_acc %= self.cpu_freq;

        let cycles_until_next = (self.cpu_freq - self.cycle_acc).div_ceil(OPL_SAMPLE_RATE as u64);
        self.next_sample_cycle = self.last_cycle_count.wrapping_add(cycles_until_next as u32);

        for _ in 0..n_out {
            let mut buf = [0i16; 2];
            nuked_opl3::generate_resampled(&mut self.chip, &mut buf);
            let mono = (buf[0] as i32 + buf[1] as i32) / 2;
            self.pending_flush.push(mono as f32 / 32768.0);
        }

        if self.pending_flush.len() >= FLUSH_SIZE {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.pending_flush.is_empty() {
            return;
        }
        let mut buf = self.consumer.inner.lock().unwrap();
        for &s in &self.pending_flush {
            if buf.len() >= self.consumer.capacity {
                buf.pop_front();
            }
            buf.push_back(s);
        }
        drop(buf);
        self.pending_flush.clear();
    }

    pub(super) fn next_sample_cycle(&self) -> u32 {
        self.next_sample_cycle
    }

    pub(super) fn reset(&mut self) {
        self.pending_flush.clear();
        nuked_opl3::reset(&mut self.chip, OPL_SAMPLE_RATE);
        self.pending_address = [0; 2];
        self.timer1_value = 0;
        self.timer2_value = 0;
        self.timer_control = 0;
        self.timer1_counter = 0;
        self.timer2_counter = 0;
        self.status = 0;
        self.cycle_acc = 0;
        self.last_cycle_count = 0;
        self.next_sample_cycle = (self.cpu_freq / OPL_SAMPLE_RATE as u64) as u32;
        self.consumer.inner.lock().unwrap().clear();
    }

    pub(super) fn read_status(&mut self, cycle_count: u32) -> u8 {
        self.advance_to_cycle(cycle_count);
        self.status
    }

    pub(super) fn write_address(&mut self, chip: u8, addr: u8, cycle_count: u32) {
        self.advance_to_cycle(cycle_count);
        self.pending_address[chip as usize] = addr;
    }

    pub(super) fn write_data(&mut self, chip: u8, val: u8, cycle_count: u32) {
        self.advance_to_cycle(cycle_count);
        let addr = self.pending_address[chip as usize];
        if chip == 0 {
            match addr {
                0x02 => self.timer1_value = val,
                0x03 => self.timer2_value = val,
                0x04 => {
                    if val & 0x80 != 0 {
                        self.status = 0;
                    } else {
                        self.timer_control = val;
                        if val & 0x01 != 0 {
                            self.timer1_counter = 0;
                        }
                        if val & 0x02 != 0 {
                            self.timer2_counter = 0;
                        }
                    }
                }
                _ => {
                    nuked_opl3::write_reg(&mut self.chip, addr as u16, val);
                }
            }
        } else {
            nuked_opl3::write_reg(&mut self.chip, 0x100 | addr as u16, val);
        }
    }
}

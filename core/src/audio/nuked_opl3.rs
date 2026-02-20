// Nuked OPL3 — Rust port of opl3.c / opl3.h (version 1.8)
// Original C implementation by Nuke.YKT
// https://github.com/nukeykt/Nuked-OPL3
//
// This file is licensed under the GNU Lesser General Public License v2.1.
// See <https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html>.
//
// Ported to Rust for emu86 — structural changes only; all algorithms are
// faithful reproductions of the upstream C source.

// ============================================================
// ROM tables (literal transcription from opl3.c)
// ============================================================

/// Log-sine ROM (256 entries), extracted from OPL2 die shot.
pub static LOGSINROM: [u16; 256] = [
    0x859, 0x6c3, 0x607, 0x58b, 0x52e, 0x4e4, 0x4a6, 0x471, 0x443, 0x41a, 0x3f5, 0x3d3, 0x3b5,
    0x398, 0x37e, 0x365, 0x34e, 0x339, 0x324, 0x311, 0x2ff, 0x2ed, 0x2dc, 0x2cd, 0x2bd, 0x2af,
    0x2a0, 0x293, 0x286, 0x279, 0x26d, 0x261, 0x256, 0x24b, 0x240, 0x236, 0x22c, 0x222, 0x218,
    0x20f, 0x206, 0x1fd, 0x1f5, 0x1ec, 0x1e4, 0x1dc, 0x1d4, 0x1cd, 0x1c5, 0x1be, 0x1b7, 0x1b0,
    0x1a9, 0x1a2, 0x19b, 0x195, 0x18f, 0x188, 0x182, 0x17c, 0x177, 0x171, 0x16b, 0x166, 0x160,
    0x15b, 0x155, 0x150, 0x14b, 0x146, 0x141, 0x13c, 0x137, 0x133, 0x12e, 0x129, 0x125, 0x121,
    0x11c, 0x118, 0x114, 0x10f, 0x10b, 0x107, 0x103, 0x0ff, 0x0fb, 0x0f8, 0x0f4, 0x0f0, 0x0ec,
    0x0e9, 0x0e5, 0x0e2, 0x0de, 0x0db, 0x0d7, 0x0d4, 0x0d1, 0x0cd, 0x0ca, 0x0c7, 0x0c4, 0x0c1,
    0x0be, 0x0bb, 0x0b8, 0x0b5, 0x0b2, 0x0af, 0x0ac, 0x0a9, 0x0a7, 0x0a4, 0x0a1, 0x09f, 0x09c,
    0x099, 0x097, 0x094, 0x092, 0x08f, 0x08d, 0x08a, 0x088, 0x086, 0x083, 0x081, 0x07f, 0x07d,
    0x07a, 0x078, 0x076, 0x074, 0x072, 0x070, 0x06e, 0x06c, 0x06a, 0x068, 0x066, 0x064, 0x062,
    0x060, 0x05e, 0x05c, 0x05b, 0x059, 0x057, 0x055, 0x053, 0x052, 0x050, 0x04e, 0x04d, 0x04b,
    0x04a, 0x048, 0x046, 0x045, 0x043, 0x042, 0x040, 0x03f, 0x03e, 0x03c, 0x03b, 0x039, 0x038,
    0x037, 0x035, 0x034, 0x033, 0x031, 0x030, 0x02f, 0x02e, 0x02d, 0x02b, 0x02a, 0x029, 0x028,
    0x027, 0x026, 0x025, 0x024, 0x023, 0x022, 0x021, 0x020, 0x01f, 0x01e, 0x01d, 0x01c, 0x01b,
    0x01a, 0x019, 0x018, 0x017, 0x017, 0x016, 0x015, 0x014, 0x014, 0x013, 0x012, 0x011, 0x011,
    0x010, 0x00f, 0x00f, 0x00e, 0x00d, 0x00d, 0x00c, 0x00c, 0x00b, 0x00a, 0x00a, 0x009, 0x009,
    0x008, 0x008, 0x007, 0x007, 0x007, 0x006, 0x006, 0x005, 0x005, 0x005, 0x004, 0x004, 0x004,
    0x003, 0x003, 0x003, 0x002, 0x002, 0x002, 0x002, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001,
    0x001, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
];

/// Exponential ROM (256 entries), extracted from OPL2 die shot.
pub static EXPROM: [u16; 256] = [
    0x7fa, 0x7f5, 0x7ef, 0x7ea, 0x7e4, 0x7df, 0x7da, 0x7d4, 0x7cf, 0x7c9, 0x7c4, 0x7bf, 0x7b9,
    0x7b4, 0x7ae, 0x7a9, 0x7a4, 0x79f, 0x799, 0x794, 0x78f, 0x78a, 0x784, 0x77f, 0x77a, 0x775,
    0x770, 0x76a, 0x765, 0x760, 0x75b, 0x756, 0x751, 0x74c, 0x747, 0x742, 0x73d, 0x738, 0x733,
    0x72e, 0x729, 0x724, 0x71f, 0x71a, 0x715, 0x710, 0x70b, 0x706, 0x702, 0x6fd, 0x6f8, 0x6f3,
    0x6ee, 0x6e9, 0x6e5, 0x6e0, 0x6db, 0x6d6, 0x6d2, 0x6cd, 0x6c8, 0x6c4, 0x6bf, 0x6ba, 0x6b5,
    0x6b1, 0x6ac, 0x6a8, 0x6a3, 0x69e, 0x69a, 0x695, 0x691, 0x68c, 0x688, 0x683, 0x67f, 0x67a,
    0x676, 0x671, 0x66d, 0x668, 0x664, 0x65f, 0x65b, 0x657, 0x652, 0x64e, 0x649, 0x645, 0x641,
    0x63c, 0x638, 0x634, 0x630, 0x62b, 0x627, 0x623, 0x61e, 0x61a, 0x616, 0x612, 0x60e, 0x609,
    0x605, 0x601, 0x5fd, 0x5f9, 0x5f5, 0x5f0, 0x5ec, 0x5e8, 0x5e4, 0x5e0, 0x5dc, 0x5d8, 0x5d4,
    0x5d0, 0x5cc, 0x5c8, 0x5c4, 0x5c0, 0x5bc, 0x5b8, 0x5b4, 0x5b0, 0x5ac, 0x5a8, 0x5a4, 0x5a0,
    0x59c, 0x599, 0x595, 0x591, 0x58d, 0x589, 0x585, 0x581, 0x57e, 0x57a, 0x576, 0x572, 0x56f,
    0x56b, 0x567, 0x563, 0x560, 0x55c, 0x558, 0x554, 0x551, 0x54d, 0x549, 0x546, 0x542, 0x53e,
    0x53b, 0x537, 0x534, 0x530, 0x52c, 0x529, 0x525, 0x522, 0x51e, 0x51b, 0x517, 0x514, 0x510,
    0x50c, 0x509, 0x506, 0x502, 0x4ff, 0x4fb, 0x4f8, 0x4f4, 0x4f1, 0x4ed, 0x4ea, 0x4e7, 0x4e3,
    0x4e0, 0x4dc, 0x4d9, 0x4d6, 0x4d2, 0x4cf, 0x4cc, 0x4c8, 0x4c5, 0x4c2, 0x4be, 0x4bb, 0x4b8,
    0x4b5, 0x4b1, 0x4ae, 0x4ab, 0x4a8, 0x4a4, 0x4a1, 0x49e, 0x49b, 0x498, 0x494, 0x491, 0x48e,
    0x48b, 0x488, 0x485, 0x482, 0x47e, 0x47b, 0x478, 0x475, 0x472, 0x46f, 0x46c, 0x469, 0x466,
    0x463, 0x460, 0x45d, 0x45a, 0x457, 0x454, 0x451, 0x44e, 0x44b, 0x448, 0x445, 0x442, 0x43f,
    0x43c, 0x439, 0x436, 0x433, 0x430, 0x42d, 0x42a, 0x428, 0x425, 0x422, 0x41f, 0x41c, 0x419,
    0x416, 0x414, 0x411, 0x40e, 0x40b, 0x408, 0x406, 0x403, 0x400,
];

/// Frequency multiplier table (×2): 1/2,1,2,3,4,5,6,7,8,9,10,10,12,12,15,15
pub static MT: [u8; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

/// Key scale level ROM.
pub static KSLROM: [u8; 16] = [
    0, 32, 40, 45, 48, 51, 53, 55, 56, 58, 59, 60, 61, 62, 63, 64,
];

/// Key scale level shift amounts indexed by reg_ksl (0–3).
pub static KSLSHIFT: [u8; 4] = [8, 1, 2, 0];

/// Envelope increment step table [rate_lo][eg_timer_lo].
pub static EG_INCSTEP: [[u8; 4]; 4] = [[0, 0, 0, 0], [1, 0, 0, 0], [1, 0, 1, 0], [1, 1, 1, 0]];

/// Register address → slot index mapping (−1 = invalid address).
pub static AD_SLOT: [i8; 0x20] = [
    0, 1, 2, 3, 4, 5, -1, -1, 6, 7, 8, 9, 10, 11, -1, -1, 12, 13, 14, 15, 16, 17, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1,
];

/// Channel index → first slot index mapping (18 channels).
pub static CH_SLOT: [u8; 18] = [
    0, 1, 2, 6, 7, 8, 12, 13, 14, 18, 19, 20, 24, 25, 26, 30, 31, 32,
];

// ============================================================
// Pointer replacement enums
// ============================================================

/// Channel type constants (chtype field).
pub const CH_2OP: u8 = 0;
pub const CH_4OP: u8 = 1;
pub const CH_4OP2: u8 = 2;
pub const CH_DRUM: u8 = 3;

/// Envelope key-on type bitmask constants.
pub const EGK_NORM: u8 = 0x01;
pub const EGK_DRUM: u8 = 0x02;

/// Source of a slot's phase-modulation input.
/// Replaces the `int16_t *mod` raw pointer from the C struct.
#[derive(Clone, Copy, Default)]
pub enum ModInput {
    #[default]
    Zero,
    SlotOut(u8),   // chip.slot[i].out
    SlotFbMod(u8), // chip.slot[i].fbmod
}

/// Source of a channel's output mix contribution.
/// Replaces the `int16_t *out[4]` raw pointers from the C struct.
#[derive(Clone, Copy, Default)]
pub enum OutSrc {
    #[default]
    Zero,
    SlotOut(u8), // chip.slot[i].out
}

// ============================================================
// Struct layout
// ============================================================

/// One FM operator (maps to `opl3_slot` in opl3.h).
#[derive(Clone, Default)]
pub struct Opl3Slot {
    pub out: i16,
    pub fbmod: i16,
    pub mod_input: ModInput, // replaces *mod
    pub prout: i16,
    pub eg_rout: u16,
    pub eg_out: u16,
    pub eg_inc: u8,
    pub eg_gen: u8, // 0=attack, 1=decay, 2=sustain, 3=release
    pub eg_rate: u8,
    pub eg_ksl: u8,
    pub trem_chip: bool, // true → use chip.tremolo, false → 0
    pub reg_vib: u8,
    pub reg_type: u8,
    pub reg_ksr: u8,
    pub reg_mult: u8,
    pub reg_ksl: u8,
    pub reg_tl: u8,
    pub reg_ar: u8,
    pub reg_dr: u8,
    pub reg_sl: u8,
    pub reg_rr: u8,
    pub reg_wf: u8,
    pub key: u8, // EGK_NORM | EGK_DRUM bitmask
    pub pg_reset: bool,
    pub pg_phase: u32,
    pub pg_phase_out: u16,
    pub slot_num: u8,
    pub channel_num: u8,
}

/// One FM channel (maps to `opl3_channel` in opl3.h).
#[derive(Clone, Default)]
pub struct Opl3Channel {
    pub slotz: [u8; 2],   // slot indices into chip.slot[]
    pub pair_idx: u8,     // paired channel index; 0xFF = none
    pub out: [OutSrc; 4], // output sources (replaces *out[4])
    pub chtype: u8,       // CH_2OP / CH_4OP / CH_4OP2 / CH_DRUM
    pub f_num: u16,
    pub block: u8,
    pub fb: u8,
    pub con: u8,
    pub alg: u8,
    pub ksv: u8,
    pub cha: u16,
    pub chb: u16,
    pub chc: u16,
    pub chd: u16,
    pub ch_num: u8,
}

/// Deferred-write queue entry (maps to `opl3_writebuf` in opl3.h).
#[derive(Clone, Copy, Default)]
pub struct Opl3WriteBuf {
    pub time: u64,
    pub reg: u16,
    pub data: u8,
}

/// The complete OPL3 chip state (maps to `opl3_chip` in opl3.h).
pub struct Opl3Chip {
    pub channel: [Opl3Channel; 18],
    pub slot: [Opl3Slot; 36],
    pub timer: u16,
    pub eg_timer: u64,
    pub eg_timerrem: u8,
    pub eg_state: u8,
    pub eg_add: u8,
    pub eg_timer_lo: u8,
    pub newm: u8, // always 0 for OPL2 compat
    pub nts: u8,
    pub rhy: u8,
    pub vibpos: u8,
    pub vibshift: u8,
    pub tremolo: u8,
    pub tremolopos: u8,
    pub tremoloshift: u8,
    pub noise: u32,
    pub mixbuff: [i32; 4],
    pub rm_hh_bit2: u8,
    pub rm_hh_bit3: u8,
    pub rm_hh_bit7: u8,
    pub rm_hh_bit8: u8,
    pub rm_tc_bit3: u8,
    pub rm_tc_bit5: u8,
    // OPL3L resampler state
    pub rateratio: i32,
    pub samplecnt: i32,
    pub oldsamples: [i16; 4],
    pub samples: [i16; 4],
    // Deferred-write buffer (OPL_WRITEBUF_SIZE = 1024)
    pub writebuf_samplecnt: u64,
    pub writebuf_cur: u32,
    pub writebuf_last: u32,
    pub writebuf_lasttime: u64,
    pub writebuf: Vec<Opl3WriteBuf>, // length 1024
}

impl Default for Opl3Chip {
    fn default() -> Self {
        Self {
            channel: std::array::from_fn(|_| Opl3Channel::default()),
            slot: std::array::from_fn(|_| Opl3Slot::default()),
            timer: 0,
            eg_timer: 0,
            eg_timerrem: 0,
            eg_state: 0,
            eg_add: 0,
            eg_timer_lo: 0,
            newm: 0,
            nts: 0,
            rhy: 0,
            vibpos: 0,
            vibshift: 0,
            tremolo: 0,
            tremolopos: 0,
            tremoloshift: 0,
            noise: 0,
            mixbuff: [0; 4],
            rm_hh_bit2: 0,
            rm_hh_bit3: 0,
            rm_hh_bit7: 0,
            rm_hh_bit8: 0,
            rm_tc_bit3: 0,
            rm_tc_bit5: 0,
            rateratio: 0,
            samplecnt: 0,
            oldsamples: [0; 4],
            samples: [0; 4],
            writebuf_samplecnt: 0,
            writebuf_cur: 0,
            writebuf_last: 0,
            writebuf_lasttime: 0,
            writebuf: vec![Opl3WriteBuf::default(); 1024],
        }
    }
}

// ============================================================
// Envelope helpers
// ============================================================

/// Envelope generator phase constants (eg_gen field values).
pub const EG_NUM_ATTACK: u8 = 0;
pub const EG_NUM_DECAY: u8 = 1;
pub const EG_NUM_SUSTAIN: u8 = 2;
pub const EG_NUM_RELEASE: u8 = 3;

/// Compute exponential output from a log-domain level.
///
/// Ported from `OPL3_EnvelopeCalcExp`.
fn envelope_calc_exp(level: u32) -> i16 {
    let level = level.min(0x1fff);
    let v = (EXPROM[(level & 0xff) as usize] as u32) << 1;
    (v >> (level >> 8)) as i16
}

/// Waveform 0 — full sine, alternating sign.
///
/// Ported from `OPL3_EnvelopeCalcSin0`.
fn envelope_calc_sin0(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let neg: u16 = if phase & 0x200 != 0 { 0xffff } else { 0 };
    let out = if phase & 0x100 != 0 {
        LOGSINROM[((phase & 0xff) ^ 0xff) as usize]
    } else {
        LOGSINROM[(phase & 0xff) as usize]
    };
    (envelope_calc_exp(out as u32 + ((envelope as u32) << 3)) as u16 ^ neg) as i16
}

/// Waveform 1 — half sine, positive lobe only.
///
/// Ported from `OPL3_EnvelopeCalcSin1`.
fn envelope_calc_sin1(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let out: u16 = if phase & 0x200 != 0 {
        0x1000
    } else if phase & 0x100 != 0 {
        LOGSINROM[((phase & 0xff) ^ 0xff) as usize]
    } else {
        LOGSINROM[(phase & 0xff) as usize]
    };
    envelope_calc_exp(out as u32 + ((envelope as u32) << 3))
}

/// Waveform 2 — absolute sine, rectified.
///
/// Ported from `OPL3_EnvelopeCalcSin2`.
fn envelope_calc_sin2(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let out = if phase & 0x100 != 0 {
        LOGSINROM[((phase & 0xff) ^ 0xff) as usize]
    } else {
        LOGSINROM[(phase & 0xff) as usize]
    };
    envelope_calc_exp(out as u32 + ((envelope as u32) << 3))
}

/// Waveform 3 — quarter sine, positive quarter only.
///
/// Ported from `OPL3_EnvelopeCalcSin3`.
fn envelope_calc_sin3(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let out: u16 = if phase & 0x100 != 0 {
        0x1000
    } else {
        LOGSINROM[(phase & 0xff) as usize]
    };
    envelope_calc_exp(out as u32 + ((envelope as u32) << 3))
}

/// Waveform 4 — double-frequency sine, ±.
///
/// Ported from `OPL3_EnvelopeCalcSin4`.
fn envelope_calc_sin4(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let neg: u16 = if (phase & 0x300) == 0x100 { 0xffff } else { 0 };
    let out: u16 = if phase & 0x200 != 0 {
        0x1000
    } else if phase & 0x80 != 0 {
        LOGSINROM[(((phase ^ 0xff) << 1) & 0xff) as usize]
    } else {
        LOGSINROM[((phase << 1) & 0xff) as usize]
    };
    (envelope_calc_exp(out as u32 + ((envelope as u32) << 3)) as u16 ^ neg) as i16
}

/// Waveform 5 — double-frequency half sine.
///
/// Ported from `OPL3_EnvelopeCalcSin5`.
fn envelope_calc_sin5(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let out: u16 = if phase & 0x200 != 0 {
        0x1000
    } else if phase & 0x80 != 0 {
        LOGSINROM[(((phase ^ 0xff) << 1) & 0xff) as usize]
    } else {
        LOGSINROM[((phase << 1) & 0xff) as usize]
    };
    envelope_calc_exp(out as u32 + ((envelope as u32) << 3))
}

/// Waveform 6 — square wave.
///
/// Ported from `OPL3_EnvelopeCalcSin6`.
fn envelope_calc_sin6(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let neg: u16 = if phase & 0x200 != 0 { 0xffff } else { 0 };
    (envelope_calc_exp((envelope as u32) << 3) as u16 ^ neg) as i16
}

/// Waveform 7 — derived sawtooth.
///
/// Ported from `OPL3_EnvelopeCalcSin7`.
fn envelope_calc_sin7(phase: u16, envelope: u16) -> i16 {
    let mut phase = phase & 0x3ff;
    let neg: u16;
    if phase & 0x200 != 0 {
        neg = 0xffff;
        phase = (phase & 0x1ff) ^ 0x1ff;
    } else {
        neg = 0;
    }
    let out = (phase << 3) as u32;
    (envelope_calc_exp(out + ((envelope as u32) << 3)) as u16 ^ neg) as i16
}

/// Dispatch to one of the 8 waveform functions by reg_wf (0–7).
///
/// Rust dispatcher for the C `envelope_sin[]` function table.
pub(crate) fn envelope_calc_sin(wf: u8, phase: u16, envelope: u16) -> i16 {
    match wf & 0x07 {
        0 => envelope_calc_sin0(phase, envelope),
        1 => envelope_calc_sin1(phase, envelope),
        2 => envelope_calc_sin2(phase, envelope),
        3 => envelope_calc_sin3(phase, envelope),
        4 => envelope_calc_sin4(phase, envelope),
        5 => envelope_calc_sin5(phase, envelope),
        6 => envelope_calc_sin6(phase, envelope),
        _ => envelope_calc_sin7(phase, envelope),
    }
}

/// Update key-scale-level attenuation from the channel's current pitch.
///
/// Ported from `OPL3_EnvelopeUpdateKSL`.
pub(crate) fn envelope_update_ksl(chip: &mut Opl3Chip, slot_idx: usize) {
    let ch = chip.slot[slot_idx].channel_num as usize;
    let f_num = chip.channel[ch].f_num;
    let block = chip.channel[ch].block;
    let ksl = ((KSLROM[(f_num >> 6) as usize] as i16) << 2) - ((0x08i16 - block as i16) << 5);
    chip.slot[slot_idx].eg_ksl = ksl.max(0) as u8;
}

/// Advance the envelope state machine for one slot.
///
/// Borrow strategy: snapshot all chip-level read-only fields into locals first,
/// then take `&mut chip.slot[slot_idx]` — keeps chip.channel and chip.slot
/// borrows non-overlapping from the compiler's perspective.
///
/// Ported from `OPL3_EnvelopeCalc`.
pub(crate) fn envelope_calc(chip: &mut Opl3Chip, slot_idx: usize) {
    let eg_add = chip.eg_add;
    let eg_state = chip.eg_state;
    let eg_timer_lo = chip.eg_timer_lo;
    let ch = chip.slot[slot_idx].channel_num as usize;
    let ksv = chip.channel[ch].ksv;
    let trem = if chip.slot[slot_idx].trem_chip {
        chip.tremolo
    } else {
        0u8
    };

    let slot = &mut chip.slot[slot_idx];

    // Output attenuation: raw envelope + TL + KSL + tremolo.
    slot.eg_out = (slot.eg_rout as u32
        + ((slot.reg_tl as u32) << 2)
        + ((slot.eg_ksl as u32) >> KSLSHIFT[slot.reg_ksl as usize])
        + trem as u32) as u16;

    let mut reset = false;
    let mut reg_rate: u8 = 0;

    if slot.key != 0 && slot.eg_gen == EG_NUM_RELEASE {
        // Key pressed while releasing → restart attack.
        reset = true;
        reg_rate = slot.reg_ar;
    } else {
        match slot.eg_gen {
            EG_NUM_ATTACK => {
                reg_rate = slot.reg_ar;
            }
            EG_NUM_DECAY => {
                reg_rate = slot.reg_dr;
            }
            EG_NUM_SUSTAIN => {
                if slot.reg_type == 0 {
                    reg_rate = slot.reg_rr;
                }
            }
            EG_NUM_RELEASE => {
                reg_rate = slot.reg_rr;
            }
            _ => {}
        }
    }

    slot.pg_reset = reset;

    let ks = ksv >> ((slot.reg_ksr ^ 1) << 1);
    let nonzero = reg_rate != 0;
    let rate = ks as u16 + ((reg_rate as u16) << 2);
    let rate_hi_raw = (rate >> 2) as u8;
    let rate_lo = (rate & 0x03) as u8;
    let rate_hi = if rate_hi_raw & 0x10 != 0 {
        0x0fu8
    } else {
        rate_hi_raw
    };
    let eg_shift = rate_hi.wrapping_add(eg_add);
    let mut shift: u8 = 0;

    if nonzero {
        if rate_hi < 12 {
            if eg_state != 0 {
                shift = match eg_shift {
                    12 => 1,
                    13 => (rate_lo >> 1) & 0x01,
                    14 => rate_lo & 0x01,
                    _ => 0,
                };
            }
        } else {
            shift =
                (rate_hi & 0x03).wrapping_add(EG_INCSTEP[rate_lo as usize][eg_timer_lo as usize]);
            if shift & 0x04 != 0 {
                shift = 0x03;
            }
            if shift == 0 {
                shift = eg_state;
            }
        }
    }

    // Local eg_rout copy — may be overridden before writing back.
    let mut eg_rout = slot.eg_rout;
    let mut eg_inc: i32 = 0;

    // Instant attack: rate_hi == 0x0f resets counter to 0 immediately.
    if reset && rate_hi == 0x0f {
        eg_rout = 0x00;
    }
    // Envelope fully off: top 7 bits all set (tests original slot.eg_rout).
    let eg_off = (slot.eg_rout & 0x1f8) == 0x1f8;
    // Non-attack phases clamp counter at max when off.
    if slot.eg_gen != EG_NUM_ATTACK && !reset && eg_off {
        eg_rout = 0x1ff;
    }

    match slot.eg_gen {
        EG_NUM_ATTACK => {
            if slot.eg_rout == 0 {
                slot.eg_gen = EG_NUM_DECAY;
            } else if slot.key != 0 && shift > 0 && rate_hi != 0x0f {
                // Exponential attack curve: increment = ~rout >> (4 - shift).
                // Uses i32 arithmetic to mirror C's signed right-shift of ~uint16.
                eg_inc = !(slot.eg_rout as i32) >> (4u32.saturating_sub(shift as u32));
            }
        }
        EG_NUM_DECAY => {
            if (slot.eg_rout >> 4) == slot.reg_sl as u16 {
                slot.eg_gen = EG_NUM_SUSTAIN;
            } else if !eg_off && !reset && shift > 0 {
                eg_inc = 1i32 << ((shift - 1) as u32);
            }
        }
        EG_NUM_SUSTAIN | EG_NUM_RELEASE => {
            if !eg_off && !reset && shift > 0 {
                eg_inc = 1i32 << ((shift - 1) as u32);
            }
        }
        _ => {}
    }

    slot.eg_rout = ((eg_rout as i32 + eg_inc) & 0x1ff) as u16;

    // State transitions — ordering matches C: reset wins over key-off.
    if reset {
        slot.eg_gen = EG_NUM_ATTACK;
    }
    if slot.key == 0 {
        slot.eg_gen = EG_NUM_RELEASE;
    }
}

/// Set key-on type bits for a slot.
///
/// Ported from `OPL3_EnvelopeKeyOn`.
pub(crate) fn envelope_key_on(slot: &mut Opl3Slot, key_type: u8) {
    slot.key |= key_type;
}

/// Clear key-on type bits for a slot.
///
/// Ported from `OPL3_EnvelopeKeyOff`.
pub(crate) fn envelope_key_off(slot: &mut Opl3Slot, key_type: u8) {
    slot.key &= !key_type;
}

// ============================================================
// Slot operations
// ============================================================

/// Read the phase-modulation input value for a slot (immutable borrow of chip).
///
/// Called *before* any mutable access to `chip.slot[slot_idx]` so the
/// exclusive borrow can follow without aliasing.
///
/// Rust helper: no direct C equivalent; resolves the `ModInput` enum replacing
/// C's `*slot->mod` pointer dereference in `OPL3_SlotGenerate` / `OPL3_SlotCalcFB`.
pub(crate) fn read_mod_input(chip: &Opl3Chip, mi: ModInput) -> i16 {
    match mi {
        ModInput::Zero => 0,
        ModInput::SlotOut(i) => chip.slot[i as usize].out,
        ModInput::SlotFbMod(i) => chip.slot[i as usize].fbmod,
    }
}

/// Update the feedback modulation register for a slot.
///
/// `fb` is the channel's feedback level — must be pre-read from
/// `chip.channel[ch].fb` by the caller to satisfy the borrow checker.
///
/// The C formula: `fbmod = (prout + out) >> (9 − fb)`
/// Both operands are `i16`; C promotes to `int` before shifting.
///
/// Ported from `OPL3_SlotCalcFB`.
pub(crate) fn slot_calc_fb(slot: &mut Opl3Slot, fb: u8) {
    if fb != 0 {
        slot.fbmod = ((slot.prout as i32 + slot.out as i32) >> (0x09 - fb as u32)) as i16;
    } else {
        slot.fbmod = 0;
    }
    slot.prout = slot.out;
}

/// Compute the output sample for one slot.
///
/// Borrow strategy — all reads complete before the exclusive write to `out`:
/// 1. Copy `mod_input` (enum, `Copy`) from slot.
/// 2. Call `read_mod_input` (immutable `chip` borrow, released on return).
/// 3. Copy remaining needed fields (all `Copy` scalars).
/// 4. Write `chip.slot[slot_idx].out` (exclusive borrow, no overlap).
///
/// C: `slot->out = envelope_sin[reg_wf](pg_phase_out + *mod, eg_out)`
/// `pg_phase_out + *mod` — both u16/i16 are promoted to int in C; the
/// wrapping semantics are preserved by casting mod_val to u16 before adding.
///
/// Ported from `OPL3_SlotGenerate`.
pub(crate) fn slot_generate(chip: &mut Opl3Chip, slot_idx: usize) {
    let mod_input = chip.slot[slot_idx].mod_input; // Copy
    let mod_val = read_mod_input(chip, mod_input); // immutable borrow released here
    let pg_phase_out = chip.slot[slot_idx].pg_phase_out;
    let eg_out = chip.slot[slot_idx].eg_out;
    let reg_wf = chip.slot[slot_idx].reg_wf;
    // phase addition mirrors C's int promotion: wrapping_add with mod cast to u16
    let phase = pg_phase_out.wrapping_add(mod_val as u16);
    chip.slot[slot_idx].out = envelope_calc_sin(reg_wf, phase, eg_out);
}

// ============================================================
// Phase generator
// ============================================================

/// Advance the phase accumulator for one slot and update rhythm-mode phase
/// outputs and the noise LFSR.
///
/// Borrow strategy: snapshot all read-only fields (channel f_num/block, slot
/// reg_vib/reg_mult/slot_num/pg_reset/pg_phase, chip vibpos/vibshift/rhy/noise
/// and all rm_* bits) into locals, compute results, then write back the two
/// slot fields and chip-level fields separately.
///
/// Ported from `OPL3_PhaseGenerate`.
pub(crate) fn phase_generate(chip: &mut Opl3Chip, slot_idx: usize) {
    // --- Snapshot ---
    let ch = chip.slot[slot_idx].channel_num as usize;
    let f_num_base = chip.channel[ch].f_num;
    let block = chip.channel[ch].block;
    let reg_vib = chip.slot[slot_idx].reg_vib;
    let reg_mult = chip.slot[slot_idx].reg_mult;
    let slot_num = chip.slot[slot_idx].slot_num;
    let pg_reset = chip.slot[slot_idx].pg_reset;
    let pg_phase = chip.slot[slot_idx].pg_phase;
    let vibpos = chip.vibpos;
    let vibshift = chip.vibshift;
    let rhy = chip.rhy;
    let noise = chip.noise;
    let rm_hh_bit2 = chip.rm_hh_bit2;
    let rm_hh_bit3 = chip.rm_hh_bit3;
    let rm_hh_bit7 = chip.rm_hh_bit7;
    let rm_hh_bit8 = chip.rm_hh_bit8;
    let rm_tc_bit3 = chip.rm_tc_bit3;
    let rm_tc_bit5 = chip.rm_tc_bit5;

    // --- Vibrato ---
    // range is an i8: derived from bits [9:7] of f_num (0–7), then halved,
    // shifted, and optionally negated depending on vibpos phase.
    let mut f_num = f_num_base;
    if reg_vib != 0 {
        let mut range = ((f_num_base >> 7) & 7) as i8;
        if vibpos & 3 == 0 {
            range = 0;
        } else if vibpos & 1 != 0 {
            range >>= 1;
        }
        range >>= vibshift; // vibshift is 0 (deep) or 1 (normal)
        if vibpos & 4 != 0 {
            range = -range;
        }
        // Wrapping add mirrors C's u16 truncation of (u16 + i8 via int promotion).
        f_num = f_num_base.wrapping_add(range as u16);
    }

    // --- Phase accumulator ---
    // Capture the output phase BEFORE the reset/increment.
    let phase = (pg_phase >> 9) as u16;
    let basefreq = ((f_num as u32) << block) >> 1;
    let base = if pg_reset { 0u32 } else { pg_phase };
    let new_pg_phase = base.wrapping_add((MT[reg_mult as usize] as u32 * basefreq) >> 1);

    // --- Rhythm-mode bit extraction ---
    // hh (slot 13) updates its own bits; tc (slot 17) updates its bits.
    // rm_xor is then recomputed with the freshest values so the same-slot
    // phase override uses the bits set in this very call.
    let mut new_rm_hh_bit2 = rm_hh_bit2;
    let mut new_rm_hh_bit3 = rm_hh_bit3;
    let mut new_rm_hh_bit7 = rm_hh_bit7;
    let mut new_rm_hh_bit8 = rm_hh_bit8;
    let mut new_rm_tc_bit3 = rm_tc_bit3;
    let mut new_rm_tc_bit5 = rm_tc_bit5;

    if slot_num == 13 {
        new_rm_hh_bit2 = (phase >> 2) as u8 & 1;
        new_rm_hh_bit3 = (phase >> 3) as u8 & 1;
        new_rm_hh_bit7 = (phase >> 7) as u8 & 1;
        new_rm_hh_bit8 = (phase >> 8) as u8 & 1;
    }
    if slot_num == 17 && (rhy & 0x20) != 0 {
        new_rm_tc_bit3 = (phase >> 3) as u8 & 1;
        new_rm_tc_bit5 = (phase >> 5) as u8 & 1;
    }

    // --- Rhythm-mode phase override ---
    let mut pg_phase_out = phase;
    if rhy & 0x20 != 0 {
        let rm_xor = (new_rm_hh_bit2 ^ new_rm_hh_bit7)
            | (new_rm_hh_bit3 ^ new_rm_tc_bit5)
            | (new_rm_tc_bit3 ^ new_rm_tc_bit5);
        match slot_num {
            13 => {
                // hi-hat: phase driven by rm_xor and noise LSB
                pg_phase_out = (rm_xor as u16) << 9;
                if rm_xor ^ (noise as u8 & 1) != 0 {
                    pg_phase_out |= 0xd0;
                } else {
                    pg_phase_out |= 0x34;
                }
            }
            16 => {
                // snare drum: driven by hh_bit8 and noise LSB
                pg_phase_out = ((new_rm_hh_bit8 as u16) << 9)
                    | (((new_rm_hh_bit8 ^ (noise as u8 & 1)) as u16) << 8);
            }
            17 => {
                // top cymbal: driven by rm_xor
                pg_phase_out = ((rm_xor as u16) << 9) | 0x80;
            }
            _ => {}
        }
    }

    // --- Noise LFSR (Galois): feedback = bit14 XOR bit0 → inserted at bit22 ---
    let n_bit = ((noise >> 14) ^ noise) & 0x01;
    let new_noise = (noise >> 1) | (n_bit << 22);

    // --- Write back ---
    chip.slot[slot_idx].pg_phase = new_pg_phase;
    chip.slot[slot_idx].pg_phase_out = pg_phase_out;
    chip.rm_hh_bit2 = new_rm_hh_bit2;
    chip.rm_hh_bit3 = new_rm_hh_bit3;
    chip.rm_hh_bit7 = new_rm_hh_bit7;
    chip.rm_hh_bit8 = new_rm_hh_bit8;
    chip.rm_tc_bit3 = new_rm_tc_bit3;
    chip.rm_tc_bit5 = new_rm_tc_bit5;
    chip.noise = new_noise;
}

// ============================================================
// Channel setup
// ============================================================

/// Wire up the modulation network for a channel.
///
/// Translates C's raw-pointer assignments into index-based enum assignments:
/// - `slot->mod = &slot->fbmod`      → `slot.mod_input = ModInput::SlotFbMod(idx)`
/// - `slot->mod = &other_slot->out`  → `slot.mod_input = ModInput::SlotOut(idx)`
/// - `slot->mod = &chip->zeromod`    → `slot.mod_input = ModInput::Zero`
/// - `chan->out[i] = &slot->out`     → `chan.out[i] = OutSrc::SlotOut(idx)`
/// - `chan->out[i] = &chip->zeromod` → `chan.out[i] = OutSrc::Zero`
///
/// In the 4-op case `ch_idx` is the *secondary* channel (ch_4op2, alg | 0x04)
/// and `pair_idx` is the *primary* channel (ch_4op, alg | 0x08). The primary's
/// outputs are zeroed — only the secondary channel contributes to the mix.
///
/// Borrow strategy: snapshot all slot/channel indices into `usize` locals before
/// any mutations so the compiler sees no overlapping exclusive borrows.
///
/// Ported from `OPL3_ChannelSetupAlg`.
pub(crate) fn channel_setup_alg(chip: &mut Opl3Chip, ch_idx: usize) {
    let chtype = chip.channel[ch_idx].chtype;
    let alg = chip.channel[ch_idx].alg;
    let s0 = chip.channel[ch_idx].slotz[0] as usize;
    let s1 = chip.channel[ch_idx].slotz[1] as usize;
    let ch_num = chip.channel[ch_idx].ch_num;
    let pair_idx = chip.channel[ch_idx].pair_idx as usize;

    // ── Drum channels ────────────────────────────────────────────────────────
    if chtype == CH_DRUM {
        if ch_num == 7 || ch_num == 8 {
            // Hi-hat / snare-drum / top-cymbal / tom: no FM modulation input.
            chip.slot[s0].mod_input = ModInput::Zero;
            chip.slot[s1].mod_input = ModInput::Zero;
            return;
        }
        // Bass drum (ch_num == 6): optional series-FM or additive.
        match alg & 0x01 {
            0x00 => {
                chip.slot[s0].mod_input = ModInput::SlotFbMod(s0 as u8);
                chip.slot[s1].mod_input = ModInput::SlotOut(s0 as u8);
            }
            _ => {
                chip.slot[s0].mod_input = ModInput::SlotFbMod(s0 as u8);
                chip.slot[s1].mod_input = ModInput::Zero;
            }
        }
        return;
    }

    // ── 4-op primary (alg bit 3): nothing to wire here; secondary handles it.
    if alg & 0x08 != 0 {
        return;
    }

    // ── 4-op secondary (alg bit 2): ch_idx = secondary, pair_idx = primary. ──
    if alg & 0x04 != 0 {
        if pair_idx >= 18 {
            return; // safety: no valid pair (shouldn't happen in OPL2 mode)
        }
        let ps0 = chip.channel[pair_idx].slotz[0] as usize;
        let ps1 = chip.channel[pair_idx].slotz[1] as usize;

        // Primary channel contributes nothing to the output mix.
        chip.channel[pair_idx].out = [OutSrc::Zero; 4];

        match alg & 0x03 {
            0x00 => {
                // Series: ps0 → ps1 → s0 → s1 → out
                chip.slot[ps0].mod_input = ModInput::SlotFbMod(ps0 as u8);
                chip.slot[ps1].mod_input = ModInput::SlotOut(ps0 as u8);
                chip.slot[s0].mod_input = ModInput::SlotOut(ps1 as u8);
                chip.slot[s1].mod_input = ModInput::SlotOut(s0 as u8);
                chip.channel[ch_idx].out = [
                    OutSrc::SlotOut(s1 as u8),
                    OutSrc::Zero,
                    OutSrc::Zero,
                    OutSrc::Zero,
                ];
            }
            0x01 => {
                // ps0 → ps1 → out;  zero → s0 → s1 → out
                chip.slot[ps0].mod_input = ModInput::SlotFbMod(ps0 as u8);
                chip.slot[ps1].mod_input = ModInput::SlotOut(ps0 as u8);
                chip.slot[s0].mod_input = ModInput::Zero;
                chip.slot[s1].mod_input = ModInput::SlotOut(s0 as u8);
                chip.channel[ch_idx].out = [
                    OutSrc::SlotOut(ps1 as u8),
                    OutSrc::SlotOut(s1 as u8),
                    OutSrc::Zero,
                    OutSrc::Zero,
                ];
            }
            0x02 => {
                // ps0 → out;  zero → ps1 → s0 → s1 → out
                chip.slot[ps0].mod_input = ModInput::SlotFbMod(ps0 as u8);
                chip.slot[ps1].mod_input = ModInput::Zero;
                chip.slot[s0].mod_input = ModInput::SlotOut(ps1 as u8);
                chip.slot[s1].mod_input = ModInput::SlotOut(s0 as u8);
                chip.channel[ch_idx].out = [
                    OutSrc::SlotOut(ps0 as u8),
                    OutSrc::SlotOut(s1 as u8),
                    OutSrc::Zero,
                    OutSrc::Zero,
                ];
            }
            _ => {
                // ps0 → out;  zero → ps1 → s0 → out;  zero → s1 → out
                chip.slot[ps0].mod_input = ModInput::SlotFbMod(ps0 as u8);
                chip.slot[ps1].mod_input = ModInput::Zero;
                chip.slot[s0].mod_input = ModInput::SlotOut(ps1 as u8);
                chip.slot[s1].mod_input = ModInput::Zero;
                chip.channel[ch_idx].out = [
                    OutSrc::SlotOut(ps0 as u8),
                    OutSrc::SlotOut(s0 as u8),
                    OutSrc::SlotOut(s1 as u8),
                    OutSrc::Zero,
                ];
            }
        }
        return;
    }

    // ── 2-op channel ─────────────────────────────────────────────────────────
    match alg & 0x01 {
        0x00 => {
            // Series FM: s0 modulates s1; s1 → out.
            chip.slot[s0].mod_input = ModInput::SlotFbMod(s0 as u8);
            chip.slot[s1].mod_input = ModInput::SlotOut(s0 as u8);
            chip.channel[ch_idx].out = [
                OutSrc::SlotOut(s1 as u8),
                OutSrc::Zero,
                OutSrc::Zero,
                OutSrc::Zero,
            ];
        }
        _ => {
            // Additive: s0 and s1 both contribute independently.
            chip.slot[s0].mod_input = ModInput::SlotFbMod(s0 as u8);
            chip.slot[s1].mod_input = ModInput::Zero;
            chip.channel[ch_idx].out = [
                OutSrc::SlotOut(s0 as u8),
                OutSrc::SlotOut(s1 as u8),
                OutSrc::Zero,
                OutSrc::Zero,
            ];
        }
    }
}

/// Update algorithm selection after a con/chtype change.
///
/// In OPL2 mode (`newm == 0`) always reduces to: `channel.alg = channel.con`
/// then `channel_setup_alg`. The full OPL3 4-op paths are kept so the function
/// works correctly if `newm` is ever set to 1 by a future OPL3 wrapper.
///
/// Ported from `OPL3_ChannelUpdateAlg`.
pub(crate) fn channel_update_alg(chip: &mut Opl3Chip, ch_idx: usize) {
    let con = chip.channel[ch_idx].con;
    chip.channel[ch_idx].alg = con;
    if chip.newm != 0 {
        let chtype = chip.channel[ch_idx].chtype;
        if chtype == CH_4OP {
            // Primary: give pair the composite alg; own alg = 0x08 (skip).
            let pair_idx = chip.channel[ch_idx].pair_idx as usize;
            let pair_con = chip.channel[pair_idx].con;
            chip.channel[pair_idx].alg = 0x04 | (con << 1) | pair_con;
            chip.channel[ch_idx].alg = 0x08;
            channel_setup_alg(chip, pair_idx);
        } else if chtype == CH_4OP2 {
            // Secondary: own alg = 0x04 | ...; give primary alg = 0x08.
            let pair_idx = chip.channel[ch_idx].pair_idx as usize;
            let pair_con = chip.channel[pair_idx].con;
            chip.channel[ch_idx].alg = 0x04 | (pair_con << 1) | con;
            chip.channel[pair_idx].alg = 0x08;
            channel_setup_alg(chip, ch_idx);
        } else {
            channel_setup_alg(chip, ch_idx);
        }
    } else {
        channel_setup_alg(chip, ch_idx);
    }
}

/// Handle a write to the rhythm-mode control register.
///
/// When bit 5 is set, channels 6–8 are reconfigured as drum channels with fixed
/// output routing. The `channel.out[]` assignments here are intentionally done
/// *before* calling `channel_setup_alg` (which only writes slot `mod_input` for
/// drum channels, not `channel.out`), so they are preserved.
///
/// Ported from `OPL3_ChannelUpdateRhythm`.
pub(crate) fn channel_update_rhythm(chip: &mut Opl3Chip, data: u8) {
    chip.rhy = data & 0x3f;
    if chip.rhy & 0x20 != 0 {
        // Snapshot slot indices before any mutations.
        let c6s0 = chip.channel[6].slotz[0];
        let c6s1 = chip.channel[6].slotz[1];
        let c7s0 = chip.channel[7].slotz[0];
        let c7s1 = chip.channel[7].slotz[1];
        let c8s0 = chip.channel[8].slotz[0];
        let c8s1 = chip.channel[8].slotz[1];

        // Bass drum (ch6): both outputs from slot1 (carrier).
        chip.channel[6].out = [
            OutSrc::SlotOut(c6s1),
            OutSrc::SlotOut(c6s1),
            OutSrc::Zero,
            OutSrc::Zero,
        ];
        // Hi-hat (slot0) + snare drum (slot1) — both on L+R.
        chip.channel[7].out = [
            OutSrc::SlotOut(c7s0),
            OutSrc::SlotOut(c7s0),
            OutSrc::SlotOut(c7s1),
            OutSrc::SlotOut(c7s1),
        ];
        // Tom (slot0) + top cymbal (slot1) — both on L+R.
        chip.channel[8].out = [
            OutSrc::SlotOut(c8s0),
            OutSrc::SlotOut(c8s0),
            OutSrc::SlotOut(c8s1),
            OutSrc::SlotOut(c8s1),
        ];

        for ch in 6..9usize {
            chip.channel[ch].chtype = CH_DRUM;
        }
        channel_setup_alg(chip, 6);
        channel_setup_alg(chip, 7);
        channel_setup_alg(chip, 8);

        let rhy = chip.rhy; // copy before slot borrows

        // hi-hat: ch7 slot0
        let s = c7s0 as usize;
        if rhy & 0x01 != 0 {
            envelope_key_on(&mut chip.slot[s], EGK_DRUM);
        } else {
            envelope_key_off(&mut chip.slot[s], EGK_DRUM);
        }

        // top cymbal: ch8 slot1
        let s = c8s1 as usize;
        if rhy & 0x02 != 0 {
            envelope_key_on(&mut chip.slot[s], EGK_DRUM);
        } else {
            envelope_key_off(&mut chip.slot[s], EGK_DRUM);
        }

        // tom: ch8 slot0
        let s = c8s0 as usize;
        if rhy & 0x04 != 0 {
            envelope_key_on(&mut chip.slot[s], EGK_DRUM);
        } else {
            envelope_key_off(&mut chip.slot[s], EGK_DRUM);
        }

        // snare drum: ch7 slot1
        let s = c7s1 as usize;
        if rhy & 0x08 != 0 {
            envelope_key_on(&mut chip.slot[s], EGK_DRUM);
        } else {
            envelope_key_off(&mut chip.slot[s], EGK_DRUM);
        }

        // bass drum: ch6 both slots
        let s0 = c6s0 as usize;
        let s1 = c6s1 as usize;
        if rhy & 0x10 != 0 {
            envelope_key_on(&mut chip.slot[s0], EGK_DRUM);
            envelope_key_on(&mut chip.slot[s1], EGK_DRUM);
        } else {
            envelope_key_off(&mut chip.slot[s0], EGK_DRUM);
            envelope_key_off(&mut chip.slot[s1], EGK_DRUM);
        }
    } else {
        for ch in 6..9usize {
            let s0 = chip.channel[ch].slotz[0] as usize;
            let s1 = chip.channel[ch].slotz[1] as usize;
            chip.channel[ch].chtype = CH_2OP;
            channel_setup_alg(chip, ch);
            envelope_key_off(&mut chip.slot[s0], EGK_DRUM);
            envelope_key_off(&mut chip.slot[s1], EGK_DRUM);
        }
    }
}

// ============================================================
// Per-channel write handlers
// ============================================================

/// Set the low 8 bits of f_num, recompute ksv, and refresh KSL attenuation.
/// OPL3 4-op primary channels propagate the change to their paired channel.
///
/// Ported from `OPL3_ChannelWriteA0`.
pub(crate) fn channel_write_a0(chip: &mut Opl3Chip, ch_idx: usize, data: u8) {
    // OPL3 only: ch_4op2 channels are driven by their primary; ignore direct writes.
    if chip.newm != 0 && chip.channel[ch_idx].chtype == CH_4OP2 {
        return;
    }

    // Snapshot nts before the mutable borrow.
    let nts = chip.nts;

    // Update f_num (low 8 bits) and recompute ksv.
    chip.channel[ch_idx].f_num = (chip.channel[ch_idx].f_num & 0x300) | u16::from(data);
    let block = chip.channel[ch_idx].block;
    let f_num = chip.channel[ch_idx].f_num;
    chip.channel[ch_idx].ksv = (block << 1) | (((f_num >> (0x09 - nts)) & 0x01) as u8);

    // Refresh KSL on both slots.
    let s0 = chip.channel[ch_idx].slotz[0] as usize;
    let s1 = chip.channel[ch_idx].slotz[1] as usize;
    envelope_update_ksl(chip, s0);
    envelope_update_ksl(chip, s1);

    // OPL3 only: propagate f_num/ksv to the paired (secondary) channel.
    if chip.newm != 0 && chip.channel[ch_idx].chtype == CH_4OP {
        let pair_idx = chip.channel[ch_idx].pair_idx as usize;
        let ksv = chip.channel[ch_idx].ksv;
        chip.channel[pair_idx].f_num = f_num;
        chip.channel[pair_idx].ksv = ksv;
        let ps0 = chip.channel[pair_idx].slotz[0] as usize;
        let ps1 = chip.channel[pair_idx].slotz[1] as usize;
        envelope_update_ksl(chip, ps0);
        envelope_update_ksl(chip, ps1);
    }
}

/// Set the high 2 bits of f_num and the block field, recompute ksv, refresh KSL.
/// OPL3 4-op primary channels propagate the change to their paired channel.
///
/// Ported from `OPL3_ChannelWriteB0`.
pub(crate) fn channel_write_b0(chip: &mut Opl3Chip, ch_idx: usize, data: u8) {
    // OPL3 only: ignore direct writes to secondary 4-op channels.
    if chip.newm != 0 && chip.channel[ch_idx].chtype == CH_4OP2 {
        return;
    }

    let nts = chip.nts;

    chip.channel[ch_idx].f_num =
        (chip.channel[ch_idx].f_num & 0xff) | (u16::from(data & 0x03) << 8);
    chip.channel[ch_idx].block = (data >> 2) & 0x07;
    let block = chip.channel[ch_idx].block;
    let f_num = chip.channel[ch_idx].f_num;
    chip.channel[ch_idx].ksv = (block << 1) | (((f_num >> (0x09 - nts)) & 0x01) as u8);

    let s0 = chip.channel[ch_idx].slotz[0] as usize;
    let s1 = chip.channel[ch_idx].slotz[1] as usize;
    envelope_update_ksl(chip, s0);
    envelope_update_ksl(chip, s1);

    // OPL3 only: propagate to paired channel.
    if chip.newm != 0 && chip.channel[ch_idx].chtype == CH_4OP {
        let pair_idx = chip.channel[ch_idx].pair_idx as usize;
        let ksv = chip.channel[ch_idx].ksv;
        chip.channel[pair_idx].f_num = f_num;
        chip.channel[pair_idx].block = block;
        chip.channel[pair_idx].ksv = ksv;
        let ps0 = chip.channel[pair_idx].slotz[0] as usize;
        let ps1 = chip.channel[pair_idx].slotz[1] as usize;
        envelope_update_ksl(chip, ps0);
        envelope_update_ksl(chip, ps1);
    }
}

/// Write feedback depth (fb), connection (con), and stereo output enables.
/// Calls `channel_update_alg` to recompute the slot routing.
/// In OPL2 compat mode (`newm == 0`) both DAC1 outputs are enabled and DAC2
/// outputs are disabled.
///
/// Ported from `OPL3_ChannelWriteC0`.
pub(crate) fn channel_write_c0(chip: &mut Opl3Chip, ch_idx: usize, data: u8) {
    chip.channel[ch_idx].fb = (data & 0x0e) >> 1;
    chip.channel[ch_idx].con = data & 0x01;
    channel_update_alg(chip, ch_idx);

    if chip.newm != 0 {
        // OPL3: bits 4-7 individually enable the four output channels.
        chip.channel[ch_idx].cha = if (data >> 4) & 0x01 != 0 { 0xffff } else { 0 };
        chip.channel[ch_idx].chb = if (data >> 5) & 0x01 != 0 { 0xffff } else { 0 };
        chip.channel[ch_idx].chc = if (data >> 6) & 0x01 != 0 { 0xffff } else { 0 };
        chip.channel[ch_idx].chd = if (data >> 7) & 0x01 != 0 { 0xffff } else { 0 };
    } else {
        // OPL2 compat: DAC1 (cha/chb) always enabled, DAC2 (chc/chd) always off.
        chip.channel[ch_idx].cha = 0xffff;
        chip.channel[ch_idx].chb = 0xffff;
        chip.channel[ch_idx].chc = 0;
        chip.channel[ch_idx].chd = 0;
    }
}

/// Trigger key-on for both slots of the channel (and pair slots in OPL3 4-op mode).
/// 4-op secondary channels (CH_4OP2) are skipped — keyed through their primary.
///
/// Ported from `OPL3_ChannelKeyOn`.
pub(crate) fn channel_key_on(chip: &mut Opl3Chip, ch_idx: usize) {
    let s0 = chip.channel[ch_idx].slotz[0] as usize;
    let s1 = chip.channel[ch_idx].slotz[1] as usize;

    if chip.newm != 0 {
        let chtype = chip.channel[ch_idx].chtype;
        if chtype == CH_4OP {
            // Key on primary and paired secondary slots.
            let pair_idx = chip.channel[ch_idx].pair_idx as usize;
            let ps0 = chip.channel[pair_idx].slotz[0] as usize;
            let ps1 = chip.channel[pair_idx].slotz[1] as usize;
            envelope_key_on(&mut chip.slot[s0], EGK_NORM);
            envelope_key_on(&mut chip.slot[s1], EGK_NORM);
            envelope_key_on(&mut chip.slot[ps0], EGK_NORM);
            envelope_key_on(&mut chip.slot[ps1], EGK_NORM);
        } else if chtype == CH_2OP || chtype == CH_DRUM {
            envelope_key_on(&mut chip.slot[s0], EGK_NORM);
            envelope_key_on(&mut chip.slot[s1], EGK_NORM);
        }
        // CH_4OP2: skipped — driven by primary channel's key-on.
    } else {
        // OPL2 compat: always key on both slots.
        envelope_key_on(&mut chip.slot[s0], EGK_NORM);
        envelope_key_on(&mut chip.slot[s1], EGK_NORM);
    }
}

/// Trigger key-off for both slots of the channel (and pair slots in OPL3 4-op mode).
/// Mirror of `channel_key_on`.
///
/// Ported from `OPL3_ChannelKeyOff`.
pub(crate) fn channel_key_off(chip: &mut Opl3Chip, ch_idx: usize) {
    let s0 = chip.channel[ch_idx].slotz[0] as usize;
    let s1 = chip.channel[ch_idx].slotz[1] as usize;

    if chip.newm != 0 {
        let chtype = chip.channel[ch_idx].chtype;
        if chtype == CH_4OP {
            let pair_idx = chip.channel[ch_idx].pair_idx as usize;
            let ps0 = chip.channel[pair_idx].slotz[0] as usize;
            let ps1 = chip.channel[pair_idx].slotz[1] as usize;
            envelope_key_off(&mut chip.slot[s0], EGK_NORM);
            envelope_key_off(&mut chip.slot[s1], EGK_NORM);
            envelope_key_off(&mut chip.slot[ps0], EGK_NORM);
            envelope_key_off(&mut chip.slot[ps1], EGK_NORM);
        } else if chtype == CH_2OP || chtype == CH_DRUM {
            envelope_key_off(&mut chip.slot[s0], EGK_NORM);
            envelope_key_off(&mut chip.slot[s1], EGK_NORM);
        }
        // CH_4OP2: skipped.
    } else {
        // OPL2 compat: always key off both slots.
        envelope_key_off(&mut chip.slot[s0], EGK_NORM);
        envelope_key_off(&mut chip.slot[s1], EGK_NORM);
    }
}

// ============================================================
// Per-slot write handlers
// ============================================================

/// Write register 0x20–0x35: AM/VIB/EGT/KSR/MULT flags.
/// Sets `trem_chip` (true = use chip-level tremolo, false = 0), vibrato,
/// envelope type, key-scale rate, and frequency multiplier.
///
/// Ported from `OPL3_SlotWrite20`.
pub(crate) fn slot_write_20(slot: &mut Opl3Slot, data: u8) {
    slot.trem_chip = (data >> 7) & 0x01 != 0; // bit 7 → AM (use chip tremolo)
    slot.reg_vib = (data >> 6) & 0x01; // bit 6 → vibrato enable
    slot.reg_type = (data >> 5) & 0x01; // bit 5 → envelope type (EGT)
    slot.reg_ksr = (data >> 4) & 0x01; // bit 4 → key scale rate
    slot.reg_mult = data & 0x0f; // bits 0-3 → frequency multiplier index
}

/// Write register 0x40–0x55: KSL/TL (key scale level / total level).
/// Refreshes KSL attenuation immediately after updating the registers.
/// Requires `chip` and `slot_idx` because `envelope_update_ksl` recalculates
/// `slot.eg_ksl` from `chip.channel[ch].ksv` and `slot.reg_ksl`.
///
/// Ported from `OPL3_SlotWrite40`.
pub(crate) fn slot_write_40(chip: &mut Opl3Chip, slot_idx: usize, data: u8) {
    chip.slot[slot_idx].reg_ksl = (data >> 6) & 0x03; // bits 6-7
    chip.slot[slot_idx].reg_tl = data & 0x3f; // bits 0-5
    envelope_update_ksl(chip, slot_idx);
}

/// Write register 0x60–0x75: AR/DR (attack rate / decay rate).
///
/// Ported from `OPL3_SlotWrite60`.
pub(crate) fn slot_write_60(slot: &mut Opl3Slot, data: u8) {
    slot.reg_ar = (data >> 4) & 0x0f; // bits 4-7 → attack rate
    slot.reg_dr = data & 0x0f; // bits 0-3 → decay rate
}

/// Write register 0x80–0x95: SL/RR (sustain level / release rate).
/// Sustain level 15 (0x0f) is expanded to 31 (0x1f) to match OPL hardware behaviour.
///
/// Ported from `OPL3_SlotWrite80`.
pub(crate) fn slot_write_80(slot: &mut Opl3Slot, data: u8) {
    slot.reg_sl = (data >> 4) & 0x0f; // bits 4-7 → sustain level
    if slot.reg_sl == 0x0f {
        slot.reg_sl = 0x1f; // OPL hardware quirk: SL=15 treated as SL=31
    }
    slot.reg_rr = data & 0x0f; // bits 0-3 → release rate
}

/// Write register 0xE0–0xF5: WS (waveform select).
/// In OPL2 compat mode (`newm == 0`), only waveforms 0–3 are available (bits 0-1).
/// In OPL3 mode (`newm != 0`), all 8 waveforms are available (bits 0-2).
/// `newm` is passed explicitly so the caller can borrow `chip.slot[slot_idx]` mutably.
///
/// Ported from `OPL3_SlotWriteE0`.
pub(crate) fn slot_write_e0(slot: &mut Opl3Slot, data: u8, newm: u8) {
    slot.reg_wf = data & 0x07;
    if newm == 0 {
        slot.reg_wf &= 0x03; // OPL2: clamp to 4 waveforms
    }
}

// ============================================================
// Top-level generation
// ============================================================

/// Fixed-point fraction bits for the resampler (RSM_FRAC).
const RSM_FRAC: i32 = 10;

/// Write-buffer queue capacity (OPL_WRITEBUF_SIZE from opl3.h line 46).
pub(crate) const OPL_WRITEBUF_SIZE: usize = 1024;

/// Maximum value of the 36-bit envelope timer (UINT64_C(0xfffffffff)).
const EG_TIMER_MAX: u64 = 0xF_FFFF_FFFF;

/// Clamp a 32-bit mixed sum to the i16 output range.
///
/// Ported from `OPL3_ClipSample`.
fn clip_sample(sample: i32) -> i16 {
    sample.clamp(-32768, 32767) as i16
}

/// Sum all 4 output sources of channel `ch` into a single i16 value.
/// Each `OutSrc` is either zero or the current `chip.slot[i].out`.
/// Requires only an immutable borrow of chip; safe to call between process_slot loops.
///
/// Rust helper: no direct C equivalent; replaces the inline `channel->out[]` pointer
/// accumulation in `OPL3_Generate4Ch`.
fn channel_accm(chip: &Opl3Chip, ch: usize) -> i16 {
    let out = &chip.channel[ch].out;
    let mut sum = 0i32;
    for os in out {
        sum += i32::from(match *os {
            OutSrc::Zero => 0,
            OutSrc::SlotOut(i) => chip.slot[i as usize].out,
        });
    }
    sum as i16 // truncate like C's int16_t assignment
}

/// Advance one FM operator slot by one sample.
/// Order: feedback → envelope → phase → waveform output.
///
/// Ported from `OPL3_ProcessSlot`.
pub(crate) fn process_slot(chip: &mut Opl3Chip, slot_idx: usize) {
    // slot_calc_fb takes (slot, fb); pre-read fb before mutable slot borrow.
    let ch_num = chip.slot[slot_idx].channel_num as usize;
    let fb = chip.channel[ch_num].fb;
    slot_calc_fb(&mut chip.slot[slot_idx], fb);
    envelope_calc(chip, slot_idx);
    phase_generate(chip, slot_idx);
    slot_generate(chip, slot_idx);
}

/// Generate one raw 4-channel sample frame: [L-DAC1, R-DAC1, L-DAC2, R-DAC2].
///
/// Uses `OPL_QUIRK_CHANNELSAMPLEDELAY = 1` (the upstream default): slots 0–14
/// are processed before the left-channel mix; slots 15–17 after; slots 18–32
/// before the right-channel mix; slots 33–35 after. This introduces a 1-slot
/// delay between L and R to match real OPL3 hardware timing.
///
/// In OPL2 compat mode (`newm = 0`): `cha = chb = 0xFFFF`, `chc = chd = 0`,
/// so buf4[0] ≈ buf4[1] (mono) and buf4[2] = buf4[3] = 0.
///
/// Ported from `OPL3_Generate4Ch`.
pub(crate) fn generate_4ch(chip: &mut Opl3Chip, buf4: &mut [i16; 4]) {
    // Output the RIGHT channel results buffered from the previous call.
    buf4[1] = clip_sample(chip.mixbuff[1]);
    buf4[3] = clip_sample(chip.mixbuff[3]);

    // OPL_QUIRK_CHANNELSAMPLEDELAY: process slots 0–14 before left mix.
    for ii in 0..15usize {
        process_slot(chip, ii);
    }

    // Accumulate LEFT channel (cha = DAC1-left, chc = DAC2-left).
    let mut mix = [0i32; 2];
    for ii in 0..18usize {
        let accm = channel_accm(chip, ii);
        let cha = chip.channel[ii].cha;
        let chc = chip.channel[ii].chc;
        // Bit-AND with u16 mask then reinterpret as i16, matching C's
        // `(int16_t)(accm & channel->cha)` semantics.
        mix[0] += i32::from((accm as u16 & cha) as i16);
        mix[1] += i32::from((accm as u16 & chc) as i16);
    }
    chip.mixbuff[0] = mix[0];
    chip.mixbuff[2] = mix[1];

    // Slots 15–17 processed after left mix, before left output.
    for ii in 15..18usize {
        process_slot(chip, ii);
    }

    // Output LEFT channel.
    buf4[0] = clip_sample(chip.mixbuff[0]);
    buf4[2] = clip_sample(chip.mixbuff[2]);

    // OPL_QUIRK: slots 18–32 before right mix.
    for ii in 18..33usize {
        process_slot(chip, ii);
    }

    // Accumulate RIGHT channel (chb = DAC1-right, chd = DAC2-right).
    mix[0] = 0;
    mix[1] = 0;
    for ii in 0..18usize {
        let accm = channel_accm(chip, ii);
        let chb = chip.channel[ii].chb;
        let chd = chip.channel[ii].chd;
        mix[0] += i32::from((accm as u16 & chb) as i16);
        mix[1] += i32::from((accm as u16 & chd) as i16);
    }
    chip.mixbuff[1] = mix[0];
    chip.mixbuff[3] = mix[1];

    // Trailing slots 33–35.
    for ii in 33..36usize {
        process_slot(chip, ii);
    }

    // Tremolo: triangle wave, period 210 ticks, updated every 64 chip-timer ticks.
    if (chip.timer & 0x3f) == 0x3f {
        chip.tremolopos = (chip.tremolopos + 1) % 210;
    }
    chip.tremolo = if chip.tremolopos < 105 {
        chip.tremolopos >> chip.tremoloshift
    } else {
        (210 - chip.tremolopos) >> chip.tremoloshift
    };

    // Vibrato: 8-step LUT, updated every 1024 chip-timer ticks.
    if (chip.timer & 0x3ff) == 0x3ff {
        chip.vibpos = (chip.vibpos + 1) & 7;
    }

    chip.timer = chip.timer.wrapping_add(1);

    // Envelope timer: computes eg_add shift on odd samples (eg_state = 1).
    if chip.eg_state != 0 {
        let mut shift = 0u8;
        while shift < 13 && ((chip.eg_timer >> (shift as u64)) & 1) == 0 {
            shift += 1;
        }
        chip.eg_add = if shift > 12 { 0 } else { shift + 1 };
        chip.eg_timer_lo = (chip.eg_timer & 0x3) as u8;
    }
    if chip.eg_timerrem != 0 || chip.eg_state != 0 {
        if chip.eg_timer == EG_TIMER_MAX {
            chip.eg_timer = 0;
            chip.eg_timerrem = 1;
        } else {
            chip.eg_timer += 1;
            chip.eg_timerrem = 0;
        }
    }
    chip.eg_state ^= 1;

    // Write buffer: drain any pending register writes up to this sample count.
    // Ported from the tail of OPL3_Generate4Ch.
    loop {
        let cur = chip.writebuf_cur as usize;
        // Snapshot fields before the write_reg mutable borrow.
        let wb_time = chip.writebuf[cur].time;
        let wb_reg = chip.writebuf[cur].reg;
        if wb_time > chip.writebuf_samplecnt {
            break;
        }
        if wb_reg & 0x200 == 0 {
            break;
        }
        let wb_data = chip.writebuf[cur].data;
        write_reg(chip, wb_reg & 0x1ff, wb_data);
        chip.writebuf_cur = ((cur + 1) % OPL_WRITEBUF_SIZE) as u32;
    }
    chip.writebuf_samplecnt += 1;
}

/// Resample the 4-channel OPL output from the native 49716 Hz rate to the
/// configured output sample rate using fixed-point linear interpolation.
///
/// Ported from `OPL3_Generate4ChResampled`.
fn generate_4ch_resampled(chip: &mut Opl3Chip, buf4: &mut [i16; 4]) {
    while chip.samplecnt >= chip.rateratio {
        chip.oldsamples = chip.samples;
        // Must write into a temporary because chip.samples is inside chip
        // which generate_4ch borrows mutably.
        let mut tmp = [0i16; 4];
        generate_4ch(chip, &mut tmp);
        chip.samples = tmp;
        chip.samplecnt -= chip.rateratio;
    }
    let ratio = chip.rateratio;
    let cnt = chip.samplecnt;
    for ((out, &old), &cur) in buf4
        .iter_mut()
        .zip(chip.oldsamples.iter())
        .zip(chip.samples.iter())
    {
        *out = ((old as i32 * (ratio - cnt) + cur as i32 * cnt) / ratio) as i16;
    }
    chip.samplecnt += 1 << RSM_FRAC;
}

/// Generate one stereo sample [L, R] at the configured output sample rate.
/// Wraps `generate_4ch_resampled` and returns only DAC1 outputs (buf4[0]/buf4[1]).
/// In OPL2 compat mode DAC2 (buf4[2]/buf4[3]) is always zero.
///
/// Ported from `OPL3_GenerateResampled`.
pub fn generate_resampled(chip: &mut Opl3Chip, buf: &mut [i16; 2]) {
    let mut samples = [0i16; 4];
    generate_4ch_resampled(chip, &mut samples);
    buf[0] = samples[0];
    buf[1] = samples[1];
}

// ============================================================
// Public API
// ============================================================

/// OPL_WRITEBUF_DELAY: minimum sample delay between write_reg_buffered writes.
const OPL_WRITEBUF_DELAY: u64 = 2;

/// Enable/disable 4-op channel pairing.
/// Bits 0-5 of `data` each control one 4-op pair.
/// Bit N=1 → channels N and N+3 become a 4-op pair; N=0 → both revert to 2-op.
///
/// Ported from `OPL3_ChannelSet4Op`.
fn channel_set_4op(chip: &mut Opl3Chip, data: u8) {
    for bit in 0u8..6 {
        // For bits 0-2: chnum = bit; for bits 3-5: chnum = bit + 6 (banks 9-11).
        let chnum = if bit >= 3 {
            (bit + 6) as usize
        } else {
            bit as usize
        };
        if (data >> bit) & 0x01 != 0 {
            chip.channel[chnum].chtype = CH_4OP;
            chip.channel[chnum + 3].chtype = CH_4OP2;
            channel_update_alg(chip, chnum);
        } else {
            chip.channel[chnum].chtype = CH_2OP;
            chip.channel[chnum + 3].chtype = CH_2OP;
            channel_update_alg(chip, chnum);
            channel_update_alg(chip, chnum + 3);
        }
    }
}

/// Reset the chip to power-on state and configure the resampler for `samplerate`.
/// Equivalent to `memset(chip, 0)` + field initialisation in the C reset function.
/// `chip.newm` is left at 0 (OPL2 compat); it is never set to 1 in AdLib mode.
///
/// Ported from `OPL3_Reset`.
pub fn reset(chip: &mut Opl3Chip, samplerate: u32) {
    // Zero everything; Default impl allocates the write buffer with 1024 entries.
    *chip = Opl3Chip::default();

    // Slot initialisation: each slot starts in the release phase with full attenuation.
    for slotnum in 0..36usize {
        chip.slot[slotnum].eg_rout = 0x1ff;
        chip.slot[slotnum].eg_out = 0x1ff;
        chip.slot[slotnum].eg_gen = EG_NUM_RELEASE;
        chip.slot[slotnum].slot_num = slotnum as u8;
        // mod_input = ModInput::Zero, trem_chip = false (both from Default)
    }

    // Channel initialisation: wire slotz, pair_idx, ch_num, cha/chb, then
    // call channel_setup_alg to configure the modulation network.
    for (channum, &ch_slot_u8) in CH_SLOT.iter().enumerate() {
        let local_ch_slot = ch_slot_u8 as usize;
        chip.channel[channum].slotz[0] = local_ch_slot as u8;
        chip.channel[channum].slotz[1] = (local_ch_slot + 3) as u8;
        chip.slot[local_ch_slot].channel_num = channum as u8;
        chip.slot[local_ch_slot + 3].channel_num = channum as u8;

        // pair_idx: channels 0-2 pair with 3-5, channels 9-11 pair with 12-14.
        chip.channel[channum].pair_idx = if channum % 9 < 3 {
            (channum + 3) as u8
        } else if channum % 9 < 6 {
            (channum - 3) as u8
        } else {
            0xFF // no pair
        };

        chip.channel[channum].ch_num = channum as u8;
        chip.channel[channum].chtype = CH_2OP;
        chip.channel[channum].cha = 0xffff; // DAC1 outputs always on (OPL2)
        chip.channel[channum].chb = 0xffff;
        // chc, chd remain 0 (DAC2 disabled in OPL2 compat mode)

        // out[] defaults to [OutSrc::Zero; 4]; channel_setup_alg fills in the live ones.
        channel_setup_alg(chip, channum);
    }

    chip.noise = 1; // LFSR must not start at 0
    chip.rateratio = (samplerate as i32 * (1 << RSM_FRAC)) / 49716;
    chip.tremoloshift = 4; // shallow tremolo by default
    chip.vibshift = 1; // shallow vibrato by default
}

/// Write a value to OPL register `reg`.
/// Dispatches to the appropriate slot/channel/chip-level handler.
/// In OPL2 compat mode (`newm = 0`) the high bank (`reg >= 0x100`) and OPL3-only
/// registers (0x104/0x105) are accepted but have no audible effect.
///
/// Ported from `OPL3_WriteReg`.
pub fn write_reg(chip: &mut Opl3Chip, reg: u16, v: u8) {
    let high = ((reg >> 8) & 0x01) as usize; // 0 = bank 0, 1 = bank 1 (OPL3 only)
    let regm = (reg & 0xff) as u8;

    match regm & 0xf0 {
        0x00 => {
            if high != 0 {
                // OPL3 bank-1 global registers
                match regm & 0x0f {
                    0x04 => channel_set_4op(chip, v),
                    0x05 => chip.newm = v & 0x01, // OPL3 mode enable (unused in AdLib)
                    _ => {}
                }
            } else {
                // Bank-0 global registers
                if regm & 0x0f == 0x08 {
                    chip.nts = (v >> 6) & 0x01; // note select
                }
            }
        }
        0x20 | 0x30 => {
            // Slot register: AM/VIB/EGT/KSR/MULT
            let adslot_idx = (regm & 0x1f) as usize;
            if AD_SLOT[adslot_idx] >= 0 {
                let slot_idx = 18 * high + AD_SLOT[adslot_idx] as usize;
                slot_write_20(&mut chip.slot[slot_idx], v);
            }
        }
        0x40 | 0x50 => {
            // Slot register: KSL/TL
            let adslot_idx = (regm & 0x1f) as usize;
            if AD_SLOT[adslot_idx] >= 0 {
                let slot_idx = 18 * high + AD_SLOT[adslot_idx] as usize;
                slot_write_40(chip, slot_idx, v);
            }
        }
        0x60 | 0x70 => {
            // Slot register: AR/DR
            let adslot_idx = (regm & 0x1f) as usize;
            if AD_SLOT[adslot_idx] >= 0 {
                let slot_idx = 18 * high + AD_SLOT[adslot_idx] as usize;
                slot_write_60(&mut chip.slot[slot_idx], v);
            }
        }
        0x80 | 0x90 => {
            // Slot register: SL/RR
            let adslot_idx = (regm & 0x1f) as usize;
            if AD_SLOT[adslot_idx] >= 0 {
                let slot_idx = 18 * high + AD_SLOT[adslot_idx] as usize;
                slot_write_80(&mut chip.slot[slot_idx], v);
            }
        }
        0xe0 | 0xf0 => {
            // Slot register: WS (waveform select)
            let adslot_idx = (regm & 0x1f) as usize;
            if AD_SLOT[adslot_idx] >= 0 {
                let slot_idx = 18 * high + AD_SLOT[adslot_idx] as usize;
                let newm = chip.newm; // pre-read before mutable slot borrow
                slot_write_e0(&mut chip.slot[slot_idx], v, newm);
            }
        }
        0xa0 => {
            // Channel register A0: f_num low 8 bits
            if (regm & 0x0f) < 9 {
                let ch_idx = 9 * high + (regm & 0x0f) as usize;
                channel_write_a0(chip, ch_idx, v);
            }
        }
        0xb0 => {
            // Channel register B0: f_num high bits / block / key-on
            if regm == 0xbd && high == 0 {
                // BD register: tremolo/vibrato depth + rhythm mode
                chip.tremoloshift = (((v >> 7) ^ 1) << 1) + 2;
                chip.vibshift = ((v >> 6) & 0x01) ^ 1;
                channel_update_rhythm(chip, v);
            } else if (regm & 0x0f) < 9 {
                let ch_idx = 9 * high + (regm & 0x0f) as usize;
                channel_write_b0(chip, ch_idx, v);
                if v & 0x20 != 0 {
                    channel_key_on(chip, ch_idx);
                } else {
                    channel_key_off(chip, ch_idx);
                }
            }
        }
        0xc0 => {
            // Channel register C0: feedback / connection / output enables
            if (regm & 0x0f) < 9 {
                let ch_idx = 9 * high + (regm & 0x0f) as usize;
                channel_write_c0(chip, ch_idx, v);
            }
        }
        _ => {}
    }
}

/// Queue a register write, scheduling it at least `OPL_WRITEBUF_DELAY` samples
/// in the future for accurate timing. The write is drained by `generate_4ch`.
///
/// Ported from `OPL3_WriteRegBuffered`.
pub fn write_reg_buffered(chip: &mut Opl3Chip, reg: u16, v: u8) {
    let writebuf_last = chip.writebuf_last as usize;

    // Snapshot writebuf[last] before the write_reg mutable borrow.
    let wb_reg = chip.writebuf[writebuf_last].reg;
    let wb_data = chip.writebuf[writebuf_last].data;
    let wb_time = chip.writebuf[writebuf_last].time;

    // If the slot has a pending valid write, flush it immediately.
    if wb_reg & 0x200 != 0 {
        write_reg(chip, wb_reg & 0x1ff, wb_data);
        chip.writebuf_cur = ((writebuf_last + 1) % OPL_WRITEBUF_SIZE) as u32;
        chip.writebuf_samplecnt = wb_time;
    }

    // Enqueue the new write with a timestamp no earlier than the previous one.
    chip.writebuf[writebuf_last].reg = reg | 0x200;
    chip.writebuf[writebuf_last].data = v;
    let time = (chip.writebuf_lasttime + OPL_WRITEBUF_DELAY).max(chip.writebuf_samplecnt);
    chip.writebuf[writebuf_last].time = time;
    chip.writebuf_lasttime = time;
    chip.writebuf_last = ((writebuf_last + 1) % OPL_WRITEBUF_SIZE) as u32;
}

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
// Step 1a — ROM tables (literal transcription from opl3.c)
// ============================================================

/// Log-sine ROM (256 entries), extracted from OPL2 die shot.
pub static LOGSINROM: [u16; 256] = [
    0x859, 0x6c3, 0x607, 0x58b, 0x52e, 0x4e4, 0x4a6, 0x471,
    0x443, 0x41a, 0x3f5, 0x3d3, 0x3b5, 0x398, 0x37e, 0x365,
    0x34e, 0x339, 0x324, 0x311, 0x2ff, 0x2ed, 0x2dc, 0x2cd,
    0x2bd, 0x2af, 0x2a0, 0x293, 0x286, 0x279, 0x26d, 0x261,
    0x256, 0x24b, 0x240, 0x236, 0x22c, 0x222, 0x218, 0x20f,
    0x206, 0x1fd, 0x1f5, 0x1ec, 0x1e4, 0x1dc, 0x1d4, 0x1cd,
    0x1c5, 0x1be, 0x1b7, 0x1b0, 0x1a9, 0x1a2, 0x19b, 0x195,
    0x18f, 0x188, 0x182, 0x17c, 0x177, 0x171, 0x16b, 0x166,
    0x160, 0x15b, 0x155, 0x150, 0x14b, 0x146, 0x141, 0x13c,
    0x137, 0x133, 0x12e, 0x129, 0x125, 0x121, 0x11c, 0x118,
    0x114, 0x10f, 0x10b, 0x107, 0x103, 0x0ff, 0x0fb, 0x0f8,
    0x0f4, 0x0f0, 0x0ec, 0x0e9, 0x0e5, 0x0e2, 0x0de, 0x0db,
    0x0d7, 0x0d4, 0x0d1, 0x0cd, 0x0ca, 0x0c7, 0x0c4, 0x0c1,
    0x0be, 0x0bb, 0x0b8, 0x0b5, 0x0b2, 0x0af, 0x0ac, 0x0a9,
    0x0a7, 0x0a4, 0x0a1, 0x09f, 0x09c, 0x099, 0x097, 0x094,
    0x092, 0x08f, 0x08d, 0x08a, 0x088, 0x086, 0x083, 0x081,
    0x07f, 0x07d, 0x07a, 0x078, 0x076, 0x074, 0x072, 0x070,
    0x06e, 0x06c, 0x06a, 0x068, 0x066, 0x064, 0x062, 0x060,
    0x05e, 0x05c, 0x05b, 0x059, 0x057, 0x055, 0x053, 0x052,
    0x050, 0x04e, 0x04d, 0x04b, 0x04a, 0x048, 0x046, 0x045,
    0x043, 0x042, 0x040, 0x03f, 0x03e, 0x03c, 0x03b, 0x039,
    0x038, 0x037, 0x035, 0x034, 0x033, 0x031, 0x030, 0x02f,
    0x02e, 0x02d, 0x02b, 0x02a, 0x029, 0x028, 0x027, 0x026,
    0x025, 0x024, 0x023, 0x022, 0x021, 0x020, 0x01f, 0x01e,
    0x01d, 0x01c, 0x01b, 0x01a, 0x019, 0x018, 0x017, 0x017,
    0x016, 0x015, 0x014, 0x014, 0x013, 0x012, 0x011, 0x011,
    0x010, 0x00f, 0x00f, 0x00e, 0x00d, 0x00d, 0x00c, 0x00c,
    0x00b, 0x00a, 0x00a, 0x009, 0x009, 0x008, 0x008, 0x007,
    0x007, 0x007, 0x006, 0x006, 0x005, 0x005, 0x005, 0x004,
    0x004, 0x004, 0x003, 0x003, 0x003, 0x002, 0x002, 0x002,
    0x002, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001, 0x001,
    0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
];

/// Exponential ROM (256 entries), extracted from OPL2 die shot.
pub static EXPROM: [u16; 256] = [
    0x7fa, 0x7f5, 0x7ef, 0x7ea, 0x7e4, 0x7df, 0x7da, 0x7d4,
    0x7cf, 0x7c9, 0x7c4, 0x7bf, 0x7b9, 0x7b4, 0x7ae, 0x7a9,
    0x7a4, 0x79f, 0x799, 0x794, 0x78f, 0x78a, 0x784, 0x77f,
    0x77a, 0x775, 0x770, 0x76a, 0x765, 0x760, 0x75b, 0x756,
    0x751, 0x74c, 0x747, 0x742, 0x73d, 0x738, 0x733, 0x72e,
    0x729, 0x724, 0x71f, 0x71a, 0x715, 0x710, 0x70b, 0x706,
    0x702, 0x6fd, 0x6f8, 0x6f3, 0x6ee, 0x6e9, 0x6e5, 0x6e0,
    0x6db, 0x6d6, 0x6d2, 0x6cd, 0x6c8, 0x6c4, 0x6bf, 0x6ba,
    0x6b5, 0x6b1, 0x6ac, 0x6a8, 0x6a3, 0x69e, 0x69a, 0x695,
    0x691, 0x68c, 0x688, 0x683, 0x67f, 0x67a, 0x676, 0x671,
    0x66d, 0x668, 0x664, 0x65f, 0x65b, 0x657, 0x652, 0x64e,
    0x649, 0x645, 0x641, 0x63c, 0x638, 0x634, 0x630, 0x62b,
    0x627, 0x623, 0x61e, 0x61a, 0x616, 0x612, 0x60e, 0x609,
    0x605, 0x601, 0x5fd, 0x5f9, 0x5f5, 0x5f0, 0x5ec, 0x5e8,
    0x5e4, 0x5e0, 0x5dc, 0x5d8, 0x5d4, 0x5d0, 0x5cc, 0x5c8,
    0x5c4, 0x5c0, 0x5bc, 0x5b8, 0x5b4, 0x5b0, 0x5ac, 0x5a8,
    0x5a4, 0x5a0, 0x59c, 0x599, 0x595, 0x591, 0x58d, 0x589,
    0x585, 0x581, 0x57e, 0x57a, 0x576, 0x572, 0x56f, 0x56b,
    0x567, 0x563, 0x560, 0x55c, 0x558, 0x554, 0x551, 0x54d,
    0x549, 0x546, 0x542, 0x53e, 0x53b, 0x537, 0x534, 0x530,
    0x52c, 0x529, 0x525, 0x522, 0x51e, 0x51b, 0x517, 0x514,
    0x510, 0x50c, 0x509, 0x506, 0x502, 0x4ff, 0x4fb, 0x4f8,
    0x4f4, 0x4f1, 0x4ed, 0x4ea, 0x4e7, 0x4e3, 0x4e0, 0x4dc,
    0x4d9, 0x4d6, 0x4d2, 0x4cf, 0x4cc, 0x4c8, 0x4c5, 0x4c2,
    0x4be, 0x4bb, 0x4b8, 0x4b5, 0x4b1, 0x4ae, 0x4ab, 0x4a8,
    0x4a4, 0x4a1, 0x49e, 0x49b, 0x498, 0x494, 0x491, 0x48e,
    0x48b, 0x488, 0x485, 0x482, 0x47e, 0x47b, 0x478, 0x475,
    0x472, 0x46f, 0x46c, 0x469, 0x466, 0x463, 0x460, 0x45d,
    0x45a, 0x457, 0x454, 0x451, 0x44e, 0x44b, 0x448, 0x445,
    0x442, 0x43f, 0x43c, 0x439, 0x436, 0x433, 0x430, 0x42d,
    0x42a, 0x428, 0x425, 0x422, 0x41f, 0x41c, 0x419, 0x416,
    0x414, 0x411, 0x40e, 0x40b, 0x408, 0x406, 0x403, 0x400,
];

/// Frequency multiplier table (×2): 1/2,1,2,3,4,5,6,7,8,9,10,10,12,12,15,15
pub static MT: [u8; 16] = [
    1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30,
];

/// Key scale level ROM.
pub static KSLROM: [u8; 16] = [
    0, 32, 40, 45, 48, 51, 53, 55, 56, 58, 59, 60, 61, 62, 63, 64,
];

/// Key scale level shift amounts indexed by reg_ksl (0–3).
pub static KSLSHIFT: [u8; 4] = [8, 1, 2, 0];

/// Envelope increment step table [rate_lo][eg_timer_lo].
pub static EG_INCSTEP: [[u8; 4]; 4] = [
    [0, 0, 0, 0],
    [1, 0, 0, 0],
    [1, 0, 1, 0],
    [1, 1, 1, 0],
];

/// Register address → slot index mapping (−1 = invalid address).
pub static AD_SLOT: [i8; 0x20] = [
     0,  1,  2,  3,  4,  5, -1, -1,
     6,  7,  8,  9, 10, 11, -1, -1,
    12, 13, 14, 15, 16, 17, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1,
];

/// Channel index → first slot index mapping (18 channels).
pub static CH_SLOT: [u8; 18] = [
     0,  1,  2,  6,  7,  8,
    12, 13, 14, 18, 19, 20,
    24, 25, 26, 30, 31, 32,
];

// ============================================================
// Step 1b — Pointer replacement enums
// ============================================================

/// Channel type constants (chtype field).
pub const CH_2OP:  u8 = 0;
pub const CH_4OP:  u8 = 1;
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
// Step 1c — Struct layout
// ============================================================

/// One FM operator (maps to `opl3_slot` in opl3.h).
#[derive(Clone, Default)]
pub struct Opl3Slot {
    pub out:          i16,
    pub fbmod:        i16,
    pub mod_input:    ModInput, // replaces *mod
    pub prout:        i16,
    pub eg_rout:      u16,
    pub eg_out:       u16,
    pub eg_inc:       u8,
    pub eg_gen:       u8,  // 0=attack, 1=decay, 2=sustain, 3=release
    pub eg_rate:      u8,
    pub eg_ksl:       u8,
    pub trem_chip:    bool, // true → use chip.tremolo, false → 0
    pub reg_vib:      u8,
    pub reg_type:     u8,
    pub reg_ksr:      u8,
    pub reg_mult:     u8,
    pub reg_ksl:      u8,
    pub reg_tl:       u8,
    pub reg_ar:       u8,
    pub reg_dr:       u8,
    pub reg_sl:       u8,
    pub reg_rr:       u8,
    pub reg_wf:       u8,
    pub key:          u8,  // EGK_NORM | EGK_DRUM bitmask
    pub pg_reset:     bool,
    pub pg_phase:     u32,
    pub pg_phase_out: u16,
    pub slot_num:     u8,
    pub channel_num:  u8,
}

/// One FM channel (maps to `opl3_channel` in opl3.h).
#[derive(Clone, Default)]
pub struct Opl3Channel {
    pub slotz:    [u8; 2],     // slot indices into chip.slot[]
    pub pair_idx: u8,          // paired channel index; 0xFF = none
    pub out:      [OutSrc; 4], // output sources (replaces *out[4])
    pub chtype:   u8,          // CH_2OP / CH_4OP / CH_4OP2 / CH_DRUM
    pub f_num:    u16,
    pub block:    u8,
    pub fb:       u8,
    pub con:      u8,
    pub alg:      u8,
    pub ksv:      u8,
    pub cha:      u16,
    pub chb:      u16,
    pub chc:      u16,
    pub chd:      u16,
    pub ch_num:   u8,
}

/// Deferred-write queue entry (maps to `opl3_writebuf` in opl3.h).
#[derive(Clone, Copy, Default)]
pub struct Opl3WriteBuf {
    pub time: u64,
    pub reg:  u16,
    pub data: u8,
}

/// The complete OPL3 chip state (maps to `opl3_chip` in opl3.h).
pub struct Opl3Chip {
    pub channel:  [Opl3Channel; 18],
    pub slot:     [Opl3Slot; 36],
    pub timer:    u16,
    pub eg_timer: u64,
    pub eg_timerrem:  u8,
    pub eg_state:     u8,
    pub eg_add:       u8,
    pub eg_timer_lo:  u8,
    pub newm:     u8, // always 0 for OPL2 compat
    pub nts:      u8,
    pub rhy:      u8,
    pub vibpos:   u8,
    pub vibshift: u8,
    pub tremolo:  u8,
    pub tremolopos:   u8,
    pub tremoloshift: u8,
    pub noise:    u32,
    pub mixbuff:  [i32; 4],
    pub rm_hh_bit2: u8,
    pub rm_hh_bit3: u8,
    pub rm_hh_bit7: u8,
    pub rm_hh_bit8: u8,
    pub rm_tc_bit3: u8,
    pub rm_tc_bit5: u8,
    // OPL3L resampler state
    pub rateratio:  i32,
    pub samplecnt:  i32,
    pub oldsamples: [i16; 4],
    pub samples:    [i16; 4],
    // Deferred-write buffer (OPL_WRITEBUF_SIZE = 1024)
    pub writebuf_samplecnt: u64,
    pub writebuf_cur:       u32,
    pub writebuf_last:      u32,
    pub writebuf_lasttime:  u64,
    pub writebuf:           Vec<Opl3WriteBuf>, // length 1024
}

impl Default for Opl3Chip {
    fn default() -> Self {
        Self {
            channel:  std::array::from_fn(|_| Opl3Channel::default()),
            slot:     std::array::from_fn(|_| Opl3Slot::default()),
            timer:    0,
            eg_timer: 0,
            eg_timerrem:  0,
            eg_state:     0,
            eg_add:       0,
            eg_timer_lo:  0,
            newm:     0,
            nts:      0,
            rhy:      0,
            vibpos:   0,
            vibshift: 0,
            tremolo:  0,
            tremolopos:   0,
            tremoloshift: 0,
            noise:    0,
            mixbuff:  [0; 4],
            rm_hh_bit2: 0,
            rm_hh_bit3: 0,
            rm_hh_bit7: 0,
            rm_hh_bit8: 0,
            rm_tc_bit3: 0,
            rm_tc_bit5: 0,
            rateratio:  0,
            samplecnt:  0,
            oldsamples: [0; 4],
            samples:    [0; 4],
            writebuf_samplecnt: 0,
            writebuf_cur:       0,
            writebuf_last:      0,
            writebuf_lasttime:  0,
            writebuf: vec![Opl3WriteBuf::default(); 1024],
        }
    }
}

// ============================================================
// Step 1d — Envelope helpers
// ============================================================

/// Envelope generator phase constants (eg_gen field values).
pub const EG_NUM_ATTACK:  u8 = 0;
pub const EG_NUM_DECAY:   u8 = 1;
pub const EG_NUM_SUSTAIN: u8 = 2;
pub const EG_NUM_RELEASE: u8 = 3;

/// Compute exponential output from a log-domain level (OPL3_EnvelopeCalcExp).
fn envelope_calc_exp(level: u32) -> i16 {
    let level = level.min(0x1fff);
    let v = (EXPROM[(level & 0xff) as usize] as u32) << 1;
    (v >> (level >> 8)) as i16
}

/// Waveform 0 — full sine, alternating sign (OPL3_EnvelopeCalcSin0).
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

/// Waveform 1 — half sine, positive lobe only (OPL3_EnvelopeCalcSin1).
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

/// Waveform 2 — absolute sine, rectified (OPL3_EnvelopeCalcSin2).
fn envelope_calc_sin2(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let out = if phase & 0x100 != 0 {
        LOGSINROM[((phase & 0xff) ^ 0xff) as usize]
    } else {
        LOGSINROM[(phase & 0xff) as usize]
    };
    envelope_calc_exp(out as u32 + ((envelope as u32) << 3))
}

/// Waveform 3 — quarter sine, positive quarter only (OPL3_EnvelopeCalcSin3).
fn envelope_calc_sin3(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let out: u16 = if phase & 0x100 != 0 {
        0x1000
    } else {
        LOGSINROM[(phase & 0xff) as usize]
    };
    envelope_calc_exp(out as u32 + ((envelope as u32) << 3))
}

/// Waveform 4 — double-frequency sine, ± (OPL3_EnvelopeCalcSin4).
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

/// Waveform 5 — double-frequency half sine (OPL3_EnvelopeCalcSin5).
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

/// Waveform 6 — square wave (OPL3_EnvelopeCalcSin6).
fn envelope_calc_sin6(phase: u16, envelope: u16) -> i16 {
    let phase = phase & 0x3ff;
    let neg: u16 = if phase & 0x200 != 0 { 0xffff } else { 0 };
    (envelope_calc_exp((envelope as u32) << 3) as u16 ^ neg) as i16
}

/// Waveform 7 — derived sawtooth (OPL3_EnvelopeCalcSin7).
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

/// Update key-scale-level attenuation from the channel's current pitch
/// (OPL3_EnvelopeUpdateKSL).
pub(crate) fn envelope_update_ksl(chip: &mut Opl3Chip, slot_idx: usize) {
    let ch    = chip.slot[slot_idx].channel_num as usize;
    let f_num = chip.channel[ch].f_num;
    let block = chip.channel[ch].block;
    let ksl = ((KSLROM[(f_num >> 6) as usize] as i16) << 2)
            - ((0x08i16 - block as i16) << 5);
    chip.slot[slot_idx].eg_ksl = ksl.max(0) as u8;
}

/// Advance the envelope state machine for one slot (OPL3_EnvelopeCalc).
///
/// Borrow strategy: snapshot all chip-level read-only fields into locals first,
/// then take `&mut chip.slot[slot_idx]` — keeps chip.channel and chip.slot
/// borrows non-overlapping from the compiler's perspective.
pub(crate) fn envelope_calc(chip: &mut Opl3Chip, slot_idx: usize) {
    let eg_add      = chip.eg_add;
    let eg_state    = chip.eg_state;
    let eg_timer_lo = chip.eg_timer_lo;
    let ch          = chip.slot[slot_idx].channel_num as usize;
    let ksv         = chip.channel[ch].ksv;
    let trem        = if chip.slot[slot_idx].trem_chip { chip.tremolo } else { 0u8 };

    let slot = &mut chip.slot[slot_idx];

    // Output attenuation: raw envelope + TL + KSL + tremolo.
    slot.eg_out = (slot.eg_rout as u32
        + ((slot.reg_tl as u32) << 2)
        + (slot.eg_ksl >> KSLSHIFT[slot.reg_ksl as usize]) as u32
        + trem as u32) as u16;

    let mut reset    = false;
    let mut reg_rate: u8 = 0;

    if slot.key != 0 && slot.eg_gen == EG_NUM_RELEASE {
        // Key pressed while releasing → restart attack.
        reset    = true;
        reg_rate = slot.reg_ar;
    } else {
        match slot.eg_gen {
            EG_NUM_ATTACK  => { reg_rate = slot.reg_ar; }
            EG_NUM_DECAY   => { reg_rate = slot.reg_dr; }
            EG_NUM_SUSTAIN => { if slot.reg_type == 0 { reg_rate = slot.reg_rr; } }
            EG_NUM_RELEASE => { reg_rate = slot.reg_rr; }
            _ => {}
        }
    }

    slot.pg_reset = reset;

    let ks          = ksv >> ((slot.reg_ksr ^ 1) << 1);
    let nonzero     = reg_rate != 0;
    let rate        = ks as u16 + ((reg_rate as u16) << 2);
    let rate_hi_raw = (rate >> 2) as u8;
    let rate_lo     = (rate & 0x03) as u8;
    let rate_hi     = if rate_hi_raw & 0x10 != 0 { 0x0fu8 } else { rate_hi_raw };
    let eg_shift    = rate_hi.wrapping_add(eg_add);
    let mut shift: u8 = 0;

    if nonzero {
        if rate_hi < 12 {
            if eg_state != 0 {
                shift = match eg_shift {
                    12 => 1,
                    13 => (rate_lo >> 1) & 0x01,
                    14 => rate_lo & 0x01,
                    _  => 0,
                };
            }
        } else {
            shift = (rate_hi & 0x03)
                .wrapping_add(EG_INCSTEP[rate_lo as usize][eg_timer_lo as usize]);
            if shift & 0x04 != 0 { shift = 0x03; }
            if shift == 0        { shift = eg_state; }
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
    if reset         { slot.eg_gen = EG_NUM_ATTACK;  }
    if slot.key == 0 { slot.eg_gen = EG_NUM_RELEASE; }
}

/// Set key-on type bits for a slot (OPL3_EnvelopeKeyOn).
pub(crate) fn envelope_key_on(slot: &mut Opl3Slot, key_type: u8) {
    slot.key |= key_type;
}

/// Clear key-on type bits for a slot (OPL3_EnvelopeKeyOff).
pub(crate) fn envelope_key_off(slot: &mut Opl3Slot, key_type: u8) {
    slot.key &= !key_type;
}

// ============================================================
// Step 1d — Phase generator
// ============================================================

/// Advance the phase accumulator for one slot and update rhythm-mode phase
/// outputs and the noise LFSR (OPL3_PhaseGenerate).
///
/// Borrow strategy: snapshot all read-only fields (channel f_num/block, slot
/// reg_vib/reg_mult/slot_num/pg_reset/pg_phase, chip vibpos/vibshift/rhy/noise
/// and all rm_* bits) into locals, compute results, then write back the two
/// slot fields and chip-level fields separately.
pub(crate) fn phase_generate(chip: &mut Opl3Chip, slot_idx: usize) {
    // --- Snapshot ---
    let ch        = chip.slot[slot_idx].channel_num as usize;
    let f_num_base = chip.channel[ch].f_num;
    let block     = chip.channel[ch].block;
    let reg_vib   = chip.slot[slot_idx].reg_vib;
    let reg_mult  = chip.slot[slot_idx].reg_mult;
    let slot_num  = chip.slot[slot_idx].slot_num;
    let pg_reset  = chip.slot[slot_idx].pg_reset;
    let pg_phase  = chip.slot[slot_idx].pg_phase;
    let vibpos    = chip.vibpos;
    let vibshift  = chip.vibshift;
    let rhy       = chip.rhy;
    let noise     = chip.noise;
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
    chip.slot[slot_idx].pg_phase     = new_pg_phase;
    chip.slot[slot_idx].pg_phase_out = pg_phase_out;
    chip.rm_hh_bit2 = new_rm_hh_bit2;
    chip.rm_hh_bit3 = new_rm_hh_bit3;
    chip.rm_hh_bit7 = new_rm_hh_bit7;
    chip.rm_hh_bit8 = new_rm_hh_bit8;
    chip.rm_tc_bit3 = new_rm_tc_bit3;
    chip.rm_tc_bit5 = new_rm_tc_bit5;
    chip.noise = new_noise;
}

# Full Sound Blaster 16 Emulation Plan

## Background and Scope

This plan adds a Sound Blaster 16 (SB16) emulation as a **new, separate device** alongside the existing standalone `Adlib`. The `Adlib` device is preserved unchanged — selecting `--sound-card adlib` continues to give pure OPL2 with no DSP/Mixer/MPU, which is useful as a compatibility baseline.

Selecting `--sound-card sb16` instantiates `SoundBlaster`, which:

1. Absorbs `SoundBlasterCdrom` into a unified struct (as anticipated in `cd-rom-sb.md`'s "Future" section).
2. Implements PCM audio output via 8-bit and 16-bit DMA.
3. Implements the SBPro/SB16 mixer chip.
4. Implements the OPL3 FM synthesizer at all SB I/O ports (reusing the existing `nuked_opl3` chip), including AdLib-compat ports `0x388`–`0x389`.
5. Implements an MPU-401 MIDI controller in UART mode.

**`Adlib` and `SoundBlaster` cannot be active simultaneously** — both claim `0x388`–`0x389`. The CLI enforces mutual exclusion.

The canonical hardware reference is `tmp/Bochs/bochs/iodev/sound/sb16.{h,cc}`.

### Development approach

Each phase follows test-driven development:
1. Write assembly test program(s) in `core/src/test_data/devices/sound_blaster/`
2. Write the Rust test(s) in `core/src/tests/devices/sound_blaster.rs`
3. Implement just enough to make those tests pass

Tests follow the same pattern as `core/src/tests/devices/adlib.rs`:
- Assembly program exits with code `0` on pass, non-zero on specific failure
- Rust test calls `run_test()` and optionally inspects ring buffers or memory

---

## Hardware Overview

### IO Port Map (base = `0x220`, configurable)

| Port | Direction | Function |
|------|-----------|----------|
| `base+0x0` | R | FM status (OPL3 chip 0) |
| `base+0x0` | W | FM address/index (OPL3 chip 0) |
| `base+0x1` | R/W | FM data (OPL3 chip 0) |
| `base+0x2` | R | FM status (OPL3 chip 1) |
| `base+0x2` | W | FM address/index (OPL3 chip 1) |
| `base+0x3` | R/W | FM data (OPL3 chip 1) |
| `base+0x4` | W | Mixer address index |
| `base+0x5` | R/W | Mixer data register |
| `base+0x6` | W | DSP reset (write `0x01` then `0x00`) |
| `base+0x8` | R | FM status (OPL2 compat, same as +0) |
| `base+0x8` | W | FM index (OPL2 compat) |
| `base+0x9` | R/W | FM data (OPL2 compat) |
| `base+0xA` | R | DSP read data port |
| `base+0xC` | R | DSP write buffer status (bit 7 = busy) |
| `base+0xC` | W | DSP write data/command |
| `base+0xE` | R | DSP read buffer status / IRQ source (bit 7 = data ready, acknowledges 8-bit IRQ) |
| `base+0xF` | R | Acknowledge 16-bit IRQ |

Additionally:
- `0x388`–`0x389` — AdLib-compat OPL2 access (maps to chip 0)
- `0x38A`–`0x38B` — OPL3 chip 1 (second pair)
- `0x330`–`0x331` — MPU-401 (data `0x330`, command/status `0x331`)

---

## Architecture in the Emulator

### Unified `SoundBlaster` struct

```rust
pub struct SoundBlaster {
    // --- Identity ---
    base_port: u16,            // default 0x220
    irq_line: u8,              // default 5 (IRQ5)
    dma8_channel: u8,          // default 1
    dma16_channel: u8,         // default 5

    // --- DSP ---
    dsp: SoundBlasterDsp,

    // --- OPL3 FM ---
    opl: SoundBlasterOpl,

    // --- Mixer ---
    mixer: SoundBlasterMixer,

    // --- MPU-401 ---
    mpu: SoundBlasterMpu,

    // --- CD-ROM interface (absorbed from SoundBlasterCdrom) ---
    cdrom: SoundBlasterCdromInner,

    // --- Audio output ---
    opl_out: PcmRingBuffer,    // FM synthesis output
    pcm_out: PcmRingBuffer,    // DSP DMA PCM output
}
```

`SoundBlaster` implements three traits:
- `Device` — all IO port dispatch
- `SoundCard` — cycle-accurate OPL3 advancement and DSP timing
- `CdromController` — disc load/eject and IRQ delegation

`Rc<RefCell<SoundBlaster>>` is registered once in `Bus::add_sound_blaster()`, which stores it as both `sound_card` and `cdrom_controller`, following the inner-Rc pattern described in `cd-rom-sb.md`.

---

## ✅ Phase 1 — Example Program

**Complete.** `examples/sound_blaster.asm` and `examples/sound_blaster.com` are written and built.

**File**: `examples/sound_blaster.asm`

Modelled on `examples/adlib.asm`. The program is a self-contained COM binary that exercises three SB-specific features that the standalone `Adlib` card cannot provide: the DSP subsystem, the mixer, and direct PCM digital audio output. It also plays FM tones through the SB's OPL ports (`0x220/0x221`) to confirm that both signal paths work at once.

```
; Build:  nasm -f bin sound_blaster.asm -o sound_blaster.com
; Run:    cargo run -p oxide86-native-gui -- --sound-card sb16 sound_blaster.com
;         cargo run -p oxide86-native-cli -- --sound-card sb16 sound_blaster.com
```

### Sections

| Section | SB-specific feature exercised |
|---------|-------------------------------|
| DSP detection | Reset handshake (`0x01`/`0x00` → `0xAA` ready byte) and version query (`0xE1`) — the entire DSP protocol is absent from AdLib |
| Mixer setup | Write master volume register `0x22` at `0x224`/`0x225` — the mixer chip does not exist on AdLib |
| Direct DAC PCM | DSP command `0x10` (Direct DAC) to output a software-timed square-wave chirp — digitized audio playback is the defining SB feature |
| OPL via SB port | Two-note FM sequence via `0x220`/`0x221` (not `0x388`/`0x389`) — confirms the SB card's own OPL port pair |

### Full listing

```nasm
; sound_blaster.asm — Sound Blaster 16 detection and feature demo
;
; Demonstrates three features exclusive to the SB (vs. standalone AdLib):
;   1. DSP detection via the reset handshake and version query (0xE1)
;   2. Mixer chip — sets master volume to maximum via registers 0x22/0x30/0x31
;   3. Direct DAC PCM — plays a frequency-swept chirp using DSP command 0x10
;
; Also plays two FM notes via the SB's own OPL ports (0x220/0x221) to confirm
; that the OPL and DSP subsystems coexist on the same card.
;
; All text is printed before audio starts so the output is readable even when
; the emulator is paused immediately after exit.
;
; NOTE: Ports > 0xFF require the DX register form of IN/OUT.
;
; Build:  nasm -f bin sound_blaster.asm -o sound_blaster.com
; Run:    cargo run -p oxide86-native-gui -- --sound-card sb16 sound_blaster.com
;         cargo run -p oxide86-native-cli -- --sound-card sb16 sound_blaster.com

[CPU 8086]
org 0x100

SB_BASE equ 0x220

; ─── Section 1: DSP detection ────────────────────────────────────────────────
;
; Write 0x01 to the DSP reset port (base+6), wait briefly, write 0x00.
; The DSP acknowledges by placing 0xAA in the read FIFO; bit 7 of base+0xE
; goes high to signal data ready.  Then send command 0xE1 to read the version.

start:
    ; Assert reset
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al

    mov cx, 100
.reset_delay:
    nop
    loop .reset_delay

    ; Deassert reset
    xor al, al
    out dx, al

    ; Poll base+E bit 7 until the DSP places 0xAA in the FIFO
    mov cx, 4000
.poll_aa:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_aa
    loop .poll_aa
    jmp .not_found          ; timeout — no DSP

.read_aa:
    mov dx, SB_BASE + 0xA
    in al, dx               ; should be 0xAA
    cmp al, 0xAA
    jne .not_found

    ; Query version (command 0xE1)
    call dsp_write
    db 0xE1

    ; Read major version byte
    call dsp_read           ; result in AL
    mov [dsp_major], al

    ; Read minor version byte
    call dsp_read
    mov [dsp_minor], al

    ; Convert major to ASCII and patch message
    mov al, [dsp_major]
    add al, '0'
    mov [msg_version + 18], al   ; "DSP version X.Y..."
    mov al, [dsp_minor]
    add al, '0'
    mov [msg_version + 20], al

    ; Print detection success
    mov dx, msg_found
    mov ah, 0x09
    int 0x21

    ; Print version line
    mov dx, msg_version
    mov ah, 0x09
    int 0x21

    jmp .detected

.not_found:
    mov dx, msg_not_found
    mov ah, 0x09
    int 0x21
    jmp .done

.detected:

; ─── Section 2: Mixer — set master volume to maximum ─────────────────────────
;
; SBPro mixer: register 0x22 = master volume (nibble L | nibble R).
; SB16 mixer:  registers 0x30 (master L, 5-bit) and 0x31 (master R, 5-bit).
; Write both for maximum compatibility.

    ; SBPro-style master volume: 0xFF = both channels full
    mov dx, SB_BASE + 4     ; mixer address port
    mov al, 0x22
    out dx, al
    mov dx, SB_BASE + 5     ; mixer data port
    mov al, 0xFF
    out dx, al

    ; SB16-style master L (reg 0x30): top 5 bits = volume, 0xF8 = max
    mov dx, SB_BASE + 4
    mov al, 0x30
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xF8
    out dx, al

    ; SB16-style master R (reg 0x31)
    mov dx, SB_BASE + 4
    mov al, 0x31
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xF8
    out dx, al

    mov dx, msg_mixer
    mov ah, 0x09
    int 0x21

; ─── Section 3: Speaker on ───────────────────────────────────────────────────
;
; DSP command 0xD1 enables the DAC output.  Without this, some SB models
; output silence even during a DMA transfer.  AdLib has no speaker control.

    call dsp_write
    db 0xD1                 ; speaker on

; ─── Section 4: Direct DAC PCM — frequency-swept chirp ───────────────────────
;
; DSP command 0x10 accepts one byte and outputs it immediately to the DAC.
; There is no hardware timing: the sample rate is entirely software-controlled
; by the delay between consecutive 0x10 commands.
;
; Waveform: square wave (0xC0 / 0x40 = ±64 from midpoint 0x80).
; Sweep: 8 tones from ~880 Hz down to ~110 Hz by doubling the half-period.
;
; At 8 MHz CPU, one NOP ≈ 0.125 µs.  Half-period in cycles:
;   880 Hz → T/2 ≈ 568 µs ≈ 4544 cycles → CX ≈ 909 NOPs (5 cycles/NOP)
;   110 Hz → T/2 ≈ 4545 µs → CX ≈ 7272 NOPs
;
; The 0x10 command write + busy poll adds ~30 cycles overhead per half-cycle,
; negligible at these frequencies.

    mov dx, msg_pcm
    mov ah, 0x09
    int 0x21

    mov bx, 900             ; initial half-period NOP count (~880 Hz at 8 MHz)
    mov si, 8               ; number of tones in the sweep

.tone_loop:
    mov di, 60              ; half-cycles per tone (~30 full cycles = ~34 ms each)

.halfcycle_high:
    ; Output high sample (0xC0 = loud positive, unsigned 8-bit)
    call dsp_write
    db 0x10
    call dsp_write_al
    db 0xC0

    ; Hold for BX NOPs to set the frequency
    mov cx, bx
.dly_hi:
    nop
    loop .dly_hi

.halfcycle_low:
    ; Output low sample (0x40 = loud negative)
    call dsp_write
    db 0x10
    call dsp_write_al
    db 0x40

    mov cx, bx
.dly_lo:
    nop
    loop .dly_lo

    dec di
    jnz .halfcycle_high

    ; Double the half-period → halve the frequency for the next tone
    shl bx, 1
    dec si
    jnz .tone_loop

    ; Brief silence (0x80 = midpoint = silent for unsigned PCM)
    call dsp_write
    db 0x10
    call dsp_write_al
    db 0x80

; ─── Section 5: Speaker off ──────────────────────────────────────────────────

    call dsp_write
    db 0xD3                 ; speaker off

; ─── Section 6: OPL FM via the SB's own ports (0x220/0x221) ─────────────────
;
; The SB exposes its OPL3 chip at base+0 / base+1 in addition to the AdLib-
; compat ports 0x388/0x389.  This section plays the same two-note sequence as
; examples/adlib.asm but accesses the chip through the SB port pair, proving
; that base+0/+1 are live.

    mov dx, msg_opl
    mov ah, 0x09
    int 0x21

    ; Enable waveform select (OPL reg 0x01 bit 5)
    call sb_opl_write
    db 0x01, 0x20

    ; Modulator (slot 0): EG=1 MULT=1
    call sb_opl_write
    db 0x20, 0x21
    ; Modulator TL=16
    call sb_opl_write
    db 0x40, 0x10
    ; Modulator AR=15 DR=0
    call sb_opl_write
    db 0x60, 0xF0
    ; Modulator SL=0 RR=7
    call sb_opl_write
    db 0x80, 0x07

    ; Carrier (slot 3): EG=1 MULT=1
    call sb_opl_write
    db 0x23, 0x21
    ; Carrier TL=0 (full volume)
    call sb_opl_write
    db 0x43, 0x00
    ; Carrier AR=15 DR=0
    call sb_opl_write
    db 0x63, 0xF0
    ; Carrier SL=0 RR=7
    call sb_opl_write
    db 0x83, 0x07

    ; Channel 0 feedback/algorithm
    call sb_opl_write
    db 0xC0, 0x08

    ; Note 1: A4 (440 Hz), key_on
    call sb_opl_write
    db 0xA0, 0x44
    call sb_opl_write
    db 0xB0, 0x32

    call delay_long

    ; Note 2: D5 (~587 Hz), key_on
    call sb_opl_write
    db 0xA0, 0x08
    call sb_opl_write
    db 0xB0, 0x33

    call delay_long

    ; Key off
    call sb_opl_write
    db 0xB0, 0x13

.done:
    mov ah, 0x4C
    xor al, al
    int 0x21

; ─── Subroutine: dsp_write ───────────────────────────────────────────────────
; Reads one inline byte after CALL and writes it to the DSP command port,
; polling the write-buffer-status port (base+C bit 7) until ready.
; Corrupts: AX, BX, CX, DX
dsp_write:
    pop bx                  ; BX = inline data address
    mov al, [bx]
    inc bx
    push bx                 ; push updated return address

    ; Poll base+C bit 7: 0=ready, 1=busy
    mov cx, 4000
.dw_poll:
    mov dx, SB_BASE + 0xC
    in al, dx
    test al, 0x80
    jz .dw_send
    loop .dw_poll
.dw_send:
    ; Retrieve the byte to send (BX already points past it; reload from [bx-1])
    ; We need the inline byte back — it was loaded into AL before the push;
    ; since AL may be clobbered by the poll, reload it.
    mov bx, sp
    mov bx, [bx]            ; BX = updated return address (points after inline byte)
    dec bx                  ; back one byte = the inline byte
    mov al, [bx]
    mov dx, SB_BASE + 0xC
    out dx, al
    ret

; ─── Subroutine: dsp_write_al ────────────────────────────────────────────────
; Like dsp_write but for a second inline byte (used with 0x10 Direct DAC).
; This is a simple alias — the inline-byte convention is the same.
dsp_write_al equ dsp_write

; ─── Subroutine: dsp_read ────────────────────────────────────────────────────
; Returns the next DSP read-FIFO byte in AL.
; Polls base+E bit 7 until data available.
; Corrupts: AX, CX, DX
dsp_read:
    mov cx, 4000
.dr_poll:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .dr_read
    loop .dr_poll
.dr_read:
    mov dx, SB_BASE + 0xA
    in al, dx
    ret

; ─── Subroutine: sb_opl_write ────────────────────────────────────────────────
; Reads two inline bytes [reg, val] and writes them to the SB OPL port pair
; (base+0 / base+1), with appropriate inter-write delays.
; Corrupts: AX, BX, CX, DX
sb_opl_write:
    pop bx
    mov al, [bx]            ; register index
    mov dx, SB_BASE + 0     ; OPL address port
    out dx, al
    ; Short address-setup delay (~3 µs at 8 MHz = 24 cycles)
    mov cx, 5
.opl_addr_dly:
    nop
    loop .opl_addr_dly
    mov al, [bx+1]          ; data value
    mov dx, SB_BASE + 1     ; OPL data port
    out dx, al
    ; Post-write delay (~23 µs = 184 cycles)
    mov cx, 37
.opl_data_dly:
    nop
    loop .opl_data_dly
    add bx, 2
    push bx
    ret

; ─── Subroutine: delay_long ──────────────────────────────────────────────────
; Busy-wait ~0.5 seconds at 8 MHz (roughly 4 million cycles).
delay_long:
    push cx
    push dx
    mov dx, 60
.dl_outer:
    mov cx, 0xFFFF
.dl_inner:
    nop
    loop .dl_inner
    dec dx
    jnz .dl_outer
    pop dx
    pop cx
    ret

; ─── Data ────────────────────────────────────────────────────────────────────
dsp_major     db 0
dsp_minor     db 0

msg_found     db 'Sound Blaster detected.', 0x0D, 0x0A, '$'
msg_not_found db 'Sound Blaster not found (no DSP ready byte).', 0x0D, 0x0A, '$'
msg_version   db 'DSP version X.Y (SB16 = 4.5)', 0x0D, 0x0A, '$'
;                            ^^ patched at runtime (offsets 18 and 20)
msg_mixer     db 'Mixer: master volume set to maximum.', 0x0D, 0x0A, '$'
msg_pcm       db 'PCM: playing frequency-swept chirp via Direct DAC...', 0x0D, 0x0A, '$'
msg_opl       db 'OPL: playing two FM notes via SB port (0x220/0x221)...', 0x0D, 0x0A, '$'
```

### Notes on `dsp_write` design

The inline-byte convention (identical to `adlib_write_reg` in `adlib.asm`) keeps call sites compact: one `call dsp_write` + one `db` byte per command send. The `dsp_write_al` alias re-uses the same routine for the data byte after a `0x10` Direct DAC command — both writes follow the same busy-poll protocol.

### What this exercises that `adlib.asm` cannot

| Feature | AdLib | Sound Blaster (this demo) |
|---------|-------|---------------------------|
| DSP reset / ready handshake | — | ✓ Section 1 |
| DSP version query (`0xE1`) | — | ✓ Section 1 |
| Mixer chip (ports `0x224`/`0x225`) | — | ✓ Section 2 |
| Speaker on/off (`0xD1`/`0xD3`) | — | ✓ Sections 3 & 5 |
| Digitized PCM audio (Direct DAC `0x10`) | — | ✓ Section 4 |
| OPL via SB base port (`0x220/0x221`) | — | ✓ Section 6 |
| OPL via AdLib port (`0x388/0x389`) | ✓ | (not used here — see `adlib.asm`) |

---

## ✅ Phase 2 — Absorb `SoundBlasterCdrom`

Create the `SoundBlaster` struct as the thinnest possible wrapper that absorbs the existing `SoundBlasterCdrom` device and stubs everything else. Goal: all existing CD-ROM tests pass unchanged; everything else compiles and returns safe defaults.

### What this phase does

- Move `SoundBlasterCdrom` state verbatim into a `SoundBlasterCdromInner` sub-struct inside `SoundBlaster`.
- `Device::io_read_u8` / `io_write_u8`: route ports `0x230`–`0x233` (CD-ROM) to `cdrom`; return `None`/`false` for all other ports (no SB ports claimed yet).
- `SoundCard::advance_to_cycle` / `next_sample_cycle`: no-op stubs (return `u32::MAX` for next sample).
- `CdromController`: delegate `load_disc`, `eject_disc`, `take_pending_irq`, `irq_line` straight to the inner struct.
- Remove `core/src/devices/sound_blaster_cdrom.rs`; update `core/src/devices/mod.rs`.
- Add `Bus::add_sound_blaster()` (see below) and `Computer::add_sound_blaster()`.

### Tests first

Re-run the existing `SoundBlasterCdrom` tests but registering a `SoundBlaster` via `computer.add_sound_blaster()` instead of `computer.add_cdrom_controller()`. All existing CD-ROM assembly tests (in `core/src/test_data/`) must still pass without modification.

**`core/src/tests/devices/sound_blaster.rs`** (initial):

```rust
use crate::{
    devices::sound_blaster::SoundBlaster,
    tests::{run_test, TEST_OFFSET, TEST_SEGMENT},
};

fn create_sb_computer() -> (
    crate::computer::Computer,
    std::sync::Arc<std::sync::RwLock<crate::video::VideoBuffer>>,
) {
    make_computer!()
}

/// CD-ROM NOP command still works after absorbing SoundBlasterCdrom into SoundBlaster.
#[test_log::test]
pub(crate) fn cdrom_nop_via_unified_card() {
    run_test(
        "devices/sound_blaster/cdrom_nop",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}
```

### `SoundBlaster` struct (Phase 2 only)

```rust
pub struct SoundBlaster {
    cdrom: SoundBlasterCdromInner,  // verbatim move from SoundBlasterCdrom
    cpu_freq: u64,
    // all other fields (dsp, opl, mixer, mpu, pcm_out, opl_out) added in later phases
}

impl SoundBlaster {
    pub fn new(cpu_freq: u64) -> Self {
        Self { cdrom: SoundBlasterCdromInner::new(), cpu_freq }
    }
}
```

### Bus registration

```rust
pub(crate) fn add_sound_blaster<T: Device + SoundCard + CdromController + 'static>(
    &mut self,
    device: T,
) {
    let rc = Rc::new(RefCell::new(device));
    self.devices.push(rc.clone());
    self.sound_card = Some(rc.clone());
    self.cdrom_controller = Some(rc.clone());
    self.pic.borrow_mut().set_cdrom(rc.clone());
    // DMA slots wired in Phase 7 when PCM DMA is implemented
}
```

---

## ✅ Phase 3 — CLI, GUI, and WASM Updates

### `native-common/src/cli.rs`

| Flag | Default | Description |
|------|---------|-------------|
| `--sound-card` | `none` | `none`, `adlib`, `sb16` — **adlib and sb16 are independent** |
| `--sound-blaster-port` | `0x220` | DSP/Mixer/OPL base port (SB16 only) |
| `--sound-blaster-irq` | `5` | IRQ line: 2, 5, 7, or 10 (SB16 only) |
| `--sound-blaster-dma8` | `1` | 8-bit DMA channel: 0, 1, or 3 (SB16 only) |
| `--sound-blaster-dma16` | `5` | 16-bit DMA channel: 5, 6, or 7 (SB16 only) |
| `--sound-blaster-cd-port` | `0x230` | CD-ROM interface base port (SB16 only) |
| `--disable-sound-blaster-cd` | false | Disable the CD-ROM sub-device (SB16 only) |
| `--cdrom` | — | ISO image path |

`SoundCardType::parse()` gains `"sb16"` and `"sb"` as recognized values. `"adlib"` maps to `SoundCardType::AdLib` as before and instantiates the standalone `Adlib` device. Selecting `sb16` instantiates `SoundBlaster` and must warn (or error) if `--disable-sound-blaster-cd` is omitted while a CD-ROM device is already registered.

### `native-common/src/lib.rs`

```rust
match cli.sound_card_type() {
    SoundCardType::None => {}
    SoundCardType::AdLib => {
        // Unchanged — existing Adlib path
        computer.add_sound_card(Adlib::new(cpu_freq));
    }
    SoundCardType::SoundBlaster16 => {
        let sb = SoundBlaster::new_with_config(SoundBlasterConfig { ... });
        let opl_consumer = sb.opl_consumer();
        let pcm_consumer = sb.pcm_consumer();
        computer.add_sound_blaster(sb);
        // Wire up Rodio with both consumers ...
    }
}
```

### Native GUI

No structural changes — `InsertCdrom` / `EjectCdrom` already call through `CdromControllerRef`.

### WASM

Add `sound_blaster_port` to `WasmComputerConfig`. Instantiate `SoundBlaster` when set. Existing `insert_cdrom` / `eject_cdrom` JS bindings are unchanged.

---

## Phase 4 — DSP: Detection and Basic Commands

**File**: `core/src/devices/sound_blaster.rs` (new)

### Tests first

**`core/src/test_data/devices/sound_blaster/dsp_reset.asm`**

```nasm
; dsp_reset.asm — DSP reset and version check
;
; Standard SB16 detection sequence:
;   1. Write 0x01 to reset port (base+6), wait, write 0x00
;   2. Poll read-data port (base+A) until bit 7 of base+E is set
;   3. Read byte — must be 0xAA (DSP ready)
;   4. Send command 0xE1 (version), read two bytes
;   5. Verify major=0x04, minor=0x05
;
; Exit codes: 0=pass, 1=DSP ready byte was not 0xAA, 2=wrong major, 3=wrong minor

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; Reset DSP: write 1
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al

    ; Short delay
    mov cx, 100
.delay1:
    nop
    loop .delay1

    ; Write 0 to complete reset
    mov al, 0x00
    out dx, al

    ; Poll base+E bit 7 until data ready (up to ~2000 cycles)
    mov cx, 2000
.poll_ready:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_aa
    loop .poll_ready

.read_aa:
    ; Read the ready byte from base+A
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0xAA
    je .send_version
    mov al, 0x01        ; fail: wrong ready byte
    jmp .exit

.send_version:
    ; Send version command 0xE1
    mov dx, SB_BASE + 0xC
    mov al, 0xE1
    out dx, al

    ; Poll and read major version
    mov cx, 2000
.poll_major:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_major
    loop .poll_major
.read_major:
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0x04
    je .read_minor_byte
    mov al, 0x02        ; fail: wrong major
    jmp .exit

.read_minor_byte:
    ; Poll and read minor version
    mov cx, 2000
.poll_minor:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .read_minor
    loop .poll_minor
.read_minor:
    mov dx, SB_BASE + 0xA
    in al, dx
    cmp al, 0x05
    je .pass
    mov al, 0x03        ; fail: wrong minor
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
```

**`core/src/test_data/devices/sound_blaster/dsp_speaker.asm`**

```nasm
; dsp_speaker.asm — Speaker on/off commands and status readback
;
; Exit codes: 0=pass, 1=wrong initial status, 2=wrong on status, 3=wrong off status

[CPU 8086]
org 0x100

SB_BASE equ 0x220

%macro sb_cmd 1
    mov dx, SB_BASE + 0xC
    mov al, %1
    out dx, al
%endmacro

%macro sb_read 0
    mov cx, 2000
%%poll:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz %%done
    loop %%poll
%%done:
    mov dx, SB_BASE + 0xA
    in al, dx
%endmacro

start:
    ; Reset DSP first
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al
    mov cx, 100
.r: nop
    loop .r
    xor al, al
    out dx, al
    mov cx, 2000
.pw:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .drain
    loop .pw
.drain:
    mov dx, SB_BASE + 0xA
    in al, dx

    ; Check default speaker status (off = 0x00)
    sb_cmd 0xD8
    sb_read
    cmp al, 0x00
    je .turn_on
    mov al, 0x01
    jmp .exit

.turn_on:
    ; Speaker on
    sb_cmd 0xD1
    sb_cmd 0xD8
    sb_read
    cmp al, 0xFF
    je .turn_off
    mov al, 0x02
    jmp .exit

.turn_off:
    ; Speaker off
    sb_cmd 0xD3
    sb_cmd 0xD8
    sb_read
    cmp al, 0x00
    je .pass
    mov al, 0x03
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
```

**`core/src/tests/devices/sound_blaster.rs`** (initial, grows each phase)

```rust
use crate::{
    devices::sound_blaster::SoundBlaster,
    tests::{run_test, TEST_OFFSET, TEST_SEGMENT},
};

fn create_sb_computer() -> (
    crate::computer::Computer,
    std::sync::Arc<std::sync::RwLock<crate::video::VideoBuffer>>,
) {
    make_computer!()
}

/// DSP reset handshake returns 0xAA; version query returns 0x04/0x05.
#[test_log::test]
pub(crate) fn dsp_reset_and_version() {
    run_test(
        "devices/sound_blaster/dsp_reset",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}

/// Speaker on (0xD1), off (0xD3), and status query (0xD8) round-trip correctly.
#[test_log::test]
pub(crate) fn dsp_speaker_control() {
    run_test(
        "devices/sound_blaster/dsp_speaker",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}
```

### DSP substate

```rust
struct SoundBlasterDsp {
    reset_seq: u8,               // tracks 0x01 → 0x00 reset handshake
    cmd: Option<u8>,             // current command opcode
    cmd_remaining: u8,           // parameter bytes still expected
    out_buf: VecDeque<u8>,       // bytes waiting in the DSP read FIFO
    irq_pending_8: bool,
    irq_pending_16: bool,
    speaker_on: bool,
    test_reg: u8,
}
```

### Commands in Phase 4

| Opcode | Name | Params | Notes |
|--------|------|--------|-------|
| `0xD1` | Speaker On | 0 | Sets `speaker_on` |
| `0xD3` | Speaker Off | 0 | Clears `speaker_on` |
| `0xD8` | Speaker Status | 0 | Returns `0xFF` (on) or `0x00` (off) |
| `0xE0` | DSP Identification | 1 | Returns `~param` |
| `0xE1` | Version | 0 | Returns two bytes: major `0x04`, minor `0x05` |
| `0xE3` | Copyright String | 0 | Streams NUL-terminated copyright string |
| `0xE4` | Write Test Reg | 1 | Stores byte in `test_reg` |
| `0xE8` | Read Test Reg | 0 | Returns `test_reg` |
| `0xF2` | Force 8-bit IRQ | 0 | Sets `irq_pending_8`, notifies PIC |
| `0xF3` | Force 16-bit IRQ | 0 | Sets `irq_pending_16`, notifies PIC |

### Reset sequence

Write `0x01` to `base+0x6`, then `0x00`. After the `0x00` write the DSP enqueues `0xAA` in the read FIFO. Bit 7 of `base+0xE` goes high to signal data available.

### PIC wiring

`SoundBlaster` implements `CdromController::irq_line()` which the PIC already queries. A single IRQ line covers DSP, MPU, and CD-ROM — no PIC changes needed.

---

## Phase 5 — OPL3 FM Integration

The SB16 exposes the same `nuked_opl3` OPL3 chip at multiple port ranges. This phase makes the SB's OPL ports respond identically to the standalone `Adlib`.

### Tests first

**`core/src/test_data/devices/sound_blaster/opl_detect.asm`**

AdLib timer detection, but accessed via the SB base port (`0x220/0x221`) instead of `0x388/0x389`. Identical logic to `detect_adlib.asm` — just the port numbers change. Exit 0=pass, 1=pre-status not clear, 2=timer did not fire.

**`core/src/test_data/devices/sound_blaster/opl_adlib_compat.asm`**

Same detection but using the AdLib-compat ports `0x388/0x389` while a SB16 is registered (not a standalone Adlib). Verifies that the SB's OPL chip responds at both port ranges simultaneously.

**`core/src/test_data/devices/sound_blaster/opl_play_tone.asm`**

Enable OPL3 voice via `0x220/0x221`, play a brief tone, exit. Mirrors `adlib_play_tone.asm` but through the SB port pair. Used by the Rust ring-buffer test.

**Tests added to `core/src/tests/devices/sound_blaster.rs`**:

```rust
/// OPL timer detection works at the SB base port (0x220/0x221).
#[test_log::test]
pub(crate) fn opl_detect_via_sb_port() {
    run_test(
        "devices/sound_blaster/opl_detect",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}

/// AdLib-compat ports (0x388/0x389) still work when SB16 is the active card.
#[test_log::test]
pub(crate) fn opl_adlib_compat() {
    run_test(
        "devices/sound_blaster/opl_adlib_compat",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}

/// Playing an OPL voice via the SB port produces non-zero PCM samples.
#[test_log::test]
pub(crate) fn opl_tone_produces_samples() {
    let program_data = load_program_data("devices/sound_blaster/opl_play_tone");
    let (mut computer, _video_buffer) = create_sb_computer();
    let sb = SoundBlaster::new(8_000_000);
    let opl_consumer = sb.opl_consumer();
    computer.add_sound_blaster(sb);
    computer.load_program(&program_data, TEST_SEGMENT, TEST_OFFSET).unwrap();
    computer.run();
    assert_eq!(Some(0), computer.get_exit_code());
    let available = opl_consumer.available();
    assert!(available > 0);
    let mut samples = vec![0.0f32; available];
    opl_consumer.drain_into(&mut samples);
    assert!(samples.iter().any(|&s| s != 0.0));
}
```

### OPL substate

```rust
struct SoundBlasterOpl {
    chip: Opl3Chip,
    pending_address: [u8; 2],      // pending index for chip 0 and chip 1
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
    opl_out: PcmRingBuffer,
}
```

This is the same logic as the existing `Adlib` struct, extracted as a sub-struct. The SB16 maps the OPL chip to:

- `base+0x0` / `base+0x1` — OPL3 chip 0
- `base+0x2` / `base+0x3` — OPL3 chip 1
- `base+0x8` / `base+0x9` — OPL2 compat alias (chip 0)
- `0x388` / `0x389` — AdLib compat (chip 0)
- `0x38A` / `0x38B` — OPL3 chip 1 alias

**`Adlib` remains registered only when `--sound-card adlib` is selected.** When `--sound-card sb16` is selected, only `SoundBlaster` is registered. Both cannot be active simultaneously.

---

## Phase 6 — Mixer Chip

**Ports**: `base+0x4` (write index), `base+0x5` (read/write data)

### Tests first

**`core/src/test_data/devices/sound_blaster/mixer_readwrite.asm`**

```nasm
; mixer_readwrite.asm — Mixer register round-trip
;
; 1. Write 0x22 to mixer index (master volume)
; 2. Write 0xCC to data port
; 3. Re-select 0x22, read data port
; 4. Verify returned value is 0xCC (or masked hardware value)
;
; Also verifies IRQ config register 0x80 reads back after write.
;
; Exit: 0=pass, 1=master vol mismatch, 2=IRQ reg mismatch

[CPU 8086]
org 0x100

SB_BASE equ 0x220

start:
    ; Write master volume
    mov dx, SB_BASE + 4
    mov al, 0x22
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0xCC
    out dx, al

    ; Read back
    mov dx, SB_BASE + 4
    mov al, 0x22
    out dx, al
    mov dx, SB_BASE + 5
    in al, dx
    cmp al, 0xCC
    je .irq_test
    mov al, 0x01
    jmp .exit

.irq_test:
    ; Write IRQ select: IRQ5 = bit 2
    mov dx, SB_BASE + 4
    mov al, 0x80
    out dx, al
    mov dx, SB_BASE + 5
    mov al, 0x04
    out dx, al

    ; Read back
    mov dx, SB_BASE + 4
    mov al, 0x80
    out dx, al
    mov dx, SB_BASE + 5
    in al, dx
    cmp al, 0x04
    je .pass
    mov al, 0x02
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
```

**Tests added to `core/src/tests/devices/sound_blaster.rs`**:

```rust
/// Mixer register write/read round-trips correctly; IRQ config register persists.
#[test_log::test]
pub(crate) fn mixer_readwrite() {
    run_test(
        "devices/sound_blaster/mixer_readwrite",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}
```

### Mixer implementation

Use a `[u8; 256]` array (initialized to hardware defaults) for all register values.

Key registers:

| Register | Description |
|----------|-------------|
| `0x00` | Reset (write any) |
| `0x04` | Voice volume (4-bit L/R) |
| `0x22` | Master volume (4-bit L/R) |
| `0x26` | FM volume (4-bit L/R) |
| `0x28` | CD volume (4-bit L/R) |
| `0x2E` | Line volume (4-bit L/R) |
| `0x30`–`0x31` | Master vol L/R (5-bit, SB16) |
| `0x32`–`0x33` | Voice vol L/R (5-bit, SB16) |
| `0x80` | IRQ select (bit 1=IRQ2, 2=IRQ5, 3=IRQ7, 4=IRQ10) |
| `0x81` | DMA select (bit 0=DMA0, 1=DMA1, 2=DMA3, 5=DMA5, 6=DMA6, 7=DMA7) |
| `0x82` | IRQ status (bit 0=8-bit, 1=16-bit, 2=MPU; read-only) |

For the initial implementation, store all register values in the array and return them on read without applying gain scaling to audio output. Gain scaling is a follow-on.

---

## Phase 7 — PCM Output via DMA

### Tests first

**`core/src/test_data/devices/sound_blaster/dsp_pcm_single.asm`**

```nasm
; dsp_pcm_single.asm — 8-bit single-cycle DMA PCM playback
;
; 1. Reset DSP
; 2. Install IRQ5 handler that sets a flag and sends EOI
; 3. Set time constant for 11025 Hz (TC = 165)
; 4. Program DMA channel 1: 256 bytes at a known address, READ mode
; 5. Issue DSP command 0x14 (single-cycle 8-bit DMA, 255 bytes)
; 6. Enable interrupts, wait for IRQ flag to be set (up to N iterations)
; 7. Exit 0 if IRQ fired, 1 if it did not
;
; DMA1 channel 1 ports: addr=0x02, count=0x03, mode=0x0B, mask=0x0A, page=0x83, flip-flop=0x0C

[CPU 8086]
org 0x100

SB_BASE equ 0x220

irq_fired db 0

start:
    ; --- Install IRQ5 handler ---
    push es
    xor ax, ax
    mov es, ax
    ; INT 0x0D = IRQ5 → vector at 4*0x0D = 0x34
    mov word [es:0x34], irq_handler
    mov word [es:0x36], cs
    pop es

    ; --- Reset DSP ---
    mov dx, SB_BASE + 6
    mov al, 0x01
    out dx, al
    mov cx, 100
.rst: nop
    loop .rst
    xor al, al
    out dx, al
    mov cx, 2000
.pw:
    mov dx, SB_BASE + 0xE
    in al, dx
    test al, 0x80
    jnz .drain
    loop .pw
.drain:
    mov dx, SB_BASE + 0xA
    in al, dx      ; consume 0xAA

    ; --- Set time constant for 11025 Hz mono ---
    ; TC = 256 - (1000000 / 11025) ≈ 256 - 90 = 166
    mov dx, SB_BASE + 0xC
    mov al, 0x40
    out dx, al
    mov al, 166
    out dx, al

    ; --- Fill audio buffer at 0x2000:0x0000 with 0x80 (silence, unsigned) ---
    push es
    mov ax, 0x2000
    mov es, ax
    xor di, di
    mov cx, 256
    mov al, 0x80
    rep stosb
    pop es

    ; --- Program DMA1 channel 1 ---
    ; Mask channel 1
    mov al, 0x05       ; ch=1, set-mask
    out 0x0A, al

    ; Flip-flop reset
    xor al, al
    out 0x0C, al

    ; Address = 0x0000 (physical = 0x2000*16 + 0x0000 = 0x20000)
    xor al, al
    out 0x02, al       ; low
    out 0x02, al       ; high

    ; Page = 0x02 (segment 0x2000 → physical page byte)
    mov al, 0x02
    out 0x83, al

    ; Flip-flop reset before count
    xor al, al
    out 0x0C, al

    ; Count = 255 (256 bytes total)
    mov al, 0xFF
    out 0x03, al       ; low
    xor al, al
    out 0x03, al       ; high

    ; Mode: single-cycle, increment, no-auto-init, READ (mem→device), ch1 = 0x49
    mov al, 0x49
    out 0x0B, al

    ; Unmask channel 1
    mov al, 0x01       ; ch=1, clear-mask
    out 0x0A, al

    ; --- Unmask IRQ5 at PIC ---
    in al, 0x21
    and al, 0xDF       ; clear bit 5
    out 0x21, al

    ; --- Issue DSP single-cycle 8-bit DMA command ---
    mov dx, SB_BASE + 0xC
    mov al, 0x14
    out dx, al
    ; Length = count - 1 = 255 (lo byte then hi byte)
    mov al, 0xFF
    out dx, al
    xor al, al
    out dx, al

    ; --- Enable CPU interrupts and wait ---
    sti
    mov cx, 0xFFFF
.wait:
    cmp byte [irq_fired], 1
    je .pass
    loop .wait

    ; Timeout
    mov al, 0x01
    jmp .exit

.pass:
    xor al, al
.exit:
    ; Mask IRQ5 again
    in al, 0x21
    or al, 0x20
    out 0x21, al
    mov ah, 0x4C
    int 0x21

irq_handler:
    mov byte [cs:irq_fired], 1
    ; Acknowledge 8-bit IRQ by reading DSP read-status port
    mov dx, SB_BASE + 0xE
    in al, dx
    ; EOI
    mov al, 0x20
    out 0x20, al
    iret
```

**`core/src/test_data/devices/sound_blaster/dsp_pcm_samples.asm`**

Plays 256 bytes of a known sawtooth waveform (values `0x00`–`0xFF`) via 8-bit unsigned DMA. Used by the Rust test to verify non-zero samples in the ring buffer.

**Tests added to `core/src/tests/devices/sound_blaster.rs`**:

```rust
/// IRQ fires after single-cycle 8-bit DMA completes.
#[test_log::test]
pub(crate) fn dsp_pcm_irq_fires() {
    run_test(
        "devices/sound_blaster/dsp_pcm_single",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}

/// 8-bit unsigned PCM DMA transfer pushes non-zero samples to the ring buffer.
#[test_log::test]
pub(crate) fn dsp_pcm_samples_in_ring_buffer() {
    let program_data = load_program_data("devices/sound_blaster/dsp_pcm_samples");
    let (mut computer, _video_buffer) = create_sb_computer();
    let sb = SoundBlaster::new(8_000_000);
    let pcm_consumer = sb.pcm_consumer();
    computer.add_sound_blaster(sb);
    computer.load_program(&program_data, TEST_SEGMENT, TEST_OFFSET).unwrap();
    computer.run();
    assert_eq!(Some(0), computer.get_exit_code());
    let available = pcm_consumer.available();
    assert!(available > 0, "ring buffer must contain samples after PCM DMA");
    let mut samples = vec![0.0f32; available];
    pcm_consumer.drain_into(&mut samples);
    assert!(samples.iter().any(|&s| s != 0.0));
}
```

### DMA Commands

| Opcode | Name | Params | Notes |
|--------|------|--------|-------|
| `0x10` | Direct DAC | 1 | Single byte output, no DMA |
| `0x14` | 8-bit single-cycle DMA | 2 | Length lo/hi; unsigned 8-bit |
| `0x16` | 8-bit single-cycle DMA (signed) | 2 | Same but signed |
| `0x1C` | 8-bit auto-init DMA | 0 | Uses previously set length |
| `0x40` | Set time constant | 1 | `256 − (1 000 000 / (channels × rate))` |
| `0x41` | Set sample rate output | 2 | SB16: 16-bit rate directly (hi, lo) |
| `0x48` | Set DMA block size | 2 | Pre-sets length for auto-init |
| `0xD0` | Halt 8-bit DMA | 0 | |
| `0xD4` | Continue 8-bit DMA | 0 | |
| `0xD5` | Halt 16-bit DMA | 0 | |
| `0xD6` | Continue 16-bit DMA | 0 | |
| `0xD9` | Exit 16-bit auto-init DMA | 0 | |
| `0xDA` | Exit 8-bit auto-init DMA | 0 | |
| `0xB0`–`0xBF` | 16-bit DMA (various modes) | 3 | Mode byte, length lo/hi |
| `0xC0`–`0xCF` | 8-bit DMA (various modes) | 3 | Mode byte, length lo/hi |

### DMA data path

For DSP playback (memory → DAC), the DMA mode is READ (memory → device). Each DMA cycle:

1. Bus reads one byte from `phys_addr` in memory.
2. Bus calls `sound_blaster.dma_write_u8(byte)`.
3. `dma_write_u8` converts the byte to `f32` and pushes it to `pcm_out`.
4. When `block_len` bytes are consumed, set `irq_pending_8` / `irq_pending_16` and update `mixer.reg[0x82]`.
5. Single-cycle mode deactivates DMA; auto-init resets the byte counter and continues.

Sample conversion:

```rust
fn pcm_u8_to_f32(b: u8) -> f32  { (b as f32 - 128.0) / 128.0 }
fn pcm_s8_to_f32(b: u8) -> f32  { (b as i8) as f32 / 128.0 }
fn pcm_s16_to_f32(lo: u8, hi: u8) -> f32 {
    i16::from_le_bytes([lo, hi]) as f32 / 32768.0
}
```

For 16-bit DMA, two consecutive bytes form one sample. `dma_write_u8` buffers the low byte and emits on the high byte.

### DREQ signaling

When the DSP starts DMA, it asserts DREQ on the DMA controller so the controller begins transferring. Add `set_dreq(channel, active)` to `DmaController` if not already present. The SB registers itself at `dma_devices[1]` (8-bit) and `dma_devices[5]` (16-bit).

---

## Phase 8 — MPU-401 MIDI (UART Mode)

**Ports**: `0x330` (data), `0x331` (command/status)

### Tests first

**`core/src/test_data/devices/sound_blaster/mpu_reset.asm`**

```nasm
; mpu_reset.asm — MPU-401 reset and UART mode entry
;
; 1. Send reset command 0xFF to port 0x331
; 2. Poll port 0x331 bit 7 until data available
; 3. Read from 0x330 — must be 0xFE (ACK)
; 4. Send UART mode command 0x3F to 0x331
; 5. Read 0x330 — must be 0xFE (ACK)
;
; Exit: 0=pass, 1=reset ACK wrong, 2=UART ACK wrong

[CPU 8086]
org 0x100

start:
    ; Send reset
    mov dx, 0x331
    mov al, 0xFF
    out dx, al

    ; Poll status bit 7 (data available = bit 7 set)
    mov cx, 5000
.poll1:
    in al, dx
    test al, 0x80
    jnz .read1
    loop .poll1
.read1:
    mov dx, 0x330
    in al, dx
    cmp al, 0xFE
    je .uart_cmd
    mov al, 0x01
    jmp .exit

.uart_cmd:
    mov dx, 0x331
    mov al, 0x3F
    out dx, al
    mov cx, 5000
.poll2:
    in al, dx
    test al, 0x80
    jnz .read2
    loop .poll2
.read2:
    mov dx, 0x330
    in al, dx
    cmp al, 0xFE
    je .pass
    mov al, 0x02
    jmp .exit

.pass:
    xor al, al
.exit:
    mov ah, 0x4C
    int 0x21
```

**Tests added to `core/src/tests/devices/sound_blaster.rs`**:

```rust
/// MPU-401 reset returns 0xFE ACK; entering UART mode also returns 0xFE.
#[test_log::test]
pub(crate) fn mpu_reset_and_uart_mode() {
    run_test(
        "devices/sound_blaster/mpu_reset",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}
```

### MPU substate

```rust
struct SoundBlasterMpu {
    uart_mode: bool,
    irq_pending: bool,
    out_buf: VecDeque<u8>,  // bytes queued for host to read from 0x330
}
```

### Port behavior

| Port | Read | Write |
|------|------|-------|
| `0x330` | Next byte from `out_buf` (or `0x00`) | Discard in UART mode |
| `0x331` | Status: bit 6=output ready (0=ready), bit 7=data available | Command byte |

### Commands

| Byte | Name | Notes |
|------|------|-------|
| `0xFF` | Reset | Clears UART mode; enqueues `0xFE` ACK |
| `0x3F` | Enter UART mode | Sets `uart_mode`; enqueues `0xFE` ACK |

MIDI output bytes in UART mode are silently discarded. No synthesizer backend needed initially.

---


## Phase 9 — Native Audio Backend

### Mixing two ring buffers

Add `MixedSource` — a Rodio `Source` that pulls from both `opl_out` and `pcm_out` ring buffers and sums them:

```rust
struct MixedSource {
    opl: PcmRingBuffer,
    pcm: PcmRingBuffer,
    sample_rate: u32,
}

impl Iterator for MixedSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let mut a = [0.0f32; 1];
        let mut b = [0.0f32; 1];
        self.opl.drain_into(&mut a);
        self.pcm.drain_into(&mut b);
        Some((a[0] + b[0]).clamp(-1.0, 1.0))
    }
}
```

### Sample rate

OPL output is fixed at `44100 Hz`. DSP PCM uses the rate negotiated via command `0x41` / `0x40`. The simplest correct path: accumulate DSP samples at the DSP sample rate in a staging buffer, then linear-resample to `44100 Hz` before pushing to `pcm_out`. Store the DSP sample rate only for timer calculations if resampling is deferred.

---

## Files Changed Summary

| File | Change |
|------|--------|
| `core/src/devices/sound_blaster.rs` | **New** — `SoundBlaster` implementing `Device + SoundCard + CdromController` |
| `core/src/devices/mod.rs` | Add `pub mod sound_blaster`; add `SoundCardType::SoundBlaster16`; **keep `SoundCardType::AdLib` and `adlib.rs` unchanged** |
| `core/src/devices/adlib.rs` | **Unchanged** |
| `core/src/devices/sound_blaster_cdrom.rs` | Remove after Phase 6 (absorbed) |
| `core/src/devices/pic.rs` | No changes |
| `core/src/bus.rs` | Add `add_sound_blaster<T>()`; register DMA devices ch 1 and ch 5 |
| `core/src/computer.rs` | Add `add_sound_blaster<T>()`; expose `opl_consumer()` / `pcm_consumer()` |
| `core/src/devices/dma.rs` | Add `set_dreq(channel, active)` if not present |
| `core/src/tests/devices/sound_blaster.rs` | **New** — all tests, grown incrementally phase by phase |
| `core/src/tests/devices/mod.rs` | Add `pub(crate) mod sound_blaster` |
| `core/src/test_data/devices/sound_blaster/*.asm` | **New** — one or more programs per phase |
| `native-common/src/cli.rs` | Add `--sound-blaster-*` flags; add `sb16` to `--sound-card` |
| `native-common/src/lib.rs` | Add `SoundBlaster` instantiation path; `Adlib` path unchanged |
| `native-audio/src/lib.rs` | Add `MixedSource` for SB16; existing Rodio/Adlib path unchanged |
| `wasm/src/lib.rs` | Add `SoundBlaster` instantiation; keep existing WASM bindings |

---

## Implementation Order

1. **Phase 1 — Example Program** ✅ — `examples/sound_blaster.asm` and `.com` are done.
2. **Phase 2 — Absorb SoundBlasterCdrom** — create `SoundBlaster` struct wrapping `SoundBlasterCdromInner`, stub all non-CD-ROM IO, add `Bus::add_sound_blaster()`, verify all existing CD-ROM tests still pass.
3. **Phase 3 — CLI, GUI, and WASM** — add `--sound-card sb16` flag, wire `SoundBlaster` instantiation in `native-common`, GUI, and WASM; now runnable with `--sound-card sb16 sound_blaster.com`.
4. **Phase 4 — DSP** — write `dsp_reset.asm` + `dsp_speaker.asm`, implement reset handshake and basic DSP commands, verify tests pass.
5. **Phase 5 — OPL3 FM** — write `opl_detect.asm` + `opl_adlib_compat.asm` + `opl_play_tone.asm`, wire OPL3 ports at SB base and `0x388`, verify tests pass.
6. **Phase 6 — Mixer** — write `mixer_readwrite.asm`, implement `[u8; 256]` register array, verify test passes.
7. **Phase 7 — PCM DMA** — write `dsp_pcm_single.asm` + `dsp_pcm_samples.asm`, implement 8-bit DMA path and IRQ, wire DMA slots in `Bus::add_sound_blaster()`, verify tests pass.
8. **Phase 8 — MPU-401** — write `mpu_reset.asm`, implement UART mode stub, verify test passes.
9. **Phase 9 — Native Audio Backend** — add `MixedSource` mixing OPL and PCM ring buffers, test manually with a game.

## Open Questions / Risks

- **DMA timing**: The DMA controller advances by `elapsed / 4` cycles. High sample rates (44100 Hz at 4.77 MHz = ~108 cycles per sample) may produce coarse audio. Monitor for gaps.
- **16-bit DMA word order**: The real SB16 uses word-addressed channel 5 DMA (each "address" = 2 bytes). The emulator's DMA is byte-addressed. Channel 5 may need special handling to pair bytes correctly.
- **Sample rate changes mid-playback**: Some software issues `0x41` between blocks. `pcm_out` is fixed at 44100 Hz; the resampler must handle rate changes without a pop.
- **Stereo interleaving**: In stereo mode L/R samples alternate. `dma_write_u8` must track L/R phase.
- **IRQ sharing**: DSP and CD-ROM share IRQ5. Priority: DSP 8-bit > DSP 16-bit > CD-ROM.

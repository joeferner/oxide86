# ATA/ATAPI Emulation Plan

## Context

CD-ROM drivers (e.g., OAKCDROM.SYS, MTMCDAI.SYS) communicate with the CD-ROM drive via
ATA/ATAPI port registers at `0x1F0–0x1F7` (primary IDE) and `0x3F6` (alternate status/control).
Currently these ports fall through to the catch-all in `IoDevice::read_byte()` and return `0xFF`,
causing the driver to see BSY=1 and ERR=1 on every status read — so it immediately fails or
spins forever.

The existing `CdRomImage` (ISO 9660) and `DriveManager` CD-ROM slots (`0xE0–0xE3`) are already
implemented. This plan adds the ATA register-level emulation layer so real CD-ROM drivers can
talk to the emulator over the ATA bus, and also exposes existing hard drives over the same bus.

---

## Architecture Decision: Where Does ATA Live?

`IoDevice` has no access to `DriveManager` or `CdRomImage`. The existing precedent for solving
this is the serial port: in `core/src/cpu/instructions/io.rs`, ports `0x3F8–0x3FF` and
`0x2F8–0x2FF` are routed through `bios.serial_io_read/write()` rather than `IoDevice`, giving
them access to the full `Bios` state.

**ATA will follow the same pattern.** An `AtaController` struct lives inside `Bios`/
`SharedBiosState`, where it can call `drive_manager` directly. The CPU I/O instruction dispatch
in `io.rs` routes `0x1F0–0x1F7` and `0x3F6` to `bios.ata_read/write()`.

---

## Files to Create

### `core/src/io/ata.rs` (NEW, ~400 lines)

Owns the ATA register state machine and data buffers. Does **not** do disk I/O itself — it
delegates to a trait so `Bios` can inject `DriveManager` access.

```
pub const ATA_STATUS_ERR:  u8 = 0x01;
pub const ATA_STATUS_DRQ:  u8 = 0x08;
pub const ATA_STATUS_DSC:  u8 = 0x10; // DRDY for ATA, service for ATAPI
pub const ATA_STATUS_DRDY: u8 = 0x40;
pub const ATA_STATUS_BSY:  u8 = 0x80;

pub const ATA_ERR_ABRT: u8 = 0x04;  // command aborted

// ATAPI device signature written to cylinder regs after SRST/IDENTIFY
pub const ATAPI_SIG_MID: u8 = 0x14;
pub const ATAPI_SIG_HI:  u8 = 0xEB;

pub enum AtaDevice { None, HardDrive(u8), CdRom(u8) }

pub enum TransferState {
    Idle,
    WaitingPacket,            // after PACKET cmd: expecting 12-byte CDB on data port
    DataReady { buf: Vec<u8>, pos: usize },   // DRQ set; driver reads data
    DataWrite { buf: Vec<u8>, pos: usize, cmd: u8 }, // DRQ set; driver writes
}

pub struct AtaChannel {
    // ATA registers (as seen by CPU reads)
    pub data_buf:   [u8; 2],        // for 16-bit IN/OUT word on port 0x1F0
    pub error:      u8,             // 0x1F1 read (features on write, handled separately)
    pub features:   u8,             // 0x1F1 write (write-only shadow)
    pub sector_count: u8,          // 0x1F2
    pub lba_low:    u8,             // 0x1F3 (sector number)
    pub lba_mid:    u8,             // 0x1F4 (cylinder low)
    pub lba_high:   u8,             // 0x1F5 (cylinder high)
    pub device_head: u8,            // 0x1F6
    pub status:     u8,             // 0x1F7 (command on write)
    pub control:    u8,             // 0x3F6 (device control)

    pub selected_device: u8,        // 0 = master, 1 = slave
    pub master: AtaDevice,
    pub slave:  AtaDevice,

    pub transfer: TransferState,
    pub packet_buf: [u8; 12],       // accumulates ATAPI CDB
    pub packet_pos: u8,
    pub pending_cmd: Option<u8>,    // set on command write, cleared after execution
}

impl AtaChannel {
    pub fn new() -> Self { ... }

    // Called by Bios; returns value for port (port is 0–7 relative to base 0x1F0)
    pub fn read_port_u8(&mut self, port: u8) -> u8
    pub fn read_port_u16(&mut self) -> u16   // port 0 only

    // Called by Bios; returns true if a disk operation is now pending
    pub fn write_port_u8(&mut self, port: u8, value: u8) -> bool
    pub fn write_port_u16(&mut self, value: u16) -> bool  // port 0 only

    pub fn read_alt_status(&self) -> u8   // 0x3F6: same as status, no side-effects
    pub fn write_control(&mut self, value: u8)  // 0x3F6 write (SRST, nIEN)

    // Execute a pending non-data ATA command.  Bios calls this with disk access.
    // Returns Some(error_code) on failure.
    pub fn exec_identify(&mut self, is_atapi: bool, model: &str, sectors: u32)
    pub fn exec_set_data_buf(&mut self, data: Vec<u8>)  // loads buffer and sets DRQ
    pub fn exec_error(&mut self, err: u8)               // sets ERR + error register
    pub fn exec_ok(&mut self)                            // clears BSY, sets DRDY
}
```

**`read_port_u8` logic (port 0–7):**
- Port 0 (`0x1F0`): return first byte of `data_buf`; port 0 reads should use `read_port_u16`
- Port 1 (`0x1F1`): return `error` register
- Port 2–5: return respective register
- Port 6 (`0x1F6`): return `device_head`
- Port 7 (`0x1F7`): return `status` (reading clears IRQ pending flag)

**`write_port_u8` logic:**
- Port 0: accumulate `data_buf`; when packet_pos reaches 12, set `pending_cmd = Some(PACKET)`
- Ports 1–6: write to corresponding register shadows; port 6 updates `selected_device`
- Port 7 (command): set BSY, store command in `pending_cmd`, return `true` (disk op pending)

**SRST (software reset via control port 0x3F6 bit 2):**
- Rising edge of SRST: set BSY, clear DRQ
- Falling edge: write ATAPI signature to cylinder regs if device is ATAPI, clear BSY, set DRDY

---

## Files to Modify

### `core/src/io/mod.rs`

Remove ATA port range from the catch-all. **No field is added to `IoDevice`** — routing moves
to Bios via io.rs. Only change is to remove `last_write` fallback for 0x1F0–0x1F7 and 0x3F6.
(Currently a no-op because nothing writes there, but cleaner to document.)

### `core/src/cpu/instructions/io.rs`

Follow the existing serial port routing pattern. In `in_al_dx` / `out_dx_al` (and the imm8
variants and word variants), add ATA cases before the `io_device` fallback:

```rust
// Primary IDE / ATAPI
0x1F0 => bios.ata_read_u16(0, is_word),     // word reads on data port
0x1F1..=0x1F7 => bios.ata_read_u8(0, (port - 0x1F0) as u8),
0x3F6 | 0x3F7  => bios.ata_read_alt(0),

// Secondary IDE (stub — return 0x7F "no drive")
0x170..=0x177 => 0x7F,
0x376          => 0x7F,
```

Write path:
```rust
0x1F0 => bios.ata_write_u16(0, value_u16),
0x1F1..=0x1F7 => bios.ata_write_u8(0, (port - 0x1F0) as u8, value),
0x3F6          => bios.ata_write_control(0, value),
```

After any write that returns `pending_cmd`, call `bios.ata_execute(0)` inline — the
command completes synchronously (no IRQ deferral needed for DOS drivers that poll status).

**Note:** `IN AX, DX` / `OUT DX, AX` must be detected at port `0x1F0` for 16-bit data transfer.
The data port is the only port that uses 16-bit I/O; all others are byte-wide.

### `core/src/cpu/bios/mod.rs`

Add `AtaChannel` to `SharedBiosState`:

```rust
use crate::io::ata::AtaChannel;

pub struct SharedBiosState<D: DiskController> {
    ...
    pub ata_primary: AtaChannel,  // primary IDE channel (master/slave)
}
```

Initialize in `SharedBiosState::new()`:
```rust
ata_primary: AtaChannel::new(),
```

Add public delegate methods on `Bios<K, D>`:

```rust
pub fn ata_read_u8(&mut self, channel: u8, reg: u8) -> u8 { ... }
pub fn ata_read_u16(&mut self, channel: u8) -> u16 { ... }
pub fn ata_read_alt(&self, channel: u8) -> u8 { ... }
pub fn ata_write_u8(&mut self, channel: u8, reg: u8, value: u8) { self.ata_execute_if_pending(channel) }
pub fn ata_write_u16(&mut self, channel: u8, value: u16) { ... }
pub fn ata_write_control(&mut self, channel: u8, value: u8) { ... }
```

Add `ata_execute(channel: u8)` which reads `pending_cmd` and dispatches:

```rust
fn ata_execute(&mut self, channel: u8) {
    let ch = &mut self.shared.ata_primary; // (channel param selects primary/secondary)
    let Some(cmd) = ch.pending_cmd.take() else { return };
    match cmd {
        0xEC => self.ata_cmd_identify(channel),
        0xA1 => self.ata_cmd_identify_packet(channel),
        0x20 | 0x21 => self.ata_cmd_read_sectors(channel),
        0x30 | 0x31 => self.ata_cmd_write_sectors(channel),
        0xA0 => { /* PACKET: wait for CDB via data port writes */ }
        0x08 => { /* DEVICE RESET — ATAPI only, just set ok */ ch.exec_ok(); }
        0x91 => { /* SET DRIVE PARAMETERS — accept and ignore */ ch.exec_ok(); }
        _    => { ch.exec_error(ATA_ERR_ABRT); }
    }
}
```

Add private command handlers on `Bios`:

**`ata_cmd_identify(channel)`** (ATA hard drive):
- Get current device (master/slave) from `ch.device_head` bit 4
- Look up hard drive from `drive_manager` (master → drive 0x80, slave → 0x81)
- If not present: `ch.exec_error(ATA_ERR_ABRT)`; return
- Build 512-byte IDENTIFY response:
  - Word 0: `0x0040` (fixed/non-removable)
  - Words 1/3/6: cylinders/heads/sectors (from `DiskGeometry`)
  - Words 10–19: serial number ("12345678" padded, byte-swapped pairs)
  - Words 23–26: firmware revision ("1.0 " padded)
  - Words 27–46: model string (padded, byte-swapped pairs)
  - Word 49: `0x0200` (LBA supported)
  - Words 60–61: total addressable sectors (LBA28)
- Call `ch.exec_set_data_buf(buf)` → sets DRQ, copies to transfer buffer

**`ata_cmd_identify_packet(channel)`** (ATAPI CD-ROM):
- Get selected device
- Check if `AtaDevice::CdRom` is present; else error
- Write ATAPI signature to cylinder regs (`lba_mid = 0x14, lba_high = 0xEB`)
- Build 512-byte IDENTIFY PACKET DEVICE response:
  - Word 0: `0x8580` (ATAPI, CD-ROM device type 5, packet size 12)
  - Words 10–19: serial number
  - Words 27–46: model ("OX86 CDROM          " padded, byte-swapped)
  - Word 49: `0x0200`
- Call `ch.exec_set_data_buf(buf)`

**`ata_cmd_read_sectors(channel)`** (ATA hard drive):
- Compute LBA from `lba_low/mid/high` + `device_head` bits (LBA28 if bit 6 of device_head set,
  else CHS using `sector_count`)
- Read `sector_count` sectors from `drive_manager` into a `Vec<u8>` (each 512 bytes)
- If error: `ch.exec_error(ATA_ERR_ABRT)`
- Else: `ch.exec_set_data_buf(data)` (driver reads all sectors via repeated data port reads)

**`ata_cmd_write_sectors(channel)`**:
- Set DRQ to accept data from driver; record `sector_count` and LBA in channel state
- When data transfer completes (all bytes written to data port), flush to `drive_manager`

**ATAPI PACKET dispatch** (called when 12-byte CDB is complete in `packet_buf`):

```rust
fn ata_exec_packet(&mut self, channel: u8) {
    let cdb = self.shared.ata_primary.packet_buf;
    match cdb[0] {
        0x00 => self.atapi_test_unit_ready(channel),
        0x03 => self.atapi_request_sense(channel),
        0x12 => self.atapi_inquiry(channel),
        0x1E => self.atapi_prevent_allow_removal(channel), // just ACK
        0x25 => self.atapi_read_capacity(channel),
        0x28 => self.atapi_read10(channel),
        0x43 => self.atapi_read_toc(channel),
        0x4A => self.atapi_get_event_status(channel),
        0x5A => self.atapi_mode_sense(channel),
        _    => self.atapi_sense_error(channel, 0x05, 0x20), // ILLEGAL REQUEST
    }
}
```

Key ATAPI commands to implement:

- **TEST UNIT READY (0x00)**: if CD-ROM slot has image → `exec_ok()`; else sense error
  `0x02/0x3A` (NOT READY / MEDIUM NOT PRESENT)
- **REQUEST SENSE (0x03)**: return 18-byte fixed sense data (no error if previous ok)
- **INQUIRY (0x12)**: return 36-byte standard response:
  - byte 0: `0x05` (CD-ROM)
  - byte 1: `0x80` (removable)
  - byte 2: `0x00` (SCSI-1 for simplicity)
  - bytes 8–15: vendor "OX86    "
  - bytes 16–31: product "CDROM           "
  - bytes 32–35: revision "1.00"
- **READ CAPACITY (0x25)**: return 8 bytes: last LBA (big-endian u32) + block size (2048 BE)
- **READ(10) (0x28)**: LBA from CDB bytes [2..6] BE u32, length from CDB bytes [7..9] BE u16;
  read sectors from `CdRomImage::read_sector(lba)` for each; concatenate into buffer;
  `exec_set_data_buf(buf)`
- **READ TOC (0x43)**: return minimal TOC:
  - 4-byte TOC header (first=1, last=1)
  - one track descriptor for track 1 (data, starts at LBA 0)
  - one lead-out descriptor (starts at last LBA+1)
- **MODE SENSE(6/10) (0x5A)**: return minimal page 0x2A (CD capabilities) or empty if unknown page
- **PREVENT/ALLOW MEDIUM REMOVAL (0x1E)**: always succeed (no-op)
- **GET EVENT STATUS NOTIFICATION (0x4A)**: return 8 bytes with no events pending

### `core/src/lib.rs`

Add `pub mod ata; pub use crate::io::ata::AtaChannel;` (to allow `Computer` to wire drives).

Add public methods to `Computer<V>`:
```rust
pub fn set_ata_master(&mut self, channel: u8, device: AtaDevice) { ... }
pub fn set_ata_slave(&mut self, channel: u8, device: AtaDevice)  { ... }
```

### `core/src/computer.rs`

In `Computer::new()` (or a new `configure_ata()` helper), after drives are loaded, wire up the
ATA channel:
```rust
// Master = first hard drive (0x80), slave = second (0x81) if present
if drive_manager.has_hard_drive(0) {
    bios.shared.ata_primary.master = AtaDevice::HardDrive(0x80);
}
if drive_manager.has_hard_drive(1) {
    bios.shared.ata_primary.slave = AtaDevice::HardDrive(0x81);
}
// If no hard drives, master = CD-ROM slot 0 (common CD-only config)
if drive_manager.has_cdrom(0) && !drive_manager.has_hard_drive(0) {
    bios.shared.ata_primary.master = AtaDevice::CdRom(0);
}
```

For CD-ROM on secondary channel (common DOS config: HDD=master, CD=slave or secondary master):
```rust
// If hard drive is on primary master, CD-ROM goes on primary slave or secondary master.
// Default: primary slave if hard drive on primary master.
if drive_manager.has_hard_drive(0) && drive_manager.has_cdrom(0) {
    bios.shared.ata_primary.slave = AtaDevice::CdRom(0);
}
```

Update `insert_cdrom` / `eject_cdrom` delegate methods to also update `ata_primary` device assignment.

---

## ATAPI IDENTIFY Response — Critical Fields

The CD-ROM driver's `IDENTIFY PACKET DEVICE` response must have these exact bits or the driver
will reject the device:

| Word | Value   | Meaning                                                    |
|------|---------|------------------------------------------------------------|
| 0    | 0x8580  | Peripheral type=5 (CD-ROM), ATAPI, 12-byte packet         |
| 49   | 0x0200  | LBA capable (required for ATAPI)                          |
| 53   | 0x0006  | Words 64–70 and 88 valid                                  |
| 63   | 0x0007  | Multiword DMA modes supported (drivers check this)        |
| 64   | 0x0003  | PIO modes 3 and 4 supported                               |
| 98   | 0x4000  | ATAPI (set in word 0 already, mirrors here)                |

All string fields (serial, model, firmware) use swapped byte pairs per ATA standard.

---

## Drive Geometry / LBA Addressing

Hard drives already have `DiskGeometry` via `DriveManager`. For ATA READ SECTORS in CHS mode:
```
lba = (cylinder * heads + head) * spt + (sector - 1)
```
where `cylinder = (lba_high << 8) | lba_mid`, `head = device_head & 0x0F`, `sector = lba_low`.

For LBA28 mode (bit 6 of `device_head` set):
```
lba = ((device_head & 0x0F) as u32) << 24 | (lba_high as u32) << 16
    | (lba_mid as u32) << 8 | lba_low as u32
```

---

## Interrupt Handling

DOS CD-ROM drivers may program the PIC to fire IRQ14 (INT 0x76) on command completion. The
emulator doesn't need to actually fire it — DOS drivers using polling (reading `0x1F7` in a
loop) will work as long as BSY clears promptly. Since all commands complete synchronously in
`ata_execute()`, BSY is already cleared by the time the poll loop reads status.

If a driver hangs waiting for IRQ14, it can be unblocked by adding IRQ14 to the existing IRQ
queue mechanism in `computer.rs` after `ata_execute()` runs. Add this only if needed.

---

## Implementation Order

1. **`core/src/io/ata.rs`** — `AtaChannel` struct, register read/write, transfer state machine
2. **`core/src/cpu/bios/mod.rs`** — add `ata_primary` field, delegate read/write methods,
   `ata_execute()` dispatcher, `ata_cmd_identify()`, `ata_cmd_identify_packet()`
3. **`core/src/cpu/instructions/io.rs`** — route `0x1F0–0x1F7`, `0x3F6` to `bios.*` methods
4. **Smoke test**: boot DOS with a hard drive — `IDENTIFY` should work; hard drive appears on ATA bus
5. **`ata_cmd_read_sectors()` / `ata_cmd_write_sectors()`** — wire to DriveManager
6. **ATAPI PACKET dispatch** — `atapi_test_unit_ready`, `atapi_inquiry`, `atapi_read_capacity`,
   `atapi_read10`, `atapi_read_toc`
7. **Wire CD-ROM in `computer.rs`** — `set_ata_master/slave` based on loaded drives
8. **Smoke test**: load a CD-ROM driver (e.g., OAKCDROM.SYS) in `CONFIG.SYS` with ISO image

---

## Test Programs

Create `test-programs/cdrom/ata_identify.asm`:
```nasm
; Send IDENTIFY DEVICE to primary master and print model string
mov dx, 0x1F6
mov al, 0xA0           ; select master
out dx, al
mov dx, 0x1F7
mov al, 0xEC           ; IDENTIFY DEVICE
out dx, al
.wait: in al, dx       ; poll BSY
test al, 0x80
jnz .wait
mov dx, 0x1F0
; read 256 words ...
```

Create `test-programs/cdrom/atapi_read_toc.asm` — selects ATAPI device, issues PACKET + READ TOC CDB.

Update `test-programs/README.md`.

---

## Verification

1. `./scripts/pre-commit.sh` — must pass
2. **Hard drive visible on ATA bus**: boot DOS, `FDISK` or `MSD` shows drive; no regression
3. **CD-ROM driver loads**: add `DEVICE=OAKCDROM.SYS /D:MSCD001` to CONFIG.SYS; driver reports
   "ATAPI CD-ROM drive found"
4. **MSCDEX works**: add `MSCDEX /D:MSCD001`; `DIR D:` lists ISO contents
5. **File reads**: copy a file from CD to HD; verify content matches ISO
6. **Eject/insert hot-swap**: eject via GUI, re-insert different ISO, `DIR D:` shows new contents

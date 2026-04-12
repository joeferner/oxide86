# CD-ROM Support Plan (SBPCD.SYS / Panasonic Interface)

## Background

SBPCD.SYS drives Panasonic/Matsushita CR-52x/CR-56x CD-ROM drives via the Sound Blaster CD interface. The protocol is pure PIO — 4 IO ports at a configurable base (default `0x230`). No DMA is used. SBPCD.SYS can work in both polling and IRQ mode; we implement full IRQ support with a configurable IRQ line (default 5).

---

## ✅ Phase 1 — Core CD-ROM backend (`core/`)

### ✅ 1.1 New file: `core/src/disk/cdrom.rs`

A thin sector-reading abstraction over the existing `DiskBackend` trait. CD-ROM sectors are 2048 bytes (ISO 9660 Mode 1). No ISO parsing is needed — SBPCD.SYS and DOS do that.

```rust
pub enum CdromError { ReadError, OutOfRange }

pub trait CdromBackend {
    fn read_sector(&mut self, lba: u32, buf: &mut [u8; 2048]) -> Result<(), CdromError>;
    fn total_sectors(&self) -> u32;
}

pub struct BackedCdrom<B: DiskBackend> { backend: B }

impl<B: DiskBackend> CdromBackend for BackedCdrom<B> {
    fn read_sector(&mut self, lba: u32, buf: &mut [u8; 2048]) -> Result<(), CdromError> {
        let offset = lba as u64 * 2048;
        self.backend.read_at(offset, buf).map_err(|_| CdromError::ReadError)?;
        Ok(())
    }
    fn total_sectors(&self) -> u32 { (self.backend.size() / 2048) as u32 }
}
```

Add `pub mod cdrom;` to `core/src/disk/mod.rs`.

### ✅ 1.2 New trait: `CdromController` in `core/src/devices/mod.rs`

This mirrors the existing `SoundCard` trait pattern so that Bus and PIC are decoupled from any specific CD-ROM interface type. Future interfaces (ATAPI/IDE, Mitsumi, Sony CDU31A, etc.) implement the same trait and slot in without touching Bus or PIC.

```rust
pub trait CdromController {
    fn load_disc(&mut self, disc: Box<dyn CdromBackend>);
    fn eject_disc(&mut self);
    /// Called by the PIC to drain a pending IRQ. Returns `true` once per interrupt.
    fn take_pending_irq(&mut self) -> bool;
    /// The PIC1 IRQ line this device raises (e.g. 5 for the default SB CD interface).
    fn irq_line(&self) -> u8;
}

pub type CdromControllerRef = Rc<RefCell<dyn CdromController>>;
```

`SoundBlasterCdrom` (and any future interface) must implement both `Device` and `CdromController`. The Bus/PIC/Computer never name the concrete type — they only hold a `CdromControllerRef`.

### ✅ 1.3 New file: `core/src/devices/sound_blaster_cdrom.rs`

Emulates the Panasonic/Matsushita CD interface used by SBPCD.SYS.

**IO port layout** (base configurable, default `0x230`):

| Port | Read | Write |
|------|------|-------|
| base+0 | Status byte | Command byte / param byte |
| base+1 | Data / result byte | (param byte — variant) |
| base+2 | Extended status | Reset (write `0xFF`) |
| base+3 | Drive select read-back | Drive select (bits 0–1 = drive 0–3) |

**Status byte** (read from base+0):
```
bit 0: result data available in base+1
bit 1: drive busy
bit 2: error flag
bit 4: audio playing (always 0 — no audio yet)
bit 5: disc present
bit 6: door open
```

**Command state machine:**

```
Idle → (write cmd byte to base+0) → RecvParams(n_remaining)
     → (all params received) → Execute → SendResult(result_bytes)
     → (all result bytes read) → Idle
```

For the read sectors command the state machine has an additional `StreamSector` state:
```
Execute → StreamSector   (after emitting the 1-byte status result)
        → (all sectors consumed) → Idle
```

**Commands to implement** (subset SBPCD.SYS needs):

| Cmd | Name | Params | Result bytes | Notes |
|-----|------|--------|--------------|-------|
| `0x00` | NOP / ping | 0 | 1 (status) | Drive presence check |
| `0x01` | Stop | 0 | 1 | |
| `0x05` | Read status | 0 | 5 | Extended status word |
| `0x09` | Set mode | 1 | 1 | Data vs audio mode |
| `0x0A` | Seek (MSF) | 3 | 1 | Seek to M:S:F |
| `0x0B` | Read sectors | 7 | 1 then sector data | MSF start (3) + count (3) + mode (1); data streamed via base+1 |
| `0x0C` | Pause | 0 | 1 | |
| `0x0D` | Resume | 0 | 1 | |
| `0x10` | Reset | 0 | 1 | |
| `0x11` | Read TOC | 2 | variable | First/last track + TOC entries |
| `0x12` | Read disc info | 0 | 6 | First track, last track, total size in MSF |

The read sectors command (`0x0B`) is the critical one. After the 1-byte status result, subsequent reads from `base+1` return the raw 2048-byte sector data, one byte at a time. The next sector is loaded automatically until the count is exhausted.

**LBA ↔ MSF conversion helpers** (needed because SBPCD uses MSF addressing):
```
lba = (M * 60 + S) * 75 + F - 150   (150 = 2-second pregap)
```

**IRQ handling:**
- After a command completes, if IRQ is enabled, set `pending_irq = true`
- `take_pending_irq() -> bool` method for PIC to drain
- IRQ line is configurable at construction time (default 5 matches real SB hardware)

**Disc-not-present behavior:** When no ISO is loaded, status bit 5 is clear and bit 6 (door open) is set. All commands return an error status byte.

**Struct:**
```rust
pub struct SoundBlasterCdrom {
    base_port: u16,
    drive_selected: u8,
    disc: Option<Box<dyn CdromBackend>>,
    state: CdromState,
    command: u8,
    params: Vec<u8>,
    result_buf: VecDeque<u8>,
    // Current PIO read state
    read_lba: u32,
    read_remaining: u32,
    read_sector_buf: [u8; 2048],
    read_sector_pos: usize,
    // Status
    door_open: bool,
    error: bool,
    pending_irq: bool,
    irq_enabled: bool,
    irq_line: u8,
}
```

`new()` signature:
```rust
pub fn new(base_port: u16, disc: Option<Box<dyn CdromBackend>>, irq_line: u8) -> Self
```

### ✅ 1.4 Modify `core/src/devices/pic.rs`

Add `cdrom: Option<CdromControllerRef>` field. The IRQ line and CPU vector are read dynamically from `cdrom.borrow().irq_line()` so the PIC needs no changes when the IRQ line is reconfigured or when a second CD-ROM interface type is added later.

```rust
// CD-ROM / Sound Blaster CD interface (IRQ line and CPU vector from device)
if let Some(ref cdrom) = self.cdrom {
    let irq = cdrom.borrow().irq_line();
    let bit = 1u8 << irq;
    let masked = self.mask & bit != 0;
    let in_service = self.in_service & bit != 0;

    if !masked && !in_service && cdrom.borrow_mut().take_pending_irq() {
        self.in_service |= bit;
        return Some(0x08 + irq);   // PIC1 base is 0x08
    }
}
```

No hardcoded `CDROM_CPU_IRQ` or `CDROM_IRQ_LINE` constants — the values come from the device.

### ✅ 1.5 Modify `core/src/bus.rs`

Add a `cdrom_controller: Option<CdromControllerRef>` field (generic, not `SoundBlasterCdrom`). Mirror the existing `add_sound_card()` pattern:

```rust
pub(crate) fn add_cdrom_controller<T: Device + CdromController + 'static>(
    &mut self,
    device: T,
) {
    let rc = Rc::new(RefCell::new(device));
    self.devices.push(rc.clone());                    // IO port dispatch
    self.cdrom_controller = Some(rc.clone());         // disc swap / IRQ access
    self.pic.borrow_mut().set_cdrom(rc);              // IRQ wiring
}
```

This is the only place that names `T` — everything above it (PIC, Computer, native-common, WASM) operates through `CdromControllerRef`.

### ✅ 1.6 Modify `core/src/computer.rs`

Expose a generic method and disc-swap helpers:
```rust
pub fn add_cdrom_controller<T: Device + CdromController + 'static>(&mut self, device: T) {
    self.bus.borrow_mut().add_cdrom_controller(device);
}
pub fn load_cdrom_disc(&mut self, disc: Box<dyn CdromBackend>) {
    if let Some(cdrom) = &self.bus.borrow().cdrom_controller {
        cdrom.borrow_mut().load_disc(disc);
    }
}
pub fn eject_cdrom_disc(&mut self) {
    if let Some(cdrom) = &self.bus.borrow().cdrom_controller {
        cdrom.borrow_mut().eject_disc();
    }
}
```

### ✅ 1.7 Modify `core/src/devices/mod.rs`

Export `pub mod sound_blaster_cdrom;`, re-export `SoundBlasterCdrom`, and re-export the new `CdromController` trait and `CdromControllerRef` type alias.

---

## ✅ Phase 2 — Native command line support (`native-common/`)

### ✅ 2.1 Modify `native-common/src/cli.rs`

The Sound Blaster CD interface is **enabled by default**. Use `--disable-sound-blaster-cd` to turn it off. The IRQ line is configurable via `--sound-blaster-irq`.

```rust
/// Sound Blaster CD-ROM interface base port
#[arg(long = "sound-blaster-port", value_name = "PORT", default_value = "0x230")]
pub sound_blaster_port: String,

/// Disable the Sound Blaster CD-ROM interface
#[arg(long = "disable-sound-blaster-cd")]
pub disable_sound_blaster_cd: bool,

/// Sound Blaster CD-ROM IRQ line (default: 5)
#[arg(long = "sound-blaster-irq", value_name = "IRQ", default_value = "5")]
pub sound_blaster_irq: u8,

/// ISO image to mount as CD-ROM at startup
#[arg(long = "cdrom", value_name = "FILE")]
pub cdrom: Option<std::path::PathBuf>,
```

Typical usage:
- No flags — CD device registered at `0x230`, IRQ 5, door open (no disc). The GUI can load an ISO later.
- `--cdrom game.iso` — device registered and ISO mounted immediately.
- `--sound-blaster-port 0x250 --sound-blaster-irq 10` — non-default port and IRQ.
- `--disable-sound-blaster-cd` — no CD device registered at all.

`--sound-card adlib` remains separate (OPL2 audio) and is unaffected.

### ✅ 2.2 Modify `native-common/src/lib.rs`

In `create_computer()`, after existing device setup:
```rust
if !cli.disable_sound_blaster_cd {
    let port_str = &cli.sound_blaster_port;
    let base_port = u16::from_str_radix(
        port_str.trim_start_matches("0x").trim_start_matches("0X"), 16,
    ).with_context(|| format!("Invalid Sound Blaster base port: {port_str}"))?;
    let disc: Option<Box<dyn CdromBackend>> = if let Some(path) = &cli.cdrom {
        let backend = FileDiskBackend::open(path_str, true)?;
        Some(Box::new(BackedCdrom::new(backend)))
    } else {
        None
    };
    let device = SoundBlasterCdrom::new(base_port, disc, cli.sound_blaster_irq);
    computer.add_cdrom_controller(device);
}
```

### ✅ 2.3 Modify `native-cli/src/command_mode.rs`

Add `load cd` and `eject cd` to the existing command mode. Follows the same pattern as `load a` / `eject a`.

Add variants to `Command`:
```rust
LoadCd(String),   // path
EjectCd,
```

Add to `Command::parse()`:
```rust
"eject cd" => Command::EjectCd,
s if s.starts_with("load cd ") => Command::LoadCd(s["load cd ".len()..].to_string()),
```

Add to help text and dispatch:
```rust
Command::EjectCd => {
    computer.eject_cdrom_disc();
    println!("CD-ROM ejected.");
}
Command::LoadCd(filename) => {
    match FileDiskBackend::open(&filename, true) {
        Ok(backend) => {
            computer.load_cdrom_disc(Box::new(BackedCdrom::new(backend)));
            println!("CD-ROM loaded: {filename}");
        }
        Err(err) => println!("Error: {err}"),
    }
}
```

The `load cd` command only works if the CD device is registered (i.e. `--disable-sound-blaster-cd` was not passed). If the device is not present, `load_cdrom_disc()` is a no-op and a message is printed: `"No CD-ROM device. Remove --disable-sound-blaster-cd to enable."`.

### ✅ 2.4 Modify `native-gui/src/menu.rs` and `native-gui/src/main.rs`

Add a **CD-ROM** submenu to the **Drives** menu, following the exact same pattern as Floppy A/B.

In `MenuAction`:
```rust
InsertCdrom,
EjectCdrom,
```

In `AppMenu`:
```rust
cdrom_present: bool,
/// False when --disable-sound-blaster-cd was passed; greys out the whole submenu.
cdrom_available: bool,
```

In `AppMenu::render()`:
```rust
ui.add_enabled_ui(self.cdrom_available, |ui| {
    ui.menu_button("CD-ROM:", |ui| {
        if ui.button("Insert Disc...").clicked() {
            action = Some(MenuAction::InsertCdrom);
            ui.close_menu();
        }
        if ui.add_enabled(self.cdrom_present, egui::Button::new("Eject Disc")).clicked() {
            action = Some(MenuAction::EjectCdrom);
            ui.close_menu();
        }
    });
});
```

In `process_egui_frame()` in `main.rs`, handle the new actions:
```rust
MenuAction::InsertCdrom => {
    insert_cdrom_dialog(computer, &mut app_state.cdrom_present,
        &mut app_state.menu, &mut app_state.notification);
}
MenuAction::EjectCdrom => {
    eject_cdrom(computer, &mut app_state.cdrom_present,
        &mut app_state.menu, &mut app_state.notification);
}
```

`cdrom_available` is set once at startup based on `!cli.common.disable_sound_blaster_cd`, so the submenu is visibly greyed out when no CD device is registered.

---

## Phase 3 — WASM support (`wasm/`)

### 3.1 Modify `wasm/src/lib.rs`

Add to `WasmComputerConfig`:
```rust
/// Base port for Sound Blaster CD interface, hex string (default "230")
pub sound_blaster_port: Option<String>,
```

Add to `Oxide86Computer`:
```rust
cdrom: Option<Arc<RwLock<Vec<u8>>>>,
```

Add new method:
```rust
#[wasm_bindgen]
pub fn load_cdrom_image(&mut self, data: Vec<u8>) {
    // Store data; picked up when power_on() creates the computer
}
```

In `power_on()`, if `cdrom` is set:
```rust
if let Some(cdrom_data) = &self.cdrom {
    let base_port = /* parse config or default 0x230 */;
    let irq_line = /* parse config or default 5 */;
    let backend = SharedMemBackend::new(Arc::clone(cdrom_data));
    let cdrom_backend = BackedCdrom::new(backend);
    let device = SoundBlasterCdrom::new(base_port, Some(Box::new(cdrom_backend)), irq_line);
    computer.add_cdrom_controller(device);
}
```

The `SharedMemBackend` already exists in `wasm/src/shared_mem_backend.rs` and implements `DiskBackend`, so it works with `BackedCdrom` directly.

Also add runtime insert/eject methods (for use after `power_on()`, matching `insert_floppy` / `eject_floppy`):
```rust
#[wasm_bindgen]
pub fn insert_cdrom(&mut self, image: js_sys::Uint8Array) {
    let data: Vec<u8> = image.to_vec();
    let shared = Arc::new(RwLock::new(data));
    self.cdrom = Some(Arc::clone(&shared));
    if let Some(computer) = &self.state {
        let backend = SharedMemBackend::new(shared);
        computer.load_cdrom_disc(Box::new(BackedCdrom::new(backend)));
    }
}

#[wasm_bindgen]
pub fn eject_cdrom(&mut self) {
    self.cdrom = None;
    if let Some(computer) = &self.state {
        computer.eject_cdrom_disc();
    }
}
```

### 3.2 Modify WASM frontend (`wasm/www/`)

Follows the exact same pattern as the floppy A/B UI. Three touch points:

**`wasm/www/src/state.ts`** — add reactive state and operations:
```typescript
private readonly cdromSignal = signal<File | null>(null);
get cdrom(): ReadonlySignal<File | null> { return this.cdromSignal; }

async insertCdrom(file: File): Promise<void> {
    const data = await file.arrayBuffer();
    this.computer.insert_cdrom(new Uint8Array(data));
    this.cdromSignal.value = file;
}

ejectCdrom(): void {
    this.computer.eject_cdrom();
    this.cdromSignal.value = null;
}
```

**`wasm/www/src/components/Toolbar.tsx`** — add CD-ROM to `driveConfigs`:
```typescript
{ label: 'D:', drive: 'cdrom' as DriveId, icon: 'bi-disc-fill', canEject: true }
```

The `DriveButton` component requires no changes — it is already generic over the drive config shape.

**`wasm/www/src/components/DrivePanel.tsx`** — the CD-ROM panel differs from floppy in two ways:
- No **Save image** button (CD-ROMs are read-only)
- No **Boot drive** toggle (booting from CD is out of scope for now)

Gate these by checking `drive === 'cdrom'` where the buttons are rendered, or pass a `readOnly` prop from the drive config. The file picker filter changes to `accept=".iso"`.

**`wasm/www/public/images.json`** — the `"cdrom": []` key already exists. Populate it with any preset ISO images as they become available.

The CD-ROM panel is only shown when `sound_blaster_port` is set in `WasmComputerConfig`. If the device is not configured, `DriveButton` for D: should be hidden or disabled, mirroring how the HDD button is conditionally shown.

---

## Phase 4 — Tests (`core/src/test_data/` and `core/src/tests/`)

### 4.1 New file: `core/src/test_data/sbpcd_test.asm`

A small COM program that:
1. Sends a NOP command (`0x00`) to port `0x230` and reads the status byte — verifies drive presence
2. Sends a Read Disc Info command (`0x12`) and reads the 6-byte result — verifies disc is present and checks last track number and total size
3. Sends a Seek to LBA 0 (MSF `00:02:00`) and then Read 1 sector (`0x0B`) — reads the first 2048 bytes of the ISO into a known memory address
4. Reads a known byte from the ISO's primary volume descriptor at offset 1 (should be `0x43` = `'C'`) and writes it to a video memory address

The test program exits via `INT 20h` after writing result bytes to memory locations that the Rust test can inspect.

### 4.2 New file: `core/src/tests/devices/sound_blaster_cdrom.rs`

```rust
#[test]
fn test_sbpcd_nop_no_disc() {
    // Create computer with SB CD device, no disc loaded
    // Run NOP command, check status byte has disc-absent bit
}

#[test]
fn test_sbpcd_nop_with_disc() {
    // Create computer with SB CD device, minimal ISO loaded
    // Run NOP command, check status byte has disc-present bit
}

#[test]
fn test_sbpcd_read_disc_info() {
    // Load a crafted minimal ISO (just a primary volume descriptor)
    // Send Read Disc Info, verify result bytes match expected track count / size
}

#[test]
fn test_sbpcd_read_sector() {
    // Load an ISO where sector 16 (LBA 16) is a known PVD
    // Seek + Read 1 sector, verify first 8 bytes of result
    // (ISO PVD starts with 0x01, "CD001", 0x01, 0x00)
}

#[test]
fn test_sbpcd_full_program() {
    // Run sbpcd_test.com against a crafted ISO
    // Inspect memory after program exits for expected bytes
}
```

Helper for tests — a function `make_minimal_iso(data: &[u8]) -> Vec<u8>` that builds a syntactically valid ISO 9660 image with a given data payload at the root, sized to a round number of 2048-byte sectors.

### 4.3 Modify `core/src/tests/devices/mod.rs`

Add `pub mod sound_blaster_cdrom;`.

---

## Files changed summary

| File | Change |
|------|--------|
| `core/src/disk/cdrom.rs` | **New** — `CdromBackend` trait + `BackedCdrom` |
| `core/src/disk/mod.rs` | Add `pub mod cdrom` |
| `core/src/devices/mod.rs` | Add `CdromController` trait (with `irq_line()`) + `CdromControllerRef` type alias; export `sound_blaster_cdrom` |
| `core/src/devices/sound_blaster_cdrom.rs` | **New** — implements `Device` + `CdromController`; configurable `irq_line` field |
| `core/src/devices/pic.rs` | Add `cdrom: Option<CdromControllerRef>` field; poll IRQ generically via `cdrom.borrow().irq_line()` |
| `core/src/bus.rs` | Add `cdrom_controller: Option<CdromControllerRef>` field; `add_cdrom_controller<T>()` |
| `core/src/computer.rs` | Expose generic `add_cdrom_controller<T>()`, `load/eject_cdrom_disc()` |
| `core/src/tests/devices/sound_blaster_cdrom.rs` | **New** — unit tests |
| `core/src/tests/devices/mod.rs` | Register test module |
| `core/src/test_data/sbpcd_test.asm` | **New** — assembly test program |
| `native-common/src/cli.rs` | `--sound-blaster-port PORT` (default `0x230`), `--disable-sound-blaster-cd`, `--sound-blaster-irq IRQ` (default `5`), `--cdrom FILE` |
| `native-common/src/lib.rs` | Create `SoundBlasterCdrom` with configurable port + IRQ; enabled by default |
| `native-cli/src/command_mode.rs` | Add `load cd <path>` and `eject cd` commands |
| `native-gui/src/menu.rs` | Add `InsertCdrom` / `EjectCdrom` actions and CD-ROM submenu; grey out when `--disable-sound-blaster-cd` |
| `native-gui/src/main.rs` | Handle `InsertCdrom` / `EjectCdrom` actions in `process_egui_frame()` |
| `wasm/src/lib.rs` | Add `sound_blaster_port` to config, `load_cdrom_image()`, `insert_cdrom()`, `eject_cdrom()` methods |
| `wasm/www/src/state.ts` | Add `cdromSignal`, `insertCdrom()`, `ejectCdrom()` |
| `wasm/www/src/components/Toolbar.tsx` | Add D: drive entry to `driveConfigs` |
| `wasm/www/src/components/DrivePanel.tsx` | Gate Save/Boot controls behind `readOnly` prop for CD-ROM |
| `wasm/www/public/images.json` | Populate `"cdrom"` array with preset ISOs as available |

---


---

## Future

### Read sector timing

Real drives take ~200ms per sector. Instant PIO delivery is currently used and is likely fine for compatibility, but some software may rely on timing delays. If issues arise, a cycle-counted delay between sector loads could be added to `stream_read_byte()`.

### Audio playback

Commands `0x0E` (Play Audio) and `0x0F` (Play Audio MSF) currently return an error status so software degrades gracefully. Full audio would require decoding Red Book audio frames from the ISO and feeding them through the existing `PcmRingBuffer` / Rodio pipeline. This is a significant addition and is deferred until there is a real use case.

### Full Sound Blaster card

On real hardware the CD-ROM interface is part of the Sound Blaster card — same PCB, same IRQ config. When SB audio (DSP/PCM, mixer, MPU-401) is added, `SoundBlasterCdrom` should be absorbed into a unified `SoundBlaster` struct rather than registering two separate devices.

#### Unified device traits

`SoundBlaster` would implement all three traits:

```rust
impl Device          for SoundBlaster { /* all SB IO ports */ }
impl SoundCard       for SoundBlaster { /* DSP/PCM sample generation */ }
impl CdromController for SoundBlaster { /* CD commands, delegated from current SoundBlasterCdrom */ }
```

The `CdromController` trait needs no changes — `SoundBlaster` simply adds the impl alongside `SoundCard`.

#### Shared inner state

`Rc<RefCell<T>>` cannot be coerced to two different `dyn Trait` fat pointers simultaneously. The solution is to move all state into a `SoundBlasterInner` struct held behind a shared `Rc<RefCell<SoundBlasterInner>>`, with `SoundBlaster` being a thin shell that clones that `Rc` when registering:

```rust
pub(crate) fn add_sound_blaster<T: Device + SoundCard + CdromController + 'static>(
    &mut self,
    device: T,
) {
    let rc = Rc::new(RefCell::new(device));
    self.devices.push(rc.clone());
    self.sound_card = Some(rc.clone());
    self.cdrom_controller = Some(rc.clone());
    self.pic.borrow_mut().set_cdrom(rc);
}
```

#### Migration steps

1. Rename `SoundBlasterCdrom` → `SoundBlaster`; move CD logic into a `SoundBlasterCdromInner` sub-struct
2. Implement `SoundCard` on `SoundBlaster` (DSP/PCM/mixer ports at `0x220+`)
3. Replace separate `add_cdrom_controller()` calls in native-common and WASM with a single `add_sound_blaster()`
4. Retire the standalone `Adlib` device — SB has OPL2 on-board at the same ports (`0x388/0x389`)

The `CdromController` trait, `CdromControllerRef`, and all callers of `load_cdrom_disc()` / `eject_cdrom_disc()` require no changes.

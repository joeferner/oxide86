# CD-ROM Support Plan

## Context

CD-ROM support was listed in TODO.md as a desired feature. This adds the ability to load ISO 9660 disc images and access them from DOS programs running in the emulator. It includes native GUI menus to insert/eject CD-ROMs, a WASM UI row for the same, INT 13h sector reads, and a minimal MSCDEX shim (INT 2Fh AH=15h) so DOS programs that check for MSCDEX can detect the CD-ROM.

## Architecture

- CD-ROM drive numbers: **0xE0–0xE3** (up to 4 slots)
- ISO 9660 reader lives in a new `core/src/cdrom.rs` (no new crates; hand-rolled, WASM-safe)
- File access bypasses `fatfs` — file contents are read directly from ISO extents into a buffer stored in `FileHandle`
- `DriveManager` routes all file/dir operations for `0xE0+` drives to the ISO 9660 path

---

## Files to Create

### `core/src/cdrom.rs` (NEW)
```
pub const CD_SECTOR_SIZE: usize = 2048;

pub struct CdRomImage { data: Vec<u8>, volume_label: String, root_extent_lba: u32, root_data_length: u32 }

impl CdRomImage:
  pub fn new(data: Vec<u8>) -> Result<Self, String>
      - validates data.len() >= 17 * CD_SECTOR_SIZE
      - reads PVD at sector 16, checks "CD001" magic + type byte == 1
      - extracts volume_label (offset 40, 32 bytes, trimmed)
      - extracts root directory: extent_lba (offset 158, 4 LE bytes), data_length (offset 166, 4 LE bytes)
  pub fn read_sector(&self, lba: u32) -> Result<[u8; CD_SECTOR_SIZE], String>
  pub fn get_volume_label(&self) -> &str
  pub fn list_directory(&self, path: &str) -> Result<Vec<IsoEntry>, String>
      - splits path on '/' and '\\', walks from root_extent_lba
      - case-insensitive component matching
  pub fn find_entry(&self, path: &str) -> Result<IsoEntry, String>
  pub fn read_file(&self, path: &str) -> Result<Vec<u8>, String>
  fn read_dir_entries(&self, extent_lba: u32, data_length: u32) -> Result<Vec<IsoEntry>, String>
      - iterates 2048-byte sectors; record_length == 0 → skip to next sector
      - skips \x00 (dot) and \x01 (dotdot) identifiers
  fn parse_dir_record(buf: &[u8]) -> Option<IsoEntry>
      - byte 0: record_length; byte 25: flags (bit1 = dir)
      - bytes [2..6]: extent_lba LE; bytes [10..14]: data_length LE
      - byte 32: id_len; bytes [33..33+id_len]: file identifier
      - strip ";1" suffix from file names; uppercase
  pub fn dos_date_from_iso(date: &[u8; 7]) -> (u16, u16)  // → (dos_date, dos_time)

pub struct IsoEntry { pub name: String, pub extent_lba: u32, pub data_length: u32, pub is_dir: bool, pub recording_date: [u8; 7] }
```

---

## Files to Modify

### `core/src/lib.rs`
1. Add `pub mod cdrom;` to module list
2. Add `pub use crate::cdrom::{CdRomImage, CD_SECTOR_SIZE};` to re-exports
3. Extend `DriveNumber` impl:
   - `pub const CDROM_BASE: u8 = 0xE0;`
   - `pub fn cdrom(slot: u8) -> Self { Self(Self::CDROM_BASE + slot) }`
   - `pub fn is_cdrom(&self) -> bool { self.0 >= 0xE0 && self.0 < 0xE4 }`
   - `pub fn cdrom_slot(&self) -> u8 { self.0 - 0xE0 }`
   - Fix `is_hard_drive()` to add `&& self.0 < 0xE0` exclusion

### `core/src/drive_manager.rs`
1. Add `use crate::cdrom::{CdRomImage, IsoEntry};` and `use crate::disk::SECTOR_SIZE;`
2. Extend `FileHandle` struct:
   ```rust
   cdrom_data: Option<Vec<u8>>,   // buffered file content for CD-ROM files
   cdrom_size: u64,               // total size (for seek FromEnd)
   ```
   Update every `FileHandle { ... }` constructor to include `cdrom_data: None, cdrom_size: 0`. Also update `file_duplicate()` to clone `cdrom_data`.
3. Add `cdrom_drives: [Option<CdRomImage>; 4]` to `DriveManager` struct; initialize as `[None, None, None, None]`
4. New methods on `DriveManager`:
   - `pub fn insert_cdrom(&mut self, slot: u8, image: CdRomImage) -> DriveNumber`
     - closes any open file handles on that drive, stores image, returns `DriveNumber::cdrom(slot)`
   - `pub fn eject_cdrom(&mut self, slot: u8) -> Option<CdRomImage>`
     - closes open handles on drive, takes and returns the image
   - `pub fn has_cdrom(&self, slot: u8) -> bool`
   - `pub fn cdrom_count(&self) -> u8`
   - `pub fn cdrom_read_sector_as_512(&self, drive: DriveNumber, lba_512: usize) -> Result<[u8; SECTOR_SIZE], DiskError>`
     - maps 512-byte LBA to CD sector: `cd_lba = lba_512 / 4`, `offset = (lba_512 % 4) * 512`
5. Modify `file_open()`:
   ```rust
   if drive.is_cdrom() {
       let image = self.cdrom_drives[drive.cdrom_slot() as usize].as_ref()
           .ok_or(DosError::InvalidDrive)?;
       let data = image.read_file(&path).map_err(|_| DosError::FileNotFound)?;
       let size = data.len() as u64;
       let handle = self.allocate_handle();
       self.open_files.insert(handle, FileHandle { drive, path, position: 0,
           access_mode, cdrom_data: Some(data), cdrom_size: size });
       return Ok(handle);
   }
   ```
6. Modify `file_read()`: if `fh.cdrom_data.is_some()`, serve from buffer slice and advance position
7. Modify `file_write()`: return `Err(DosError::AccessDenied)` if `fh.drive.is_cdrom()`
8. Modify `file_seek()`: if `fh.drive.is_cdrom()`, compute new position using `cdrom_size`, update `fh.position`
9. Modify `find_first()`: if `drive.is_cdrom()`, call `cdrom_find_first()` which uses `image.list_directory()` and converts `IsoEntry` → `FindData`; store results in `searches` HashMap as usual
10. Modify `find_next()`: existing code already works from `searches` HashMap; no change needed
11. Reject CD-ROM drives in `file_create()`, `dir_create()`, `dir_remove()`, `dir_change()` with `DosError::AccessDenied`

### `core/src/cpu/bios/mod.rs`
Add delegate methods to `Bios`:
```rust
pub fn insert_cdrom(&mut self, slot: u8, image: CdRomImage) -> DriveNumber {
    self.shared.drive_manager.insert_cdrom(slot, image)
}
pub fn eject_cdrom(&mut self, slot: u8) -> Option<CdRomImage> { ... }
pub fn has_cdrom(&self, slot: u8) -> bool { ... }
pub fn cdrom_count(&self) -> u8 { ... }
```
Add `use crate::cdrom::CdRomImage;` import.

### `core/src/cpu/bios/int13.rs`
Add CD-ROM routing at top of `handle_int13()` (before the `match function`):
```rust
let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8);
if drive.is_cdrom() {
    self.handle_int13_cdrom(bus, io, drive);
    return;
}
```
Add private `fn handle_int13_cdrom(bus, io, drive)` handling:
- `AH=00h`: reset → success
- `AH=01h`: get status → return `io.shared.last_disk_status`
- `AH=02h`: read sectors — use `io.shared.drive_manager.cdrom_read_sector_as_512()`, write to ES:BX
  - CHS→flat 512-LBA: `lba_512 = cylinder * 75 + (sector - 1)` (75 spt CD-ROM convention)
- `AH=15h`: get disk type → `AH=0x03` (CD-ROM), `CX:DX=0` if drive has disc; else CF=1
- others → warn, set carry, `DiskError::InvalidCommand`

### `core/src/cpu/bios/int2f.rs`
1. Update `handle_int2f` signature: `fn handle_int2f(&mut self, bus: &mut Bus, io: &mut super::Bios)`
2. Update call site in `mod.rs`: `0x2F => self.handle_int2f(bus, io),`
3. Add `0x15 => self.int2f_mscdex(bus, io, subfunction),` to the match
4. Add `fn int2f_mscdex(bus, io, subfunction)`:
   - `AL=00h`: install check → if `cdrom_count > 0`: `AX=0xADAD, BX=count`; else `AX=0, BX=0`
   - `AL=0Bh`: drive check → `AX=0xADAD, BX=0` if DOS drive maps to CD-ROM slot, else `AX=0`
     (slot N maps to DOS letter `2 + hard_drive_count + N`)
   - `AL=0Ch`: version → `BX=0x0200, CX=0x0000`
   - `AL=0Dh`: drive letters → write first DOS letter for each active CD-ROM slot to ES:BX buffer
   - others → warn

### `native-common/src/cli.rs`
Add to `CommonCli`:
```rust
/// CD-ROM ISO image(s) - can be specified multiple times (up to 4)
#[arg(long = "cdrom", action = clap::ArgAction::Append)]
pub cdroms: Vec<String>,
```

### `native-common/src/setup.rs`
Add and export `load_cdroms<V: VideoController>(computer, cdroms: &[String]) -> Result<()>`:
- reads each file, calls `CdRomImage::new(data)`, calls `computer.bios_mut().insert_cdrom(slot, image)`
- logs each loaded CD-ROM

Call from both `native-cli` and `native-gui` `main.rs` after `load_disks()`.

### `native-gui/src/menu.rs`
1. Add `InsertCdRom, EjectCdRom` to `MenuAction` enum
2. Add `cdrom_present: bool` to `AppMenu`; add `update_cdrom_state(&mut self, present: bool)`
3. In `render()`, add "CD-ROM" menu after "Floppy":
   ```rust
   ui.menu_button("CD-ROM", |ui| {
       if ui.button("Insert ISO...").clicked() { action = Some(MenuAction::InsertCdRom); ui.close_menu(); }
       if ui.add_enabled(self.cdrom_present, egui::Button::new("Eject CD-ROM")).clicked() {
           action = Some(MenuAction::EjectCdRom); ui.close_menu();
       }
   });
   ```
4. Update `is_debug_action()` and `drive_number()` to not panic on new variants

### `native-gui/src/main.rs`
1. Add `cdrom_present: bool` to `AppState`; initialize from `!cli.common.cdroms.is_empty()`
2. In `process_egui_frame()`, handle new menu actions:
   - `InsertCdRom` → `show_insert_cdrom_dialog(computer, &mut app_state.cdrom_present, &mut app_state.menu, &mut app_state.notification)`
   - `EjectCdRom` → `eject_cdrom(computer, &mut app_state.cdrom_present, &mut app_state.menu, &mut app_state.notification)`
3. Add `fn show_insert_cdrom_dialog(...)`:
   - `rfd::FileDialog::new().add_filter("ISO Images", &["iso"]).pick_file()`
   - on selection: call `load_and_insert_cdrom(0, &path, ...)`
4. Add `fn load_and_insert_cdrom(slot, path, computer, cdrom_present, menu, notification)`:
   - `std::fs::read(path)`, `CdRomImage::new(data)`, `computer.bios_mut().insert_cdrom(slot, image)`
   - update state + show success/error notification
5. Add `fn eject_cdrom(...)`: calls `computer.bios_mut().eject_cdrom(0)`, updates state + notification
6. Call `load_cdroms(&mut computer, &cli.common.cdroms)?` after `load_disks()`

### `wasm/src/lib.rs`
Add three exported methods to `Emu86Computer`:
```rust
#[wasm_bindgen] pub fn load_cdrom(&mut self, slot: u8, data: Vec<u8>) -> Result<(), JsValue>
#[wasm_bindgen] pub fn eject_cdrom_slot(&mut self, slot: u8) -> Result<(), JsValue>
#[wasm_bindgen] pub fn cdrom_count(&self) -> u8
```
Add `use emu86_core::CdRomImage;`. If `computer.bios()` (immutable) doesn't exist in `Computer`, add it in `core/src/computer.rs`.

### `wasm/www/src/components/DriveControl.tsx`
Add CD-ROM row after Hard Drive C: section:
- `cdromFile` signal
- `handleCdRomChange`: reads file, calls `computer.value?.load_cdrom(0, data)`
- `handleEjectCdRom`: calls `computer.value?.eject_cdrom_slot(0)`
- Render: label "CD-ROM Drive:", `<FileButton accept=".iso">`, eject `<ActionIcon>`
- Add re-mount of `cdromFile` in `useSignalEffect` block

---

## Edge Cases
- Empty CD slot: all operations return appropriate error (`FileNotFound`, `DriveNotReady`)
- Path case insensitivity: use `.eq_ignore_ascii_case()` in component matching
- Version suffix stripping (`;1`): strip from `;` onwards in `parse_dir_record`
- Dot entries (`\x00`, `\x01`): skip in directory enumeration
- `record_length == 0` in directory: skip to next 2048-byte boundary
- `FileHandle` constructors: all existing sites need `cdrom_data: None, cdrom_size: 0`
- `file_duplicate()`: clone `cdrom_data` Vec

---

## Test Program

Create `test-programs/cdrom/cdrom_detect.asm` and update `test-programs/README.md`:
```nasm
; MSCDEX detection test
mov ax, 0x1500
xor bx, bx
int 0x2F
; AX=0xADAD → installed, BX=count
```

---

## Verification
1. `./scripts/pre-commit.sh` — must pass (build + clippy)
2. Native CLI: `cargo run -p emu86-native-cli -- --cdrom game.iso program.com`
3. Native GUI: `cargo run -p emu86-native-gui -- --cdrom game.iso --boot --floppy-a dos.img`
   - CD-ROM menu → Insert ISO..., Eject CD-ROM
4. WASM: load UI, use CD-ROM file picker, verify status shows "Loaded CD-ROM: name.iso"
5. Functional: INT 2Fh AH=15h AL=00h returns AX=0xADAD; INT 13h AH=15h for 0xE0 returns AH=03h; file open/read from ISO works; file write returns AccessDenied

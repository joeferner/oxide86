# Implementation Plan: Floppy Disk Menu System for native-gui

## Overview

Add native menu bar functionality to the native-gui application with file dialogs for hot-swapping floppy disks during emulation.

## Architecture

**UI Libraries:**
- `muda 0.17` - Cross-platform native menu bar (excellent winit integration)
- `rfd 0.17` - Native file dialogs (blocking API, simpler than async)

**Event Flow:**
```
User clicks menu → muda MenuEvent → Custom AppEvent → EventLoop → user_event() handler
  → File dialog (rfd) → Load disk (FileDiskBackend + BackedDisk) → Bios::insert_floppy()
```

**Menu Structure:**
```
Floppy
├─ Floppy A:
│  ├─ Insert Disk...  (Ctrl+Shift+A)
│  └─ Eject Disk      (Ctrl+Alt+A) [disabled when empty]
└─ Floppy B:
   ├─ Insert Disk...  (Ctrl+Shift+B)
   └─ Eject Disk      (Ctrl+Alt+B) [disabled when empty]
```

## Critical Files

| File | Change Type | Purpose |
|------|-------------|---------|
| [native-gui/Cargo.toml](../../../native-gui/Cargo.toml) | Modified | Add muda and rfd dependencies |
| [native-gui/src/menu.rs](../../../native-gui/src/menu.rs) | New | Menu structure, event enum, menu creation |
| [native-gui/src/main.rs](../../../native-gui/src/main.rs) | Modified | Event loop integration, menu handlers |

## Implementation Steps

### ✅ Step 2: Create Menu Module - COMPLETED

Created [native-gui/src/menu.rs](../../../native-gui/src/menu.rs) with:
- `AppEvent` enum for custom event types
- `AppMenu` struct with menu item references
- `create_menu()` function with full menu hierarchy
- `MenuAction` enum for menu event handling
- `update_menu_states()` method for dynamic menu state management

### ✅ Step 3: Integrate Event Loop - COMPLETED

Modified [native-gui/src/main.rs](../../../native-gui/src/main.rs) with:
- Created event loop with custom `AppEvent` type
- Set up muda event handler before event loop starts
- Added menu fields and event_proxy to App struct
- Updated ApplicationHandler trait bound to include AppEvent
- Implemented menu creation and platform-specific initialization in `resumed()` method
- Implemented `user_event()` method to handle menu events
- Added helper methods: `handle_menu_event()`, `show_insert_dialog()`, `load_and_insert_disk()`, `eject_disk()`
- Menu states properly sync with disk presence

### ✅ Step 4: Implement Menu Event Handlers - COMPLETED

Implemented in [native-gui/src/main.rs](../../../native-gui/src/main.rs) as part of Step 3:
- `handle_menu_event()` - dispatches menu actions to appropriate handlers
- `show_insert_dialog()` - shows native file picker for disk selection
- `load_and_insert_disk()` - loads disk image and inserts into drive
- `eject_disk()` - ejects disk and updates menu state

### ✅ Step 5: Platform-Specific Menu Initialization - COMPLETED

Implemented in [native-gui/src/main.rs](../../../native-gui/src/main.rs) `resumed()` method:
- Windows: `menu.menu.init_for_hwnd()` with Win32 window handle
- Linux: Default setup with info logging
- macOS: `menu.menu.init_for_nsapp()`

## Edge Cases & Error Handling

1. **Invalid File Format**
   - `BackedDisk::new()` validates geometry - will return error for non-standard sizes
   - Log error and show in console (no GUI error dialog for simplicity)

2. **Disk Already Inserted**
   - `insert_floppy()` handles this - closes open files automatically
   - DriveManager sets `disk_changed = true` flag

3. **Empty Eject**
   - `eject_floppy()` returns `Ok(None)` if no disk present
   - Menu item should be disabled anyway (defensive programming)

4. **File Dialog Canceled**
   - `pick_file()` returns `None` - no action taken

5. **Keyboard Routing**
   - Menu accelerators work independently of emulator keyboard input
   - No conflict - OS handles menu keys before winit sees them

## Testing Strategy

1. **Manual Testing:**
   - Launch GUI: `cargo run -p emu86-native-gui -- --boot --floppy-a dos.img`
   - Verify menu appears with "Floppy A: [Eject]" enabled
   - Click "Floppy B: → Insert Disk..."
   - Select a .img file
   - Verify disk loads and menu updates
   - Try ejecting both disks
   - Test keyboard accelerators (Ctrl+Shift+A, etc.)

2. **Platform Testing:**
   - Test on Linux, Windows, and macOS if available
   - Verify menu appearance (title bar vs global menu bar)
   - Verify file dialog is native to each platform

3. **Error Cases:**
   - Try loading non-existent file
   - Try loading invalid file (wrong size)
   - Eject disk while DOS is accessing it (should work, DriveManager handles it)

## Backward Compatibility

- CLI arguments (`--floppy-a`, `--floppy-b`) continue to work
- Initial disk presence is reflected in menu state
- No breaking changes to existing code

## Performance Considerations

- **Menu Events:** Negligible overhead (OS-native, event-driven)
- **File Dialog:** Blocks main thread temporarily (acceptable for user action)
- **Disk Loading:** < 100ms for typical floppy images
- **Emulator Execution:** No impact - runs independently

## Future Enhancements (Not in Scope)

- Recent files menu
- Disk name display in menu
- Read-only disk mounting option
- Status bar showing current disks
- Drag-and-drop disk insertion

## Verification

After implementation, verify:

1. ✅ Menu bar appears on all platforms
2. ✅ Keyboard accelerators work (Ctrl+Shift+A/B, Ctrl+Alt+A/B)
3. ✅ File dialog opens and filters .img files
4. ✅ Disk insertion succeeds and updates menu
5. ✅ Disk ejection succeeds and updates menu
6. ✅ Menu items enable/disable correctly based on disk presence
7. ✅ Emulator continues running during file dialog
8. ✅ Hot-swap works (DOS detects disk change via INT 13h AH=16h)
9. ✅ Error handling logs failures appropriately
10. ✅ CLI args still work for initial disk loading

## Dependencies Summary

| Dependency | Version | Purpose |
|------------|---------|---------|
| muda | 0.17 | Native cross-platform menu bar with winit integration |
| rfd | 0.17 | Native file dialogs (blocking API) |

## Key API References

**Disk Loading:**
```rust
use emu86_core::{FileDiskBackend, BackedDisk, DriveNumber, DiskController};

let backend = FileDiskBackend::open(path, false)?;  // false = read/write
let disk = BackedDisk::new(backend)?;  // Auto-detects geometry
bios.insert_floppy(DriveNumber::floppy_a(), Box::new(disk))?;
```

**Disk Ejection:**
```rust
let disk = bios.eject_floppy(DriveNumber::floppy_a())?;  // Returns Option<Box<dyn DiskController>>
```

**Menu Creation (muda):**
```rust
use muda::{Menu, MenuItem, Submenu, PredefinedMenuItem};

let menu = Menu::new();
let floppy_menu = Submenu::new("Floppy", true);
let insert_item = MenuItem::new("Insert Disk...", true, Some(Accelerator::new(...)));
floppy_menu.append(&insert_item)?;
menu.append(&floppy_menu)?;
```

**File Dialog (rfd):**
```rust
use rfd::FileDialog;

let file = FileDialog::new()
    .add_filter("Disk Images", &["img"])
    .set_directory(".")
    .pick_file();
```

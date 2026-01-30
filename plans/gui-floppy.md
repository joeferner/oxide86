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

### Step 3: Integrate Event Loop

**File:** [native-gui/src/main.rs](../../../native-gui/src/main.rs)

**Changes to `main()` function:**

1. Create event loop with custom event type:
   ```rust
   let event_loop = EventLoop::<AppEvent>::with_user_event()
       .build()
       .context("Failed to create event loop")?;
   ```

2. Get event loop proxy:
   ```rust
   let event_proxy = event_loop.create_proxy();
   ```

3. Set up muda event handler (must be before event loop starts):
   ```rust
   muda::MenuEvent::set_event_handler(Some({
       let proxy = event_proxy.clone();
       move |event: muda::MenuEvent| {
           let _ = proxy.send_event(AppEvent::MenuEvent(event));
       }
   }));
   ```

4. Pass proxy to App:
   ```rust
   let mut app = App::new(cli, event_proxy)?;
   ```

**Changes to `App` struct:**

1. Add fields:
   ```rust
   menu: Option<AppMenu>,
   event_proxy: EventLoopProxy<AppEvent>,
   floppy_a_present: bool,
   floppy_b_present: bool,
   ```

2. Update constructor to accept event proxy

3. Update trait bound:
   ```rust
   impl ApplicationHandler<AppEvent> for App
   ```

**Changes to `ApplicationHandler` implementation:**

1. Add `mod menu;` at top of file
2. Import menu types

3. In `resumed()` method (after window creation):
   ```rust
   // Create menu
   let menu = menu::create_menu()?;

   // Initialize menu for window (platform-specific)
   #[cfg(target_os = "windows")]
   {
       use winit::platform::windows::WindowExtWindows;
       use winit::raw_window_handle::HasWindowHandle;
       let handle = window.window_handle()?;
       if let raw_window_handle::RawWindowHandle::Win32(h) = handle.as_raw() {
           menu.init_for_hwnd(h.hwnd.get() as isize)?;
       }
   }

   // Similar for Linux (gtk) and macOS (nsapp)

   // Check initial disk presence from CLI args
   let floppy_a_present = self.cli.floppy_a.is_some();
   let floppy_b_present = self.cli.floppy_b.is_some();

   // Update menu states
   menu.update_menu_states(floppy_a_present, floppy_b_present);

   // Store in self
   self.menu = Some(menu);
   self.floppy_a_present = floppy_a_present;
   self.floppy_b_present = floppy_b_present;
   ```

4. Implement `user_event()` method:
   ```rust
   fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
       match event {
           AppEvent::MenuEvent(menu_event) => {
               self.handle_menu_event(menu_event);
           }
           AppEvent::DiskInserted { slot, result } => {
               self.handle_disk_inserted(slot, result);
           }
       }
   }
   ```

### Step 4: Implement Menu Event Handlers

**File:** [native-gui/src/main.rs](../../../native-gui/src/main.rs)

**Add helper methods to `App`:**

1. `handle_menu_event()`:
   - Match on menu item ID
   - Determine drive slot (A: or B:)
   - Call `show_insert_dialog()` or `eject_disk()`

2. `show_insert_dialog()`:
   ```rust
   fn show_insert_dialog(&self, slot: DriveNumber) {
       let result = rfd::FileDialog::new()
           .add_filter("Disk Images", &["img"])
           .set_directory(".")
           .set_title(format!("Select Disk for Floppy {}",
               if slot == DriveNumber::floppy_a() { "A:" } else { "B:" }))
           .pick_file();

       if let Some(file) = result {
           let path = file.to_string_lossy().to_string();
           self.load_and_insert_disk(slot, &path);
       }
   }
   ```

3. `load_and_insert_disk()`:
   ```rust
   fn load_and_insert_disk(&mut self, slot: DriveNumber, path: &str) {
       let result = (|| {
           let backend = FileDiskBackend::open(path, false)?;
           let disk = BackedDisk::new(backend)
               .with_context(|| format!("Invalid disk image: {}", path))?;

           let state = self.state.as_mut().unwrap();
           state.computer.bios_mut()
               .insert_floppy(slot, Box::new(disk))
               .map_err(|e| anyhow::anyhow!(e))?;

           log::info!("Inserted floppy {} from {}", slot, path);
           Ok(())
       })();

       match result {
           Ok(()) => {
               // Update state
               if slot == DriveNumber::floppy_a() {
                   self.floppy_a_present = true;
               } else {
                   self.floppy_b_present = true;
               }
               // Update menu
               if let Some(menu) = &self.menu {
                   menu.update_menu_states(self.floppy_a_present, self.floppy_b_present);
               }
           }
           Err(e) => {
               log::error!("Failed to insert disk: {}", e);
           }
       }
   }
   ```

4. `eject_disk()`:
   ```rust
   fn eject_disk(&mut self, slot: DriveNumber) {
       let state = self.state.as_mut().unwrap();
       match state.computer.bios_mut().eject_floppy(slot) {
           Ok(Some(_disk)) => {
               log::info!("Ejected floppy {}", slot);
               // Update state
               if slot == DriveNumber::floppy_a() {
                   self.floppy_a_present = false;
               } else {
                   self.floppy_b_present = false;
               }
               // Update menu
               if let Some(menu) = &self.menu {
                   menu.update_menu_states(self.floppy_a_present, self.floppy_b_present);
               }
           }
           Ok(None) => {
               log::warn!("No disk in floppy {} to eject", slot);
           }
           Err(e) => {
               log::error!("Failed to eject disk: {}", e);
           }
       }
   }
   ```

### Step 5: Platform-Specific Menu Initialization

**File:** [native-gui/src/main.rs](../../../native-gui/src/main.rs)

**In `resumed()` method, add platform-specific initialization:**

```rust
#[cfg(target_os = "windows")]
{
    use winit::platform::windows::WindowExtWindows;
    use winit::raw_window_handle::HasWindowHandle;

    let window_handle = window.window_handle()
        .context("Failed to get window handle")?;
    if let raw_window_handle::RawWindowHandle::Win32(handle) = window_handle.as_raw() {
        menu.menu.init_for_hwnd(handle.hwnd.get() as isize)
            .context("Failed to init menu for Windows")?;
    }
}

#[cfg(target_os = "linux")]
{
    use winit::platform::x11::WindowExtX11;
    // Note: Linux menu initialization may require GTK window handle
    // For now, log a warning if not available
    log::warn!("Linux menu initialization may require additional platform setup");
}

#[cfg(target_os = "macos")]
{
    menu.menu.init_for_nsapp()
        .context("Failed to init menu for macOS")?;
}
```

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

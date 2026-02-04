# Plan: WASM Disk Management Features

## Overview
Add comprehensive disk editing and file management capabilities to the WASM interface, enabling users to create blank disks, download disk images, browse directory structures, and upload/download files.

## Requirements
1. Create new blank floppy (1.44MB, 720KB) and hard drive images
2. Download floppy and hard drive images to browser
3. Interface (modal dialog) to browse disk directory structure
4. Download individual files from disk to browser
5. Upload files/directories from browser into disk

## Key Design Decisions

### FAT Formatting Strategy
**Decision:** Use pre-formatted disk templates
**Rationale:** The fatfs crate is read/write only (no formatting). Implementing full FAT12/FAT16 formatting is complex. Pre-formatted templates provide standard formats with minimal code.

**Approach:**
- Create minimal formatted disk images using native tools (mformat/mkfs.msdos)
- Extract boot sector + FAT tables as byte array constants
- At runtime, copy template and resize for target geometry
- Supports: FAT12 1.44MB floppy, FAT12 720KB floppy, FAT16 hard drives

### UI Architecture
**Decision:** Modal dialog with tabbed/sectioned interface
**Rationale:** Keeps main UI clean, familiar pattern, allows operation while emulator paused

### Disk Access Pattern
**Decision:** Allow disk operations whether emulator is running or stopped
**Rationale:** Uses MemoryDiskBackend (all in-memory), no file locking issues, more flexible UX

### File Upload Handling
**Decision:** Single file upload with directory path support
**Rationale:** Browser directory API is inconsistent, single file is universally supported, can create directory structure from paths

## Architecture

```
JavaScript Layer (UI)
    ↓ (WASM bindings)
Rust WASM Methods (wasm/src/lib.rs)
    ↓ (uses existing)
DriveManager + fatfs (core/src/drive_manager.rs)
    ↓
MemoryDiskBackend (core/src/disk.rs)
```

## Implementation Phases

### Phase 1: FAT Templates Module

**New file:** `wasm/src/fat_templates.rs`

Create pre-formatted boot sector templates:
```rust
pub const FAT12_1440K_TEMPLATE: &[u8] = &[/* boot sector + FATs */];
pub const FAT12_720K_TEMPLATE: &[u8] = &[/* boot sector + FATs */];
pub const FAT16_10MB_TEMPLATE: &[u8] = &[/* boot sector + FATs */];

pub fn format_floppy_1440k() -> Vec<u8>
pub fn format_floppy_720k() -> Vec<u8>
pub fn format_hard_drive(size_mb: u32) -> Vec<u8>
```

**Generation steps:**
1. Use mformat/mkfs.msdos to create minimal formatted images on host
2. Extract first few sectors (boot sector + FAT tables)
3. Embed as const byte arrays
4. Pad to full geometry at runtime

**Modifications needed:**
- Add `mod fat_templates;` to `wasm/src/lib.rs`

### Phase 2: Core WASM Methods

**File:** `wasm/src/lib.rs`

Add methods to `Emu86Computer` struct:

#### Disk Creation Methods
```rust
#[wasm_bindgen]
pub fn create_blank_floppy_1440k(&mut self) -> Result<(), JsValue>
pub fn create_blank_floppy_720k(&mut self) -> Result<(), JsValue>
pub fn create_blank_hard_drive(&mut self, size_mb: u32) -> Result<(), JsValue>
```

Implementation pattern:
- Get formatted template from fat_templates module
- Create MemoryDiskBackend with template data
- Wrap in BackedDisk
- Insert into appropriate drive slot via DriveManager

#### Disk Download Methods
```rust
#[wasm_bindgen]
pub fn get_floppy_data(&self, drive: u8) -> Result<Vec<u8>, JsValue>
pub fn get_hard_drive_data(&self, drive_index: u8) -> Result<Vec<u8>, JsValue>
```

Implementation pattern:
- Access disk through DriveManager
- Read all sectors sequentially (0 to total_sectors)
- Build complete Vec<u8> image
- Return to JavaScript for blob creation

#### Directory Browsing Methods
```rust
#[wasm_bindgen]
pub fn list_directory(&mut self, drive: u8, path: String) -> Result<JsValue, JsValue>
pub fn get_current_directory(&self, drive: u8) -> Result<String, JsValue>
```

Implementation pattern:
- Build search pattern: "C:\PATH\*.*"
- Use DriveManager's find_first() and find_next()
- Convert FindData entries to JSON objects
- Return as JsValue using serde-wasm-bindgen

JSON structure:
```json
{
  "name": "filename.txt",
  "size": 1234,
  "isDirectory": false,
  "date": "2024-01-15",
  "time": "14:30:00",
  "attributes": 0x20
}
```

#### File Operation Methods
```rust
#[wasm_bindgen]
pub fn read_file_from_disk(&mut self, drive: u8, path: String) -> Result<Vec<u8>, JsValue>
pub fn write_file_to_disk(&mut self, drive: u8, path: String, data: Vec<u8>) -> Result<(), JsValue>
pub fn create_directory_on_disk(&mut self, drive: u8, path: String) -> Result<(), JsValue>
pub fn delete_from_disk(&mut self, drive: u8, path: String) -> Result<(), JsValue>
```

Implementation pattern (file read):
- Build full path: "C:\PATH\FILE.TXT"
- Use DriveManager's file_open() for reading
- Read in chunks (32KB) via file_read()
- Close handle via file_close()
- Return complete Vec<u8>

Implementation pattern (file write):
- Parse path to extract directories
- Create parent directories if needed (via dir_create)
- Use DriveManager's file_create()
- Write data in chunks (32KB) via file_write()
- Close handle via file_close()

**Dependencies to add to `wasm/Cargo.toml`:**
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde-wasm-bindgen = "0.6"
serde_json = "1.0"
```

### Phase 3: JavaScript UI Component

**New file:** `wasm/www/disk-manager.js`

Create modular disk management component:
```javascript
export class DiskManager {
    constructor(computer, updateStatusCallback)

    // UI initialization
    createUI()
    createDiskManagerButton()
    createModalDialog()

    // Disk operations
    async createBlankDisk(type, geometry)
    async downloadDisk(driveType, driveNumber)
    async browseDisk(drive)

    // File operations
    async downloadFile(drive, path)
    async uploadFile(drive, file, targetPath)
    async deleteFile(drive, path)

    // UI rendering
    showDiskBrowser(drive)
    renderDirectoryTree(entries, currentPath)
    renderFileList(entries)

    // Event handlers
    attachBrowserEventHandlers(drive, modal)
    handleFileSelection(event)
    handleDirectoryClick(path)
}
```

**Key UI methods:**

`createUI()`: Creates modal dialog structure
```javascript
<div class="disk-manager-modal">
  <div class="modal-dialog">
    <div class="modal-header">
      <h2>Disk Manager</h2>
      <button class="close-button">×</button>
    </div>
    <div class="modal-body">
      <div class="tabs">
        <button data-tab="create">Create Disk</button>
        <button data-tab="download">Download Disk</button>
        <button data-tab="browse">Browse Disk</button>
      </div>
      <div class="tab-content" id="tab-content">
        <!-- Dynamic content -->
      </div>
    </div>
  </div>
</div>
```

`renderFileList()`: Displays directory contents
```javascript
<ul class="file-list">
  <li class="directory" data-path="/DOCS">
    📁 DOCS <span class="date">2024-01-15</span>
  </li>
  <li class="file" data-path="/README.TXT">
    📄 README.TXT <span class="size">1.2 KB</span> <span class="date">2024-01-15</span>
  </li>
</ul>
```

`downloadDisk()`: Downloads disk image as blob
```javascript
async downloadDisk(driveType, driveNumber) {
    const data = driveType === 'floppy'
        ? await this.computer.get_floppy_data(driveNumber)
        : await this.computer.get_hard_drive_data(driveNumber);

    const blob = new Blob([data], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `drive_${driveType}_${driveNumber}.img`;
    a.click();
    URL.revokeObjectURL(url);
}
```

`uploadFile()`: Uploads file with directory creation
```javascript
async uploadFile(drive, file, targetPath) {
    const data = new Uint8Array(await file.arrayBuffer());
    await this.computer.write_file_to_disk(drive, targetPath, data);
    this.updateStatus(`Uploaded ${file.name} to ${targetPath}`);
}
```

### Phase 4: HTML/CSS Integration

**File:** `wasm/www/index.html`

**Modifications:**

1. Import disk-manager.js module
```html
<script type="module">
    import { DiskManager } from './disk-manager.js';
    let diskManager = null;

    async function main() {
        // ... existing initialization ...
        computer = new Emu86Computer('display');
        diskManager = new DiskManager(computer, updateStatus);
        diskManager.createUI();
        // ... rest of setup ...
    }
</script>
```

2. Add "Disk Manager" button in controls section
```html
<div class="disk-operations">
    <button id="disk-manager-btn" class="secondary">Disk Manager</button>
</div>
```

3. Add CSS styles for modal and file browser
```css
.disk-manager-modal {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal-dialog {
    background: #2a2a2a;
    border: 1px solid #444;
    border-radius: 8px;
    width: 80%;
    max-width: 800px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
}

.file-list {
    list-style: none;
    padding: 0;
    overflow-y: auto;
}

.file-list li {
    padding: 8px 12px;
    border-bottom: 1px solid #333;
    cursor: pointer;
}

.file-list li:hover {
    background: #3a3a3a;
}

.file-list .directory { font-weight: bold; }
```

4. Wire up event handlers
```javascript
document.getElementById('disk-manager-btn').addEventListener('click', () => {
    diskManager.showDiskBrowser(0); // Default to drive A
});
```

## Critical Files

### Files to Modify
1. **`wasm/src/lib.rs`** - Add all WASM methods to Emu86Computer
2. **`wasm/Cargo.toml`** - Add serde dependencies
3. **`wasm/www/index.html`** - Add UI button, modal structure, CSS, initialization
4. **`core/src/drive_manager.rs`** - Reference only (may need minor visibility adjustments)

### Files to Create
1. **`wasm/src/fat_templates.rs`** - Pre-formatted disk templates
2. **`wasm/www/disk-manager.js`** - Complete UI component module

## Implementation Order

1. **Day 1-2: FAT Templates**
   - Generate formatted disk images using native tools
   - Extract boot sectors and FAT tables
   - Create fat_templates.rs with constants
   - Test blank disk creation manually

2. **Day 3-4: WASM Methods (Part 1)**
   - Implement create_blank_* methods
   - Implement get_*_data methods
   - Test with browser console commands

3. **Day 5-6: WASM Methods (Part 2)**
   - Implement list_directory method
   - Implement read_file_from_disk method
   - Implement write_file_to_disk method
   - Test each method individually

4. **Day 7-8: JavaScript UI Component**
   - Create disk-manager.js skeleton
   - Implement modal dialog structure
   - Implement file list rendering
   - Test UI without backend integration

5. **Day 9-10: Integration & Testing**
   - Wire up all buttons and event handlers
   - Connect JavaScript UI to WASM methods
   - Test complete workflows end-to-end
   - Add error handling and status updates

6. **Day 11: Polish**
   - Add CSS styling and animations
   - Add progress indicators for large files
   - Add confirmation dialogs for destructive operations
   - Final testing of all features

## Verification Steps

### Manual Testing Checklist

1. **Disk Creation**
   - [ ] Create 1.44MB floppy, verify boots
   - [ ] Create 720KB floppy, verify boots
   - [ ] Create 10MB hard drive, verify recognizes partitions
   - [ ] Boot created disk, verify empty filesystem

2. **Disk Download**
   - [ ] Download floppy A with data
   - [ ] Re-upload same image, verify data persists
   - [ ] Download hard drive C
   - [ ] Verify downloaded .img file size matches geometry

3. **Directory Browsing**
   - [ ] Browse empty disk (shows no entries)
   - [ ] Browse disk with files (shows all files)
   - [ ] Browse nested directories (shows correct path)
   - [ ] Verify directory vs file icons

4. **File Download**
   - [ ] Download text file, verify contents
   - [ ] Download binary file, verify byte-for-byte
   - [ ] Download file from subdirectory
   - [ ] Download large file (>100KB)

5. **File Upload**
   - [ ] Upload text file to root
   - [ ] Upload file to subdirectory (creates dirs)
   - [ ] Upload multiple files
   - [ ] Upload large file (>1MB), verify no corruption
   - [ ] Boot emulator and verify files are accessible

6. **Error Handling**
   - [ ] Try to download from empty drive (shows error)
   - [ ] Upload to non-existent drive (shows error)
   - [ ] Upload to full disk (shows error)
   - [ ] Invalid filename characters (sanitized or rejected)

### Integration Testing
```bash
# Build WASM
cd wasm
./scripts/build.sh

# Serve locally
cd www
python3 -m http.server 8080

# Open browser to http://localhost:8080
# Run through manual testing checklist above
```

## Edge Cases & Error Handling

1. **Full Disk**: Write operations return DosError::DiskFull, show toast notification
2. **Invalid Paths**: Sanitize paths, reject . and .. navigation attempts
3. **Large Files**: Show progress bar for files >1MB, chunk operations
4. **Special Characters**: Convert to DOS-compatible 8.3 filenames, warn user
5. **Empty Drives**: Disable browse/download buttons when no disk inserted
6. **Long Filenames**: Truncate to 8.3 format, show warning
7. **Concurrent Operations**: Disable buttons during async operations

## Future Enhancements (Post-MVP)

- Drag-and-drop file upload with directory preservation
- Bulk file operations (multi-select, delete multiple)
- Disk image format conversion (IMG ↔ VHD ↔ ISO)
- Hex editor for direct sector viewing/editing
- Disk defragmentation visualization
- Real-time disk usage display (free/used space)
- Browser filesystem API integration for persistence
- ZIP file import/export (entire disk as ZIP)

## Notes

- All disk operations work in-memory (MemoryDiskBackend)
- No persistence between page reloads unless user downloads
- FAT12 maximum volume size: ~32MB
- FAT16 recommended for hard drives >32MB
- DOS 8.3 filename format enforced (FILENAME.EXT)
- Case-insensitive filesystem (stored uppercase)

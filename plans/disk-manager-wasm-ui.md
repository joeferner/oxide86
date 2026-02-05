# Plan: WASM Disk Management Features

## Overview
Add disk editing and file management capabilities to the WASM interface, enabling users to download disk images, browse directory structures, and upload/download files. Users must provide pre-formatted disk images (created externally).

## Requirements
1. Download floppy and hard drive images to browser
2. Interface (modal dialog) to browse disk directory structure
3. Download individual files from disk to browser
4. Upload files/directories from browser into disk

## Key Design Decisions

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
React/TypeScript UI Layer (DiskManager.tsx)
    ↓ (WASM bindings via wasm-bindgen)
Rust WASM Methods (wasm/src/lib.rs)
    ↓ (uses existing)
DriveManager + fatfs (core/src/drive_manager.rs)
    ↓
MemoryDiskBackend (core/src/disk.rs)
```

## Implementation Phases

### Phase 1: Core WASM Methods

**File:** `wasm/src/lib.rs`

Add methods to `Emu86Computer` struct:

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

**Note:** React and Mantine dependencies are already installed in `wasm/www/package.json`:
- `react` + `react-dom`
- `@mantine/core` + `@mantine/hooks` (provides Modal, Tabs, Table, Button, FileButton, etc.)

### Phase 2: React UI Component

**New file:** `wasm/www/src/components/DiskManager.tsx`

Create disk management modal component using Mantine UI:

**TypeScript Interfaces:**
```typescript
interface FileEntry {
  name: string;
  path: string;
  size: number;
  isDirectory: boolean;
  date: string;
  time: string;
  attributes: number;
}

interface DiskManagerProps {
  computer: Emu86Computer | null;
  opened: boolean;
  onClose: () => void;
  onStatusUpdate: (message: string) => void;
}
```

**Component Implementation:**
```typescript
export function DiskManager({ computer, opened, onClose, onStatusUpdate }: DiskManagerProps) {
  const [activeTab, setActiveTab] = useState<'browse' | 'download'>('browse');
  const [currentDrive, setCurrentDrive] = useState<number>(0);
  const [currentPath, setCurrentPath] = useState<string>('/');
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(false);

  // Disk operations
  const downloadDisk = async (driveType: 'floppy' | 'hdd', driveNumber: number)
  const browseDisk = async (drive: number, path: string)

  // File operations
  const downloadFile = async (drive: number, path: string)
  const uploadFile = async (drive: number, file: File, targetPath: string)
  const deleteFile = async (drive: number, path: string)
  const createDirectory = async (drive: number, path: string)
}
```

**Component Structure:**
```tsx
<Modal opened={opened} onClose={() => setOpened(false)} size="xl" title="Disk Manager">
  <Tabs value={activeTab} onChange={setActiveTab}>
    <Tabs.List>
      <Tabs.Tab value="browse">Browse Files</Tabs.Tab>
      <Tabs.Tab value="download">Download Disk</Tabs.Tab>
    </Tabs.List>

    <Tabs.Panel value="browse">
      <FileList />
    </Tabs.Panel>

    <Tabs.Panel value="download">
      <DownloadDiskForm />
    </Tabs.Panel>
  </Tabs>
</Modal>
```

**Key Functions:**

`downloadDisk()`: Downloads disk image as blob
```typescript
const downloadDisk = async (driveType: 'floppy' | 'hdd', driveNumber: number) => {
  try {
    const data = driveType === 'floppy'
      ? await computer?.get_floppy_data(driveNumber)
      : await computer?.get_hard_drive_data(driveNumber);

    if (!data) throw new Error('No data returned');

    const blob = new Blob([data], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `drive_${driveType}_${driveNumber}.img`;
    a.click();
    URL.revokeObjectURL(url);
    onStatusUpdate(`Downloaded ${driveType} ${driveNumber}`);
  } catch (e) {
    onStatusUpdate(`Error: ${e}`);
  }
}
```

`uploadFile()`: Uploads file with directory creation
```typescript
const uploadFile = async (drive: number, file: File, targetPath: string) => {
  try {
    const arrayBuffer = await file.arrayBuffer();
    const data = new Uint8Array(arrayBuffer);
    await computer?.write_file_to_disk(drive, targetPath, data);
    onStatusUpdate(`Uploaded ${file.name} to ${targetPath}`);
    await browseDisk(drive, currentPath); // Refresh listing
  } catch (e) {
    onStatusUpdate(`Error uploading: ${e}`);
  }
}
```

**FileList Component:**
```tsx
<Stack>
  <Group justify="space-between">
    <Breadcrumbs>{pathParts.map(/* ... */)}</Breadcrumbs>
    <Group>
      <FileButton onChange={(file) => file && uploadFile(currentDrive, file, currentPath)}>
        {(props) => <Button {...props}>Upload File</Button>}
      </FileButton>
      <Button onClick={() => refreshListing()}>Refresh</Button>
    </Group>
  </Group>

  <Table>
    <Table.Thead>
      <Table.Tr>
        <Table.Th>Name</Table.Th>
        <Table.Th>Size</Table.Th>
        <Table.Th>Date</Table.Th>
        <Table.Th>Actions</Table.Th>
      </Table.Tr>
    </Table.Thead>
    <Table.Tbody>
      {files.map((file) => (
        <Table.Tr key={file.name}>
          <Table.Td>
            {file.isDirectory ? '📁' : '📄'} {file.name}
          </Table.Td>
          <Table.Td>{file.isDirectory ? '-' : formatSize(file.size)}</Table.Td>
          <Table.Td>{file.date}</Table.Td>
          <Table.Td>
            <Group gap="xs">
              {!file.isDirectory && (
                <Button size="compact-xs" onClick={() => downloadFile(currentDrive, file.path)}>
                  Download
                </Button>
              )}
              <Button size="compact-xs" color="red" onClick={() => deleteFile(currentDrive, file.path)}>
                Delete
              </Button>
            </Group>
          </Table.Td>
        </Table.Tr>
      ))}
    </Table.Tbody>
  </Table>
</Stack>
```

### Phase 3: React Integration

**File:** `wasm/www/src/App.tsx`

**Modifications:**

1. Import DiskManager component
```typescript
import { DiskManager } from './components/DiskManager'
```

2. Add state for DiskManager modal
```typescript
const [diskManagerOpened, setDiskManagerOpened] = useState(false)
```

3. Add DiskManager button to controls section
```typescript
<Group gap="xs" mt="xs">
  <Button
    onClick={() => setDiskManagerOpened(true)}
    variant="default"
    fullWidth
  >
    Disk Manager
  </Button>
</Group>
```

4. Add DiskManager component (renders when opened)
```typescript
<DiskManager
  computer={computer}
  opened={diskManagerOpened}
  onClose={() => setDiskManagerOpened(false)}
  onStatusUpdate={handleStatusUpdate}
/>
```

**Full Integration Example:**
```typescript
function App() {
  // ... existing state ...
  const [diskManagerOpened, setDiskManagerOpened] = useState(false)

  return (
    <Container size="xl" p="md">
      {/* ... existing UI ... */}

      <Paper shadow="sm" p="md" style={{ flex: 1, minWidth: 300 }} withBorder>
        <Stack gap="xs">
          <DriveControl
            computer={computer}
            onStatusUpdate={handleStatusUpdate}
          />

          {/* Add Disk Manager button */}
          <Button
            onClick={() => setDiskManagerOpened(true)}
            variant="default"
            disabled={!computer}
          >
            Disk Manager
          </Button>

          {/* ... existing controls ... */}
        </Stack>
      </Paper>

      {/* Disk Manager Modal */}
      <DiskManager
        computer={computer}
        opened={diskManagerOpened}
        onClose={() => setDiskManagerOpened(false)}
        onStatusUpdate={handleStatusUpdate}
      />
    </Container>
  )
}
```

**Styling:**
- Mantine components handle all styling automatically
- Use Mantine theme for consistent look and feel
- No custom CSS needed for modal/dialog behavior
- Use SCSS modules only for custom component-specific styles if needed

## Critical Files

### Files to Modify
1. **`wasm/src/lib.rs`** - Add all WASM methods to Emu86Computer
2. **`wasm/Cargo.toml`** - Add serde dependencies
3. **`wasm/www/src/App.tsx`** - Add DiskManager button and modal state
4. **`core/src/drive_manager.rs`** - Reference only (may need minor visibility adjustments)

### Files to Create
1. **`wasm/www/src/components/DiskManager.tsx`** - Complete React component with modal UI
2. **`wasm/www/src/components/DiskManager.module.scss`** - Component-specific styles (if needed)

## Implementation Order

1. **Day 1-2: WASM Methods (Part 1)**
   - Implement get_*_data methods
   - Test with browser console commands

2. **Day 3-4: WASM Methods (Part 2)**
   - Implement list_directory method
   - Implement read_file_from_disk method
   - Implement write_file_to_disk method
   - Test each method individually with TypeScript

3. **Day 5-6: React Component Structure**
   - Create DiskManager.tsx component skeleton
   - Define TypeScript interfaces for props and file entries
   - Implement modal using Mantine's Modal component
   - Create tabbed interface using Mantine's Tabs component
   - Test UI rendering without WASM integration

4. **Day 7-8: React Component Logic**
   - Implement disk operations (download, browse)
   - Implement file operations (upload, download, delete)
   - Add state management with useState hooks
   - Connect component to WASM methods
   - Add error handling with try-catch

5. **Day 9: Integration & Polish**
   - Integrate DiskManager into App.tsx
   - Test complete workflows end-to-end
   - Add loading states and progress indicators
   - Add confirmation dialogs for destructive operations (using Mantine modals)
   - Final testing of all features

## Verification Steps

### Manual Testing Checklist

1. **Disk Download**
   - [ ] Download floppy A with data
   - [ ] Re-upload same image, verify data persists
   - [ ] Download hard drive C
   - [ ] Verify downloaded .img file size matches geometry

2. **Directory Browsing**
   - [ ] Browse empty disk (shows no entries)
   - [ ] Browse disk with files (shows all files)
   - [ ] Browse nested directories (shows correct path)
   - [ ] Verify directory vs file icons

3. **File Download**
   - [ ] Download text file, verify contents
   - [ ] Download binary file, verify byte-for-byte
   - [ ] Download file from subdirectory
   - [ ] Download large file (>100KB)

4. **File Upload**
   - [ ] Upload text file to root
   - [ ] Upload file to subdirectory (creates dirs)
   - [ ] Upload multiple files
   - [ ] Upload large file (>1MB), verify no corruption
   - [ ] Boot emulator and verify files are accessible

5. **Error Handling**
   - [ ] Try to download from empty drive (shows error)
   - [ ] Upload to non-existent drive (shows error)
   - [ ] Upload to full disk (shows error)
   - [ ] Invalid filename characters (sanitized or rejected)

### Integration Testing
```bash
# Build WASM
cd wasm
./scripts/build.sh

# Start Vite dev server (from wasm/www directory)
cd www
npm run dev

# Open browser to http://localhost:5173 (or URL shown by Vite)
# Run through manual testing checklist above

# Production build test
npm run build
npm run preview
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
- Users must provide pre-formatted disk images (use native tools like mformat, mkfs.msdos, or other disk image creation tools)
- Supported formats: FAT12 (floppy), FAT16 (hard drive)
- FAT12 maximum volume size: ~32MB
- FAT16 recommended for hard drives >32MB
- DOS 8.3 filename format enforced (FILENAME.EXT)
- Case-insensitive filesystem (stored uppercase)

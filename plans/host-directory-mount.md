# Host Directory Mounting - Implementation Plan

## Context

Users need to mount host filesystem directories as DOS drives to easily transfer files between the host and the emulated DOS environment. This eliminates the need to create and manage disk images for simple file access scenarios.

**User Requirements:**
- Read-write access (changes in DOS sync back to host)
- Multiple directories can be mounted simultaneously
- Drive letters specified via CLI argument (e.g., `--mount-dir /host/path:E:`)
- Direct mapping approach where possible

**Architectural Constraint:**
The DriveManager uses the `fatfs` crate for all file operations via `fatfs::FileSystem::new(adapter, ...)`. This means any DiskController implementation MUST provide valid FAT12/16/32 filesystem sectors. We cannot bypass this without major refactoring.

**Solution:**
Create an in-memory FAT16 filesystem populated from the host directory, implement bi-directional synchronization between the FAT image and host files, and provide a seamless user experience through CLI arguments.

---

## Implementation Approach

### Architecture Overview

```
Host Directory              HostDirectoryDisk              DOS Programs
(/home/user/dos/)          (DiskController impl)          (INT 21h file ops)
     │                              │                            │
     │ 1. Initial scan              │                            │
     ├─────────────────────>        │                            │
     │                              │                            │
     │ 2. Generate FAT16 image      │                            │
     │    (in-memory)               │                            │
     │                              │                            │
     │                              │ 3. Read/write sectors      │
     │                              │<─────────────────────────> │
     │                              │   (via fatfs)              │
     │                              │                            │
     │ 4. Sync dirty files          │                            │
     │<──────────────────────       │                            │
     │    (on close/periodic)       │                            │
```

### Core Strategy

1. **Scan host directory** at mount time to inventory files
2. **Generate FAT16 filesystem** in memory populated with host files
3. **Track writes** via DiskController::write_sector_lba()
4. **Sync changes** back to host on file close and shutdown

---

## Critical Files

### New Files

1. **core/src/host_directory_disk.rs**
   - HostDirectoryDisk struct implementing DiskController
   - Directory scanning and file mapping
   - Write tracking and sync-back logic

2. **core/src/fat16_format.rs**
   - FAT16 boot sector generation
   - FAT table initialization
   - Root directory setup

### Modified Files

1. **core/src/lib.rs**
   - Export new modules: `pub mod host_directory_disk;`, `pub mod fat16_format;`

2. **native-cli/src/main.rs**
   - Add `mount_dirs: Vec<String>` field to Cli struct
   - Parse mount arguments (format: `/path:E:` or `/path:0x82`)
   - Load HostDirectoryDisk instances for each mount
   - Sync all mounts on shutdown

3. **native-gui/src/main.rs**
   - Same changes as native-cli

4. **core/Cargo.toml**
   - No new dependencies needed (use existing: fatfs, anyhow, walkdir)

---

## Detailed Implementation

### 1. HostDirectoryDisk Structure

```rust
// core/src/host_directory_disk.rs

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use anyhow::Result;

pub struct HostDirectoryDisk {
    host_path: PathBuf,
    fat_image: BackedDisk<MemoryDiskBackend>,
    dirty_sectors: HashSet<usize>,
    file_map: HashMap<String, PathBuf>,  // DOS path (uppercase) -> host path
    read_only: bool,
}

impl HostDirectoryDisk {
    pub fn new(host_path: PathBuf, read_only: bool) -> Result<Self> {
        // 1. Scan directory
        let files = scan_directory(&host_path)?;

        // 2. Calculate required size
        let total_bytes: u64 = files.iter().map(|f| f.size).sum();
        let sectors = calculate_fat16_sectors(total_bytes)?;

        // 3. Create and format blank FAT16 image
        let backend = MemoryDiskBackend::new_blank(sectors * SECTOR_SIZE);
        let mut disk = BackedDisk::new(backend)?;
        format_fat16(&mut disk, sectors)?;

        // 4. Populate with files
        let file_map = populate_fat_image(&mut disk, &host_path, files)?;

        Ok(Self {
            host_path,
            fat_image: disk,
            dirty_sectors: HashSet::new(),
            file_map,
            read_only,
        })
    }

    pub fn sync_to_host(&mut self) -> Result<()> {
        // Extract changed files from FAT image and write to host
        // Only sync files tracked in dirty_sectors
    }
}

impl DiskController for HostDirectoryDisk {
    fn read_sector_lba(&self, lba: usize) -> Result<[u8; SECTOR_SIZE]> {
        self.fat_image.read_sector_lba(lba)
    }

    fn write_sector_lba(&mut self, lba: usize, data: &[u8; SECTOR_SIZE]) -> Result<()> {
        self.fat_image.write_sector_lba(lba, data)?;
        if !self.read_only {
            self.dirty_sectors.insert(lba);
        }
        Ok(())
    }

    fn read_sector_chs(&self, c: u16, h: u16, s: u16) -> Result<[u8; SECTOR_SIZE]> {
        let lba = self.geometry().chs_to_lba(c, h, s);
        self.read_sector_lba(lba)
    }

    fn write_sector_chs(&mut self, c: u16, h: u16, s: u16, data: &[u8; SECTOR_SIZE]) -> Result<()> {
        let lba = self.geometry().chs_to_lba(c, h, s);
        self.write_sector_lba(lba, data)
    }

    fn geometry(&self) -> &DiskGeometry {
        self.fat_image.geometry()
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }
}
```

### 2. FAT16 Formatting

```rust
// core/src/fat16_format.rs

pub fn format_fat16(disk: &mut dyn DiskController, total_sectors: usize) -> Result<()> {
    // FAT16 parameters
    let sectors_per_cluster = 8;  // 4KB clusters
    let reserved_sectors = 1;     // Boot sector only
    let num_fats = 2;              // Standard: 2 copies
    let root_entries = 512;        // Standard for FAT16
    let root_sectors = (root_entries * 32 + SECTOR_SIZE - 1) / SECTOR_SIZE;

    // Calculate FAT size
    let clusters = (total_sectors - reserved_sectors - root_sectors) / sectors_per_cluster;
    let fat_entries = clusters + 2;  // +2 for reserved entries
    let fat_size = (fat_entries * 2 + SECTOR_SIZE - 1) / SECTOR_SIZE;

    // Write boot sector
    let mut boot_sector = [0u8; SECTOR_SIZE];
    // Jump instruction
    boot_sector[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
    // OEM ID
    boot_sector[3..11].copy_from_slice(b"EMU86   ");
    // Bytes per sector
    boot_sector[11..13].copy_from_slice(&512u16.to_le_bytes());
    // Sectors per cluster
    boot_sector[13] = sectors_per_cluster as u8;
    // Reserved sectors
    boot_sector[14..16].copy_from_slice(&(reserved_sectors as u16).to_le_bytes());
    // Number of FATs
    boot_sector[16] = num_fats as u8;
    // Root entries
    boot_sector[17..19].copy_from_slice(&(root_entries as u16).to_le_bytes());
    // Total sectors (16-bit if < 65536, else 0)
    if total_sectors < 65536 {
        boot_sector[19..21].copy_from_slice(&(total_sectors as u16).to_le_bytes());
    }
    // Media descriptor (0xF8 = hard disk)
    boot_sector[21] = 0xF8;
    // Sectors per FAT
    boot_sector[22..24].copy_from_slice(&(fat_size as u16).to_le_bytes());
    // Total sectors (32-bit)
    if total_sectors >= 65536 {
        boot_sector[32..36].copy_from_slice(&(total_sectors as u32).to_le_bytes());
    }
    // Boot signature
    boot_sector[510..512].copy_from_slice(&[0x55, 0xAA]);

    disk.write_sector_lba(0, &boot_sector)?;

    // Write FAT tables (both copies)
    let mut fat_sector = [0u8; SECTOR_SIZE];
    // First FAT entry: media descriptor
    fat_sector[0] = 0xF8;
    fat_sector[1] = 0xFF;
    // Second FAT entry: end of chain marker
    fat_sector[2] = 0xFF;
    fat_sector[3] = 0xFF;

    for copy in 0..num_fats {
        let fat_start = reserved_sectors + (copy * fat_size);
        disk.write_sector_lba(fat_start, &fat_sector)?;
        // Zero remaining FAT sectors
        let zero = [0u8; SECTOR_SIZE];
        for i in 1..fat_size {
            disk.write_sector_lba(fat_start + i, &zero)?;
        }
    }

    // Zero root directory
    let root_start = reserved_sectors + (num_fats * fat_size);
    let zero = [0u8; SECTOR_SIZE];
    for i in 0..root_sectors {
        disk.write_sector_lba(root_start + i, &zero)?;
    }

    Ok(())
}

fn calculate_fat16_sectors(data_bytes: u64) -> Result<usize> {
    // Add 20% overhead for FAT structures
    let total_bytes = (data_bytes as f64 * 1.2) as u64;
    // Round up to power of 2, minimum 16MB
    let sectors = (total_bytes / SECTOR_SIZE as u64).max(32768);
    let sectors = sectors.next_power_of_two() as usize;

    // FAT16 max: 65525 clusters * 8 sectors/cluster
    if sectors > 524200 {
        anyhow::bail!("Directory too large for FAT16 (max ~256MB)");
    }

    Ok(sectors)
}
```

### 3. Directory Scanning and Population

```rust
// core/src/host_directory_disk.rs (continued)

struct FileEntry {
    dos_name: String,      // 8.3 format, uppercase
    host_path: PathBuf,
    size: u64,
    is_dir: bool,
}

fn scan_directory(path: &Path) -> Result<Vec<FileEntry>> {
    use walkdir::WalkDir;

    let mut entries = Vec::new();
    let base_path = path;

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        let host_path = entry.path();

        // Get relative path from base
        let rel_path = host_path.strip_prefix(base_path)?;
        if rel_path.as_os_str().is_empty() {
            continue;  // Skip root
        }

        // Convert to DOS 8.3 format
        let dos_name = to_dos_name(rel_path)?;

        entries.push(FileEntry {
            dos_name,
            host_path: host_path.to_path_buf(),
            size: entry.metadata()?.len(),
            is_dir: entry.file_type().is_dir(),
        });
    }

    Ok(entries)
}

fn to_dos_name(path: &Path) -> Result<String> {
    // Convert Unix path to DOS path with 8.3 names
    let mut result = String::new();

    for component in path.components() {
        if !result.is_empty() {
            result.push('/');
        }

        let name = component.as_os_str().to_string_lossy();

        // Convert to 8.3: "long_filename.txt" -> "LONG_FIL.TXT"
        let (base, ext) = if let Some(dot_pos) = name.rfind('.') {
            (&name[..dot_pos], Some(&name[dot_pos + 1..]))
        } else {
            (name.as_ref(), None)
        };

        let base = &base[..base.len().min(8)].to_uppercase();
        let dos_name = if let Some(ext) = ext {
            let ext = &ext[..ext.len().min(3)].to_uppercase();
            format!("{}.{}", base, ext)
        } else {
            base.to_string()
        };

        result.push_str(&dos_name);
    }

    Ok(result)
}

fn populate_fat_image(
    disk: &mut BackedDisk<MemoryDiskBackend>,
    base_path: &Path,
    files: Vec<FileEntry>,
) -> Result<HashMap<String, PathBuf>> {
    let mut file_map = HashMap::new();

    // Mount FAT image with fatfs
    let mut adapter = DiskAdapter::new(Box::new(disk.clone()));
    adapter.reset_position();
    let fs = fatfs::FileSystem::new(&mut adapter, fatfs::FsOptions::new())?;
    let root = fs.root_dir();

    // Create directories first
    for entry in files.iter().filter(|e| e.is_dir) {
        root.create_dir(&entry.dos_name)?;
    }

    // Create files
    for entry in files.iter().filter(|e| !e.is_dir) {
        let host_data = std::fs::read(&entry.host_path)?;
        let mut file = root.create_file(&entry.dos_name)?;
        file.write_all(&host_data)?;
        file_map.insert(entry.dos_name.clone(), entry.host_path.clone());
    }

    drop(fs);
    Ok(file_map)
}
```

### 4. CLI Integration

```rust
// native-cli/src/main.rs

// Add to Cli struct:
/// Mount host directory as DOS drive (format: /path:E: or /path:0x82)
#[arg(long = "mount-dir", action = clap::ArgAction::Append)]
mount_dirs: Vec<String>,

// After hard disk loading:
// Track mounted drives for sync on shutdown
let mut mounted_drives = Vec::new();

for mount_spec in cli.mount_dirs.iter() {
    let (host_path, drive_letter) = parse_mount_arg(mount_spec)?;

    if !host_path.exists() {
        anyhow::bail!("Mount path does not exist: {}", host_path.display());
    }
    if !host_path.is_dir() {
        anyhow::bail!("Mount path is not a directory: {}", host_path.display());
    }

    log::info!("Mounting {} as drive {}...", host_path.display(), drive_letter);

    let host_disk = HostDirectoryDisk::new(host_path, false)?;
    let drive_num = computer.bios_mut().add_hard_drive(Box::new(host_disk));

    if drive_num.to_letter() != drive_letter {
        log::warn!(
            "Requested drive {} but assigned {} (drives must be sequential)",
            drive_letter, drive_num.to_letter()
        );
    }

    mounted_drives.push(drive_num);
    log::info!("Mounted as drive {}", drive_num.to_letter());
}

computer.update_bda_hard_drive_count();

// ... emulation loop ...

// Before exit:
log::info!("Syncing mounted directories...");
for drive_num in mounted_drives {
    // Access drive and sync if it's a HostDirectoryDisk
    // (need to add getter method to Bios/DriveManager)
    computer.bios_mut().sync_mounted_drive(drive_num)?;
}

fn parse_mount_arg(arg: &str) -> Result<(PathBuf, char)> {
    let parts: Vec<&str> = arg.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid format. Use: /path:E: or /path:0x82");
    }

    let path = PathBuf::from(parts[1]);
    let drive_spec = parts[0].trim_end_matches(':');

    let drive_letter = if drive_spec.starts_with("0x") {
        // Hex format: 0x82 -> D:
        let num = u8::from_str_radix(&drive_spec[2..], 16)?;
        DriveNumber::from_standard(num).to_letter()
    } else if drive_spec.len() == 1 {
        // Letter format: E
        drive_spec.chars().next().unwrap().to_ascii_uppercase()
    } else {
        anyhow::bail!("Invalid drive specifier: {}", drive_spec);
    };

    if drive_letter < 'C' {
        anyhow::bail!("Mounted directories must use hard drive letters (C: or higher)");
    }

    Ok((path, drive_letter))
}
```

### 5. Sync Support in DriveManager

```rust
// core/src/drive_manager.rs

impl DriveManager {
    pub fn sync_drive(&mut self, drive: DriveNumber) -> Result<(), String> {
        let drive_state = self.get_drive_mut(drive)
            .ok_or_else(|| format!("Drive {} not found", drive))?;

        // Check if this is a HostDirectoryDisk by attempting downcast
        // (requires adding as_any/as_any_mut to DiskController trait)
        if let Some(adapter) = &mut drive_state.adapter {
            // Extract the disk from adapter and sync
            // This is complex - may need refactoring
            // Alternative: Add sync() method to DiskController trait
        }

        Ok(())
    }
}

// Alternative approach: Add sync() to DiskController trait
pub trait DiskController {
    // ... existing methods ...

    /// Sync changes to backing storage (no-op for most implementations)
    fn sync(&mut self) -> Result<()> {
        Ok(())  // Default: no-op
    }
}

impl DiskController for HostDirectoryDisk {
    fn sync(&mut self) -> Result<()> {
        self.sync_to_host()
    }
}
```

---

## Implementation Phases

### Phase 1: FAT16 Foundation (2-3 hours)
1. Create `core/src/fat16_format.rs`
2. Implement boot sector generation
3. Implement FAT table initialization
4. Write unit tests for FAT structures
5. Verify fatfs can mount generated filesystem

### Phase 2: Directory Scanning (1-2 hours)
1. Implement `scan_directory()` with walkdir
2. Implement `to_dos_name()` with 8.3 conversion
3. Handle filename collisions
4. Test with various directory structures

### Phase 3: Image Population (2-3 hours)
1. Implement `populate_fat_image()` using fatfs
2. Create directories and files in FAT image
3. Build file mapping (DOS name -> host path)
4. Test reading files through fatfs

### Phase 4: DiskController Implementation (1 hour)
1. Implement DiskController trait for HostDirectoryDisk
2. Add write tracking (dirty_sectors)
3. Test read operations through emulator

### Phase 5: Write-Back Sync (2-3 hours)
1. Implement `sync_to_host()` method
2. Extract files from FAT image
3. Write changed files to host
4. Test bidirectional sync

### Phase 6: CLI Integration (1-2 hours)
1. Add --mount-dir argument parsing
2. Integrate with drive loading
3. Add sync on shutdown
4. Test end-to-end

### Phase 7: Polish & Edge Cases (2-3 hours)
1. Handle long filenames gracefully
2. Add progress indicators
3. Improve error messages
4. Add size limits and warnings
5. Update documentation

**Total Estimated Time: 11-17 hours**

---

## Testing Strategy

### Unit Tests
- FAT16 structures are valid (boot sector, FAT tables)
- 8.3 filename conversion handles edge cases
- Directory scanning finds all files

### Integration Tests
1. Mount test directory with known files
2. Verify files appear in DOS (via INT 21h)
3. Create new file in DOS, check host after sync
4. Modify file in DOS, verify host changes
5. Test with nested directories
6. Test with edge cases (empty files, large files, special chars)

### Manual Testing
```bash
# Create test directory
mkdir /tmp/dos_test
echo "Hello from host" > /tmp/dos_test/TEST.TXT

# Mount and boot
cargo run -p emu86-native-cli -- --boot --floppy-a freedos.img --mount-dir /tmp/dos_test:D:

# In DOS:
# D:
# DIR
# TYPE TEST.TXT
# ECHO Hello from DOS > NEW.TXT
# EXIT

# Check host
cat /tmp/dos_test/NEW.TXT  # Should contain "Hello from DOS"
```

---

## Limitations & Future Enhancements

### Known Limitations
1. **FAT16 size limit**: Max ~256MB per mounted directory
2. **8.3 filenames**: Long names truncated
3. **No hot-reload**: Host changes not visible until remount
4. **Sequential drive letters**: Cannot skip drive letters
5. **Native only**: WASM cannot access host filesystem

### Future Enhancements
1. **inotify/FSEvents**: Watch host for changes, live updates
2. **FAT32 support**: Larger directories (2GB+)
3. **LFN support**: Long filename extensions
4. **Read-only mode**: `--mount-dir-ro` flag
5. **Floppy mounting**: Allow mounting as A: or B:
6. **WASM support**: File System Access API integration

---

## Verification Steps

After implementation:

1. **Compile**: `cargo build --release`
2. **Mount directory**: `cargo run -p emu86-native-cli -- --boot --floppy-a dos.img --mount-dir ~/dos_files:D:`
3. **Boot DOS**: Verify system boots
4. **Check drive**: `D:`, `DIR` - see files from host
5. **Read file**: `TYPE EXISTING.TXT` - verify content
6. **Write file**: `ECHO test > NEW.TXT` - create new file
7. **Exit & check**: Exit emulator, verify `~/dos_files/NEW.TXT` exists on host
8. **Multiple mounts**: Test with multiple `--mount-dir` arguments

---

## Dependencies

All required crates already in use:
- `fatfs` - FAT filesystem access (already used)
- `walkdir` - Directory traversal (add if not present)
- `anyhow` - Error handling (already used)
- Standard library for file I/O

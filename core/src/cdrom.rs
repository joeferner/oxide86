//! CD-ROM ISO 9660 filesystem support.
//!
//! Provides read-only access to ISO 9660 disc images. Used by the emulator
//! to expose CD-ROM drives to DOS programs through INT 13h (sector reads)
//! and INT 21h (file operations via DriveManager).

/// Sector size for CD-ROM (2048 bytes, vs 512 for floppy/HDD)
pub const CD_SECTOR_SIZE: usize = 2048;

/// LBA of the Primary Volume Descriptor
const PVD_SECTOR: usize = 16;

/// ISO 9660 directory entry, normalized for DOS compatibility
#[derive(Debug, Clone)]
pub struct IsoEntry {
    /// Filename: uppercase, ";1" stripped
    pub name: String,
    /// Starting LBA of the file/directory extent
    pub extent_lba: u32,
    /// Size in bytes
    pub data_length: u32,
    /// True if this is a directory
    pub is_dir: bool,
    /// ISO 9660 7-byte recording date: year-since-1900, month, day, hour, min, sec, gmt_offset
    pub recording_date: [u8; 7],
}

/// CD-ROM disc image backed by raw ISO 9660 data.
///
/// Platform-agnostic (works in both native and WASM) — holds the entire
/// image in memory and parses the ISO 9660 filesystem on demand.
pub struct CdRomImage {
    data: Vec<u8>,
    volume_label: String,
    root_extent_lba: u32,
    root_data_length: u32,
}

impl CdRomImage {
    /// Create a new CD-ROM image from raw ISO data.
    ///
    /// Validates the Primary Volume Descriptor at sector 16.
    /// Returns an error string if the image is invalid or too small.
    pub fn new(data: Vec<u8>) -> Result<Self, String> {
        // Minimum size: up to and including PVD sector
        if data.len() < (PVD_SECTOR + 1) * CD_SECTOR_SIZE {
            return Err(format!(
                "ISO image too small: {} bytes (need at least {})",
                data.len(),
                (PVD_SECTOR + 1) * CD_SECTOR_SIZE
            ));
        }

        let pvd_offset = PVD_SECTOR * CD_SECTOR_SIZE;
        let pvd = &data[pvd_offset..pvd_offset + CD_SECTOR_SIZE];

        // Check Primary Volume Descriptor type (byte 0 = 1) and identifier "CD001"
        if pvd[0] != 1 {
            return Err(format!(
                "Expected Primary Volume Descriptor (type 1) at sector {}, got type {}",
                PVD_SECTOR, pvd[0]
            ));
        }
        if &pvd[1..6] != b"CD001" {
            return Err("Invalid ISO 9660 signature (expected 'CD001')".to_string());
        }

        // Volume identifier: bytes 40-71 (32 bytes), padded with spaces
        let volume_label = pvd[40..72]
            .iter()
            .rev()
            .skip_while(|&&b| b == b' ')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|&b| b as char)
            .collect::<String>();

        // Root directory record is embedded in PVD at offset 156 (34 bytes)
        let root_rec = &pvd[156..190];
        // Extent location: bytes [2..6] LE
        let root_extent_lba =
            u32::from_le_bytes([root_rec[2], root_rec[3], root_rec[4], root_rec[5]]);
        // Data length: bytes [10..14] LE
        let root_data_length =
            u32::from_le_bytes([root_rec[10], root_rec[11], root_rec[12], root_rec[13]]);

        log::info!(
            "ISO 9660: label='{}', root at LBA {} ({} bytes)",
            volume_label,
            root_extent_lba,
            root_data_length
        );

        Ok(Self {
            data,
            volume_label,
            root_extent_lba,
            root_data_length,
        })
    }

    /// Read a 2048-byte sector at the given LBA.
    pub fn read_sector(&self, lba: u32) -> Result<[u8; CD_SECTOR_SIZE], String> {
        let offset = lba as usize * CD_SECTOR_SIZE;
        if offset + CD_SECTOR_SIZE > self.data.len() {
            return Err(format!(
                "CD-ROM LBA {} out of range (image size {} bytes)",
                lba,
                self.data.len()
            ));
        }
        let mut sector = [0u8; CD_SECTOR_SIZE];
        sector.copy_from_slice(&self.data[offset..offset + CD_SECTOR_SIZE]);
        Ok(sector)
    }

    /// Return the volume label (disc name), trimmed of trailing spaces.
    pub fn get_volume_label(&self) -> &str {
        &self.volume_label
    }

    /// Return the total size of the image in bytes.
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// List the contents of a directory on the disc.
    ///
    /// Path components are matched case-insensitively.
    /// The root directory is `"/"` or `""`.
    pub fn list_directory(&self, path: &str) -> Result<Vec<IsoEntry>, String> {
        let (extent_lba, data_length) = self.resolve_directory(path)?;
        self.read_dir_entries(extent_lba, data_length)
    }

    /// Find a single entry (file or directory) by path.
    pub fn find_entry(&self, path: &str) -> Result<IsoEntry, String> {
        // Normalize and split
        let normalized = path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        if parts.is_empty() {
            // Asking for root itself — return synthetic entry
            return Ok(IsoEntry {
                name: String::new(),
                extent_lba: self.root_extent_lba,
                data_length: self.root_data_length,
                is_dir: true,
                recording_date: [0u8; 7],
            });
        }

        // Walk all components except the last to find the parent directory
        let mut current_lba = self.root_extent_lba;
        let mut current_len = self.root_data_length;

        for component in &parts[..parts.len() - 1] {
            let entries = self.read_dir_entries(current_lba, current_len)?;
            let found = entries
                .iter()
                .find(|e| e.is_dir && e.name.eq_ignore_ascii_case(component));
            match found {
                Some(e) => {
                    current_lba = e.extent_lba;
                    current_len = e.data_length;
                }
                None => return Err(format!("Directory not found: {}", component)),
            }
        }

        // Find the final component
        let target = parts.last().unwrap();
        let entries = self.read_dir_entries(current_lba, current_len)?;
        entries
            .into_iter()
            .find(|e| e.name.eq_ignore_ascii_case(target))
            .ok_or_else(|| format!("Entry not found: {}", target))
    }

    /// Read the full contents of a file on the disc.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        let entry = self.find_entry(path)?;
        if entry.is_dir {
            return Err(format!("'{}' is a directory, not a file", path));
        }

        let offset = entry.extent_lba as usize * CD_SECTOR_SIZE;
        let length = entry.data_length as usize;

        if offset + length > self.data.len() {
            return Err(format!(
                "File extent at LBA {} (size {}) exceeds image bounds",
                entry.extent_lba, length
            ));
        }

        Ok(self.data[offset..offset + length].to_vec())
    }

    /// Read a raw slice of the disc data for a given byte offset and length.
    ///
    /// Used by the WASM file browser and INT 13h raw sector access.
    /// Returns as many bytes as available up to `length`.
    pub fn read_raw(&self, offset: usize, length: usize) -> &[u8] {
        let end = (offset + length).min(self.data.len());
        if offset >= self.data.len() {
            &[]
        } else {
            &self.data[offset..end]
        }
    }

    // --- Private helpers ---

    /// Resolve a path to a directory's (extent_lba, data_length).
    fn resolve_directory(&self, path: &str) -> Result<(u32, u32), String> {
        let normalized = path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        let mut current_lba = self.root_extent_lba;
        let mut current_len = self.root_data_length;

        for component in parts {
            let entries = self.read_dir_entries(current_lba, current_len)?;
            let found = entries
                .iter()
                .find(|e| e.is_dir && e.name.eq_ignore_ascii_case(component));
            match found {
                Some(e) => {
                    current_lba = e.extent_lba;
                    current_len = e.data_length;
                }
                None => return Err(format!("Directory not found: {}", component)),
            }
        }

        Ok((current_lba, current_len))
    }

    /// Parse all directory records from a directory extent.
    fn read_dir_entries(&self, extent_lba: u32, data_length: u32) -> Result<Vec<IsoEntry>, String> {
        let mut entries = Vec::new();
        let mut bytes_remaining = data_length as usize;
        let mut lba = extent_lba as usize;

        while bytes_remaining > 0 {
            let sector = self.read_sector(lba as u32)?;
            lba += 1;
            let sector_bytes = bytes_remaining.min(CD_SECTOR_SIZE);
            bytes_remaining = bytes_remaining.saturating_sub(CD_SECTOR_SIZE);

            let mut pos = 0usize;
            while pos < sector_bytes {
                let record_len = sector[pos] as usize;

                if record_len == 0 {
                    // Zero record length signals padding to end of sector
                    break;
                }

                if pos + record_len > sector_bytes {
                    break; // Malformed record
                }

                if let Some(entry) = Self::parse_dir_record(&sector[pos..pos + record_len]) {
                    entries.push(entry);
                }

                pos += record_len;
            }
        }

        Ok(entries)
    }

    /// Parse a single ISO 9660 directory record from a byte slice.
    /// Returns None for dot/dotdot entries or malformed records.
    fn parse_dir_record(data: &[u8]) -> Option<IsoEntry> {
        let record_len = data[0] as usize;
        if record_len < 34 || data.len() < record_len {
            return None;
        }

        let id_len = data[32] as usize;
        if id_len == 0 || 33 + id_len > record_len {
            return None;
        }

        let identifier = &data[33..33 + id_len];

        // Skip dot (current dir) and dotdot (parent dir) entries
        if identifier == b"\x00" || identifier == b"\x01" {
            return None;
        }

        let flags = data[25];
        let is_dir = (flags & 0x02) != 0;

        // Extent LBA: bytes [2..6] LE
        let extent_lba = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        // Data length: bytes [10..14] LE
        let data_length = u32::from_le_bytes([data[10], data[11], data[12], data[13]]);

        // Recording date: bytes [18..25]
        let mut recording_date = [0u8; 7];
        recording_date.copy_from_slice(&data[18..25]);

        let name = Self::normalize_name(identifier, is_dir);

        Some(IsoEntry {
            name,
            extent_lba,
            data_length,
            is_dir,
            recording_date,
        })
    }

    /// Normalize an ISO 9660 file identifier to uppercase ASCII with version suffix stripped.
    fn normalize_name(raw: &[u8], is_dir: bool) -> String {
        let s = if is_dir {
            // Directories don't have version numbers
            String::from_utf8_lossy(raw).to_string()
        } else {
            // Strip ";1" (or any ";N") version suffix from file names
            let s = String::from_utf8_lossy(raw).to_string();
            if let Some(pos) = s.rfind(';') {
                s[..pos].to_string()
            } else {
                s
            }
        };
        s.to_uppercase()
    }

    /// Convert an ISO 9660 7-byte recording date to DOS packed date/time.
    ///
    /// ISO date: [year-since-1900, month, day, hour, min, sec, gmt_offset_quarters]
    /// DOS date: bits 15-9 = year-since-1980, bits 8-5 = month, bits 4-0 = day
    /// DOS time: bits 15-11 = hour, bits 10-5 = min, bits 4-0 = sec/2
    pub fn dos_date_from_iso(date: &[u8; 7]) -> (u16, u16) {
        let year_1900 = date[0] as u16;
        let month = date[1] as u16;
        let day = date[2] as u16;
        let hour = date[3] as u16;
        let minute = date[4] as u16;
        let second = date[5] as u16;

        // Clamp year to DOS range (1980-2107)
        let dos_year = if year_1900 >= 80 { year_1900 - 80 } else { 0 };
        let dos_month = month.max(1).min(12);
        let dos_day = day.max(1).min(31);

        let dos_date = (dos_year << 9) | (dos_month << 5) | dos_day;
        let dos_time = (hour << 11) | (minute << 5) | (second / 2);

        (dos_date, dos_time)
    }
}

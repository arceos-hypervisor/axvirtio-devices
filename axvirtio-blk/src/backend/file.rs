use alloc::string::String;

use super::traits::BlockBackend;
use crate::constants::*;
use axvirtio_common::{VirtioError, VirtioResult};

#[cfg(feature = "file-backend")]
use axfs::fops::{File, OpenOptions};

/// File-based block device backend
///
/// This backend stores data in a file on the host filesystem.
#[cfg(feature = "file-backend")]
pub struct FileBackend {
    /// File path
    file_path: String,
    /// Device capacity in sectors
    capacity: u64,
    /// Read-only flag
    read_only: bool,
}

/// Placeholder FileBackend for when file-backend feature is disabled
#[cfg(not(feature = "file-backend"))]
pub struct FileBackend;

#[cfg(feature = "file-backend")]
impl FileBackend {
    /// Create a new file backend
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to use as storage
    /// * `capacity_sectors` - Capacity in 512-byte sectors
    /// * `read_only` - Whether the device should be read-only
    pub fn new(file_path: String, capacity_sectors: u64, read_only: bool) -> VirtioResult<Self> {
        if file_path.is_empty() {
            return Err(VirtioError::InvalidConfig);
        }

        if capacity_sectors == 0 {
            return Err(VirtioError::InvalidConfig);
        }

        // Try to create/open the file to validate it exists or can be created
        let mut opts = OpenOptions::new();
        if read_only {
            opts.read(true);
        } else {
            opts.create(true);
            opts.write(true);
            opts.read(true);
        }
        let _file = File::open(&file_path, &opts).map_err(|_| VirtioError::BackendError)?;

        Ok(Self {
            file_path,
            capacity: capacity_sectors,
            read_only,
        })
    }

    /// Validate sector access bounds
    fn validate_access(&self, sector: u64, size: usize) -> VirtioResult<()> {
        let block_size = SECTOR_SIZE as usize;

        // Check alignment
        if size % block_size != 0 {
            return Err(VirtioError::InvalidBufferSize);
        }

        // Check bounds
        let sectors_needed = size / block_size;
        if sector + sectors_needed as u64 > self.capacity {
            return Err(VirtioError::InvalidSector);
        }

        Ok(())
    }
}

#[cfg(not(feature = "file-backend"))]
impl FileBackend {
    pub fn new(_file_path: String, _capacity_sectors: u64, _read_only: bool) -> VirtioResult<Self> {
        Err(VirtioError::BackendError)
    }
}

#[cfg(feature = "file-backend")]
impl BlockBackend for FileBackend {
    fn read(&self, sector: u64, buffer: &mut [u8]) -> VirtioResult<usize> {
        self.validate_access(sector, buffer.len())?;

        // Open file for reading
        let mut opts = OpenOptions::new();
        opts.read(true);
        let file = File::open(&self.file_path, &opts).map_err(|_| VirtioError::BackendError)?;

        // Calculate byte offset
        let offset = sector * SECTOR_SIZE_U64;

        // Read data from file at offset
        let bytes_read = file
            .read_at(offset, buffer)
            .map_err(|_| VirtioError::BackendError)?;

        Ok(bytes_read)
    }

    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        if self.read_only {
            return Err(VirtioError::BackendError);
        }

        self.validate_access(sector, buffer.len())?;

        // Open file for writing
        let mut opts = OpenOptions::new();
        opts.write(true);
        let file = File::open(&self.file_path, &opts).map_err(|_| VirtioError::BackendError)?;

        // Calculate byte offset
        let offset = sector * SECTOR_SIZE_U64;

        // Write data to file at offset
        let bytes_written = file
            .write_at(offset, buffer)
            .map_err(|_| VirtioError::BackendError)?;

        Ok(bytes_written)
    }

    fn flush(&self) -> VirtioResult<()> {
        // Open file and sync
        let mut opts = OpenOptions::new();
        opts.read(true);
        let file = File::open(&self.file_path, &opts).map_err(|_| VirtioError::BackendError)?;
        file.flush().map_err(|_| VirtioError::BackendError)?;
        Ok(())
    }
}

#[cfg(not(feature = "file-backend"))]
impl BlockBackend for FileBackend {
    fn read(&self, _sector: u64, _buffer: &mut [u8]) -> VirtioResult<usize> {
        Err(VirtioError::BackendError)
    }

    fn write(&self, _sector: u64, _buffer: &[u8]) -> VirtioResult<usize> {
        Err(VirtioError::BackendError)
    }

    fn flush(&self) -> VirtioResult<()> {
        Err(VirtioError::BackendError)
    }
}

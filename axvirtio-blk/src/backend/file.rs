use alloc::string::String;

use super::traits::BlockBackend;
use crate::constants::*;
use axvirtio_common::{VirtioError, VirtioResult};

#[cfg(feature = "file-backend")]
use std::fs::{File, OpenOptions};
#[cfg(feature = "file-backend")]
use std::io::{Seek, SeekFrom, Read, Write};
#[cfg(feature = "file-backend")]
use spin::Mutex;

/// File-based block device backend
///
/// This backend stores data in a file on the host filesystem.
/// Uses separate read and write file handles for better performance.
#[cfg(feature = "file-backend")]
pub struct FileBackend {
    /// Device capacity in sectors
    capacity: u64,
    /// Read-only flag
    read_only: bool,
    /// Dedicated file handle for read operations
    read_file: Mutex<Option<File>>,
    /// Dedicated file handle for write operations (None for read-only devices)
    write_file: Mutex<Option<File>>,
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

        // Create read file handle
        let mut read_opts = OpenOptions::new();
        read_opts.read(true);
        let read_file = Some(read_opts.open(&file_path).map_err(|_| VirtioError::BackendError)?);

        // Create write file handle if not read-only
        let write_file = if read_only {
            None
        } else {
            let mut write_opts = OpenOptions::new();
            write_opts.create(true).write(true).read(true);
            Some(write_opts.open(&file_path).map_err(|_| VirtioError::BackendError)?)
        };

        Ok(Self {
            capacity: capacity_sectors,
            read_only,
            read_file: Mutex::new(read_file),
            write_file: Mutex::new(write_file),
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

        // Calculate byte offset
        let offset = sector * SECTOR_SIZE_U64;

        // Use the dedicated read file handle
        let mut read_file_guard = self.read_file.lock();
        let read_file = read_file_guard.as_mut().ok_or(VirtioError::BackendError)?;

        // Seek to the offset and read data
        if let Err(_) = read_file.seek(SeekFrom::Start(offset)) {
            return Err(VirtioError::BackendError);
        }

        let bytes_read = read_file.read(buffer).map_err(|_| VirtioError::BackendError)?;

        Ok(bytes_read)
    }

    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        if self.read_only {
            return Err(VirtioError::BackendError);
        }

        self.validate_access(sector, buffer.len())?;

        // Calculate byte offset
        let offset = sector * SECTOR_SIZE_U64;

        // Use the dedicated write file handle
        let mut write_file_guard = self.write_file.lock();
        let write_file = write_file_guard.as_mut().ok_or(VirtioError::BackendError)?;

        // Seek to the offset and write data
        if let Err(_) = write_file.seek(SeekFrom::Start(offset)) {
            return Err(VirtioError::BackendError);
        }

        let bytes_written = write_file.write(buffer).map_err(|_| VirtioError::BackendError)?;

        Ok(bytes_written)
    }

    fn flush(&self) -> VirtioResult<()> {
        // If we have a write file handle, use it for flushing
        let mut write_file_guard = self.write_file.lock();
        if let Some(write_file) = write_file_guard.as_mut() {
            write_file.flush().map_err(|_| VirtioError::BackendError)?;
        }
        // If no write file exists (read-only device), there's nothing to flush
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

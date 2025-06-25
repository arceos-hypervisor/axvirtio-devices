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
/// Uses cached file handle to avoid reopening file on every operation.
#[cfg(feature = "file-backend")]
pub struct FileBackend {
    /// File path
    file_path: String,
    /// Device capacity in sectors
    capacity: u64,
    /// Read-only flag
    read_only: bool,
    /// Cached file handle for performance optimization
    cached_file: Mutex<Option<File>>,
    /// Track whether the cached file was opened with write access
    cached_file_writable: Mutex<bool>,
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
        let _file = opts.open(&file_path).map_err(|_| VirtioError::BackendError)?;

        Ok(Self {
            file_path,
            capacity: capacity_sectors,
            read_only,
            cached_file: Mutex::new(None),
            cached_file_writable: Mutex::new(false),
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

    /// Get or create a cached file handle
    ///
    /// This method implements lazy initialization of the file handle to improve performance
    /// by avoiding repeated file open/close operations.
    fn get_or_create_file(&self, write_access: bool) -> VirtioResult<()> {
        let mut cached_file_guard = self.cached_file.lock();
        let mut cached_writable_guard = self.cached_file_writable.lock();

        // Check if we already have a cached file with sufficient permissions
        if let Some(_) = cached_file_guard.as_ref() {
            // If we need write access but the cached file is read-only, we need to reopen
            if write_access && !*cached_writable_guard {
                // Clear the cache and reopen with write access
                *cached_file_guard = None;
                *cached_writable_guard = false;
            } else {
                // Existing file has sufficient permissions
                return Ok(());
            }
        }

        // Create new file handle with appropriate permissions
        let mut opts = OpenOptions::new();
        let will_be_writable = write_access && !self.read_only;

        if will_be_writable {
            opts.create(true).write(true).read(true);
        } else {
            opts.read(true);
        }

        let file = opts.open(&self.file_path).map_err(|_| VirtioError::BackendError)?;
        *cached_file_guard = Some(file);
        *cached_writable_guard = will_be_writable;
        Ok(())
    }

    /// Clear the cached file handle (used for error recovery)
    fn clear_cached_file(&self) {
        let mut cached_file_guard = self.cached_file.lock();
        let mut cached_writable_guard = self.cached_file_writable.lock();
        *cached_file_guard = None;
        *cached_writable_guard = false;
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

        // Ensure we have a cached file handle
        if let Err(e) = self.get_or_create_file(false) {
            self.clear_cached_file();
            return Err(e);
        }

        // Calculate byte offset
        let offset = sector * SECTOR_SIZE_U64;

        // Use the cached file handle
        let mut cached_file_guard = self.cached_file.lock();
        let file = cached_file_guard.as_mut().unwrap(); // Safe because get_or_create_file succeeded

        // Seek to the offset and read data
        if let Err(_) = file.seek(SeekFrom::Start(offset)) {
            // Clear cache on error to allow retry
            drop(cached_file_guard);
            self.clear_cached_file();
            return Err(VirtioError::BackendError);
        }

        let bytes_read = match file.read(buffer) {
            Ok(n) => n,
            Err(_) => {
                // Clear cache on error to allow retry
                drop(cached_file_guard);
                self.clear_cached_file();
                return Err(VirtioError::BackendError);
            }
        };

        Ok(bytes_read)
    }

    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        if self.read_only {
            return Err(VirtioError::BackendError);
        }

        self.validate_access(sector, buffer.len())?;

        // Ensure we have a cached file handle with write access
        if let Err(e) = self.get_or_create_file(true) {
            self.clear_cached_file();
            return Err(e);
        }

        // Calculate byte offset
        let offset = sector * SECTOR_SIZE_U64;

        // Use the cached file handle
        let mut cached_file_guard = self.cached_file.lock();
        let file = cached_file_guard.as_mut().unwrap(); // Safe because get_or_create_file succeeded

        // Seek to the offset and write data
        if let Err(_) = file.seek(SeekFrom::Start(offset)) {
            // Clear cache on error to allow retry
            drop(cached_file_guard);
            self.clear_cached_file();
            return Err(VirtioError::BackendError);
        }

        let bytes_written = match file.write(buffer) {
            Ok(n) => n,
            Err(_) => {
                // Clear cache on error to allow retry
                drop(cached_file_guard);
                self.clear_cached_file();
                return Err(VirtioError::BackendError);
            }
        };

        Ok(bytes_written)
    }

    fn flush(&self) -> VirtioResult<()> {
        // If we have a cached file, use it for flushing
        let mut cached_file_guard = self.cached_file.lock();
        if let Some(file) = cached_file_guard.as_mut() {
            file.flush().map_err(|_| VirtioError::BackendError)?;
        }
        // If no cached file exists, there's nothing to flush
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

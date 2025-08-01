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

/// Cache structure for single sector caching with write-back support
#[cfg(feature = "file-backend")]
struct SectorCache {
    /// Cached data buffer (size = SECTOR_SIZE_U64)
    data: [u8; SECTOR_SIZE as usize],
    /// Sector number that is cached
    sector_num: u64,
    /// Whether the cache is valid
    valid: bool,
    /// Whether the cached data is dirty (needs to be written to disk)
    dirty: bool,
}

#[cfg(feature = "file-backend")]
impl SectorCache {
    fn new() -> Self {
        Self {
            data: [0; SECTOR_SIZE as usize],
            sector_num: 0,
            valid: false,
            dirty: false,
        }
    }

    fn invalidate(&mut self) {
        self.valid = false;
        self.dirty = false;
    }

    fn is_hit(&self, sector: u64) -> bool {
        self.valid && self.sector_num == sector
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn update_read(&mut self, sector: u64, data: &[u8]) {
        if data.len() >= SECTOR_SIZE as usize {
            self.data[..SECTOR_SIZE as usize].copy_from_slice(&data[..SECTOR_SIZE as usize]);
            self.sector_num = sector;
            self.valid = true;
            self.dirty = false; // Read data is clean
        }
    }

    fn update_write(&mut self, sector: u64, data: &[u8]) {
        if data.len() >= SECTOR_SIZE as usize {
            self.data[..SECTOR_SIZE as usize].copy_from_slice(&data[..SECTOR_SIZE as usize]);
            self.sector_num = sector;
            self.valid = true;
            self.dirty = true; // Write data is dirty
        }
    }

    fn get_data(&self) -> &[u8] {
        &self.data
    }

    fn mark_clean(&mut self) {
        self.dirty = false;
    }

    fn get_sector(&self) -> u64 {
        self.sector_num
    }
}

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
    /// Read/Write cache for single sector with write-back support
    cache: Mutex<SectorCache>,
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
            cache: Mutex::new(SectorCache::new()),
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

    /// Flush dirty cache to disk
    fn flush_cache_if_dirty(&self) -> VirtioResult<()> {
        let mut cache = self.cache.lock();
        
        if cache.valid && cache.is_dirty() {
            // Need to write dirty cache to disk
            let offset = cache.get_sector() * SECTOR_SIZE_U64;
            
            let mut write_file_guard = self.write_file.lock();
            let write_file = write_file_guard.as_mut().ok_or(VirtioError::BackendError)?;

            // Seek to the offset and write cached data
            if let Err(_) = write_file.seek(SeekFrom::Start(offset)) {
                return Err(VirtioError::BackendError);
            }

            write_file.write_all(cache.get_data()).map_err(|_| VirtioError::BackendError)?;
            cache.mark_clean();
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

        // For single sector reads, try cache first
        if buffer.len() == SECTOR_SIZE as usize {
            let mut cache = self.cache.lock();
            
            if cache.is_hit(sector) {
                // Cache hit
                if cache.is_dirty() {
                    // If cache is dirty, we need to write it to disk first
                    // since we're about to read the same sector
                    drop(cache); // Release lock to avoid deadlock
                    self.flush_cache_if_dirty()?;
                    
                    // Re-acquire lock and get data
                    let cache = self.cache.lock();
                    buffer.copy_from_slice(cache.get_data());
                    return Ok(SECTOR_SIZE as usize);
                } else {
                    // Cache is clean, just return cached data
                    buffer.copy_from_slice(cache.get_data());
                    return Ok(SECTOR_SIZE as usize);
                }
            } else {
                // Cache miss - check if we need to flush existing dirty cache
                if cache.valid && cache.is_dirty() {
                    // Flush existing dirty cache before loading new data
                    drop(cache); // Release lock to avoid deadlock
                    self.flush_cache_if_dirty()?;
                }
            }
        }

        // Cache miss or multi-sector read - read from file
        let offset = sector * SECTOR_SIZE_U64;
        let mut read_file_guard = self.read_file.lock();
        let read_file = read_file_guard.as_mut().ok_or(VirtioError::BackendError)?;

        // Seek to the offset and read data
        if let Err(_) = read_file.seek(SeekFrom::Start(offset)) {
            return Err(VirtioError::BackendError);
        }

        let bytes_read = read_file.read(buffer).map_err(|_| VirtioError::BackendError)?;

        // Update cache for single sector reads
        if buffer.len() == SECTOR_SIZE as usize && bytes_read == SECTOR_SIZE as usize {
            let mut cache = self.cache.lock();
            cache.update_read(sector, buffer);
        }

        Ok(bytes_read)
    }

    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        if self.read_only {
            return Err(VirtioError::BackendError);
        }

        self.validate_access(sector, buffer.len())?;

        // For single sector writes, use write-back cache
        if buffer.len() == SECTOR_SIZE as usize {
            let mut cache = self.cache.lock();
            
            // If cache has different sector and is dirty, flush it first
            if cache.valid && cache.is_dirty() && !cache.is_hit(sector) {
                drop(cache); // Release lock to avoid deadlock
                self.flush_cache_if_dirty()?;
                
                // Re-acquire lock
                let mut cache = self.cache.lock();
                cache.update_write(sector, buffer);
            } else {
                // Same sector or cache is clean, just update cache
                cache.update_write(sector, buffer);
            }
            
            return Ok(SECTOR_SIZE as usize);
        }

        // Multi-sector write - flush cache if it overlaps and write directly to disk
        let sectors_written = buffer.len() / SECTOR_SIZE as usize;
        let mut cache = self.cache.lock();
        
        // Check if any written sector overlaps with cached sector
        for i in 0..sectors_written {
            let written_sector = sector + i as u64;
            if cache.is_hit(written_sector) {
                cache.invalidate();
                break;
            }
        }
        drop(cache);

        // Write directly to disk for multi-sector writes
        let offset = sector * SECTOR_SIZE_U64;
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
        // First flush any dirty cache
        self.flush_cache_if_dirty()?;

        // Then flush the file handle
        let mut write_file_guard = self.write_file.lock();
        if let Some(write_file) = write_file_guard.as_mut() {
            write_file.flush().map_err(|_| VirtioError::BackendError)?;
        }
        
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
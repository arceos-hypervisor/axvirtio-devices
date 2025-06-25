use alloc::vec::Vec;
use spin::Mutex;

use super::traits::BlockBackend;
use crate::constants::*;
use axvirtio_common::{VirtioError, VirtioResult};

/// Memory-based block device backend
///
/// This backend stores all data in memory, useful for testing and temporary storage.
pub struct MemoryBackend {
    /// Storage data
    data: Mutex<Vec<u8>>,
    /// Device capacity in sectors
    capacity: u64,
    /// Read-only flag
    read_only: bool,
}

impl MemoryBackend {
    /// Create a new memory backend with specified capacity
    ///
    /// # Arguments
    /// * `capacity_sectors` - Capacity in 512-byte sectors
    /// * `read_only` - Whether the device should be read-only
    pub fn new(capacity_sectors: u64, read_only: bool, _device_index: usize) -> VirtioResult<Self> {
        if capacity_sectors == 0 {
            return Err(VirtioError::InvalidConfig);
        }

        let capacity_bytes = capacity_sectors * SECTOR_SIZE_U64;
        let mut data = vec![0u8; capacity_bytes as usize];

        // 写一些 fat32 的初始扇区的内容
        if capacity_sectors >= 34 {
            let boot_sector = include_bytes!("/home/debin/Codes/arceos-umhv/arm_tiny/ramdisk0.img");
            data[..boot_sector.len()].copy_from_slice(boot_sector);
        }

        Ok(Self {
            data: Mutex::new(data),
            capacity: capacity_sectors,
            read_only,
        })
    }

    /// Validate sector access bounds
    fn validate_sector_access(&self, sector: u64, size: usize) -> VirtioResult<()> {
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

impl BlockBackend for MemoryBackend {
    fn read(&self, sector: u64, buffer: &mut [u8]) -> VirtioResult<usize> {
        self.validate_sector_access(sector, buffer.len())?;

        let start_byte = (sector * SECTOR_SIZE_U64) as usize;
        let end_byte = start_byte + buffer.len();

        let data = self.data.lock();
        if end_byte > data.len() {
            return Err(VirtioError::InvalidSector);
        }

        buffer.copy_from_slice(&data[start_byte..end_byte]);
        Ok(buffer.len())
    }

    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        if self.read_only {
            return Err(VirtioError::BackendError);
        }

        self.validate_sector_access(sector, buffer.len())?;

        let start_byte = (sector * SECTOR_SIZE_U64) as usize;
        let end_byte = start_byte + buffer.len();

        let mut data = self.data.lock();
        if end_byte > data.len() {
            return Err(VirtioError::InvalidSector);
        }

        data[start_byte..end_byte].copy_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&self) -> VirtioResult<()> {
        // For memory backend, flush is a no-op
        Ok(())
    }
}

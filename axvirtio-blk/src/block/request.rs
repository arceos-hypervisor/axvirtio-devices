use crate::backend::BlockBackend;
use crate::constants::*;
use alloc::vec::Vec;
use axaddrspace::GuestPhysAddr;

/// Block request types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRequestType {
    /// Read request
    Read,
    /// Write request
    Write,
    /// Flush request
    Flush,
}

impl From<u32> for BlockRequestType {
    fn from(value: u32) -> Self {
        match value {
            VIRTIO_BLK_T_IN => BlockRequestType::Read,
            VIRTIO_BLK_T_OUT => BlockRequestType::Write,
            VIRTIO_BLK_T_FLUSH => BlockRequestType::Flush,
            _ => BlockRequestType::Read, // Default to read for unknown types
        }
    }
}

impl From<BlockRequestType> for u32 {
    fn from(request_type: BlockRequestType) -> Self {
        match request_type {
            BlockRequestType::Read => VIRTIO_BLK_T_IN,
            BlockRequestType::Write => VIRTIO_BLK_T_OUT,
            BlockRequestType::Flush => VIRTIO_BLK_T_FLUSH,
        }
    }
}

/// Data source for block requests
#[derive(Debug)]
pub enum DataSource {
    /// Guest memory buffers (for VirtIO protocol)
    GuestMemory {
        buffers: Vec<(GuestPhysAddr, usize, bool)>, // (addr, len, is_write)
        status_addr: GuestPhysAddr,
    },
}

/// Unified block request structure
#[derive(Debug)]
pub struct BlockRequest {
    /// Request type
    pub request_type: BlockRequestType,
    /// Starting sector
    pub sector: u64,
    /// Data source (buffer or guest memory)
    pub data_source: DataSource,
}

impl BlockRequest {
    /// Create a new block request with guest memory buffers
    pub fn new_virtio(
        request_type: BlockRequestType,
        sector: u64,
        buffers: Vec<(GuestPhysAddr, usize, bool)>,
        status_addr: GuestPhysAddr,
    ) -> Self {
        Self {
            request_type,
            sector,
            data_source: DataSource::GuestMemory {
                buffers,
                status_addr,
            },
        }
    }

    /// Get the size of the request in bytes
    pub fn size(&self) -> usize {
        match &self.data_source {
            DataSource::GuestMemory { buffers, .. } => buffers.iter().map(|(_, len, _)| *len).sum(),
        }
    }

    /// Execute the request and return result
    pub fn execute(&self, backend: &dyn BlockBackend) -> BlockRequestResult {
        match &self.data_source {
            DataSource::GuestMemory {
                buffers,
                status_addr,
            } => {
                let status = self.execute_guest_memory_request(backend, buffers, *status_addr);
                // Write status to guest memory
                unsafe {
                    let status_ptr = status_addr.as_usize() as *mut u8;
                    core::ptr::write_volatile(status_ptr, status);
                }
                BlockRequestResult { status }
            }
        }
    }

    /// Execute request with guest memory buffers
    fn execute_guest_memory_request(
        &self,
        backend: &dyn BlockBackend,
        buffers: &[(GuestPhysAddr, usize, bool)],
        _status_addr: GuestPhysAddr,
    ) -> u8 {
        let request_type_u32: u32 = self.request_type.into();
        match request_type_u32 {
            VIRTIO_BLK_T_IN => self.handle_read_request_guest_memory(backend, buffers),
            VIRTIO_BLK_T_OUT => self.handle_write_request_guest_memory(backend, buffers),
            VIRTIO_BLK_T_FLUSH => self.handle_flush_request_guest_memory(backend),
            _ => {
                log::warn!("Unsupported request type: {}", request_type_u32);
                VIRTIO_BLK_S_UNSUPP
            }
        }
    }

    /// Handle read request with guest memory
    fn handle_read_request_guest_memory(
        &self,
        backend: &dyn BlockBackend,
        buffers: &[(GuestPhysAddr, usize, bool)],
    ) -> u8 {
        let total_len = self.size();
        let mut buffer = vec![0u8; total_len];

        // Read data from backend
        match backend.read(self.sector, &mut buffer) {
            Ok(bytes_read) => {
                log::debug!(
                    "Read {} bytes from backend at sector {}",
                    bytes_read,
                    self.sector
                );

                // Copy data to guest memory buffers
                let mut buffer_offset = 0;
                for (guest_addr, len, is_write) in buffers {
                    if !is_write {
                        log::warn!("Read request has non-writable data buffer");
                        continue;
                    }

                    let end_offset = buffer_offset + len;
                    if end_offset > buffer.len() {
                        log::warn!("Data buffer exceeds read data range");
                        return VIRTIO_BLK_S_IOERR;
                    }

                    unsafe {
                        let dest_ptr = guest_addr.as_usize() as *mut u8;
                        core::ptr::copy_nonoverlapping(
                            buffer[buffer_offset..end_offset].as_ptr(),
                            dest_ptr,
                            *len,
                        );
                    }

                    buffer_offset = end_offset;
                }

                VIRTIO_BLK_S_OK
            }
            Err(e) => {
                log::error!("Failed to read from backend: {:?}", e);
                VIRTIO_BLK_S_IOERR
            }
        }
    }

    /// Handle write request with guest memory
    fn handle_write_request_guest_memory(
        &self,
        backend: &dyn BlockBackend,
        buffers: &[(GuestPhysAddr, usize, bool)],
    ) -> u8 {
        let total_len = self.size();
        let mut buffer = vec![0u8; total_len];
        let mut buffer_offset = 0;

        // Read data from guest memory buffers
        for (guest_addr, len, is_write) in buffers {
            if *is_write {
                log::warn!("Write request has writable data buffer");
                continue;
            }

            let end_offset = buffer_offset + len;
            if end_offset > buffer.len() {
                log::warn!("Data buffer exceeds write data range");
                return VIRTIO_BLK_S_IOERR;
            }

            unsafe {
                let src_ptr = guest_addr.as_usize() as *const u8;
                core::ptr::copy_nonoverlapping(
                    src_ptr,
                    buffer[buffer_offset..end_offset].as_mut_ptr(),
                    *len,
                );
            }

            buffer_offset = end_offset;
        }

        // Write data to backend
        match backend.write(self.sector, &buffer) {
            Ok(bytes_written) => {
                log::debug!(
                    "Wrote {} bytes to backend at sector {}",
                    bytes_written,
                    self.sector
                );
                VIRTIO_BLK_S_OK
            }
            Err(e) => {
                log::error!("Failed to write to backend: {:?}", e);
                VIRTIO_BLK_S_IOERR
            }
        }
    }

    /// Handle flush request with guest memory
    fn handle_flush_request_guest_memory(&self, backend: &dyn BlockBackend) -> u8 {
        // Flush the backend
        match backend.flush() {
            Ok(_) => {
                log::debug!("Flushed backend");
                VIRTIO_BLK_S_OK
            }
            Err(e) => {
                log::error!("Failed to flush backend: {:?}", e);
                VIRTIO_BLK_S_IOERR
            }
        }
    }
}

/// Block request processing result
#[derive(Debug)]
pub struct BlockRequestResult {
    /// Request status
    pub status: u8,
}

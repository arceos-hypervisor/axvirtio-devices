use crate::error::{VirtioError, VirtioResult};
use crate::memory::{AddressTranslator, GuestMemoryAccess, GuestMemoryAccessor};
use crate::{constants::*, VirtioDeviceType};
use alloc::vec::Vec;
use axaddrspace::GuestPhysAddr;

/// VirtIO queue descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtqDesc {
    /// Address (guest-physical)
    pub addr: u64,
    /// Length
    pub len: u32,
    /// Flags
    pub flags: u16,
    /// Next descriptor index (if VIRTQ_DESC_F_NEXT is set)
    pub next: u16,
}

impl VirtqDesc {
    /// Create a new descriptor
    pub fn new(addr: u64, len: u32, flags: u16, next: u16) -> Self {
        Self {
            addr,
            len,
            flags,
            next,
        }
    }

    /// Check if this descriptor has the NEXT flag
    pub fn has_next(&self) -> bool {
        (self.flags & VIRTQ_DESC_F_NEXT) != 0
    }

    /// Check if this descriptor is writable
    pub fn is_write(&self) -> bool {
        (self.flags & VIRTQ_DESC_F_WRITE) != 0
    }

    /// Check if this descriptor is indirect
    pub fn is_indirect(&self) -> bool {
        (self.flags & VIRTQ_DESC_F_INDIRECT) != 0
    }

    /// Get the guest physical address
    pub fn guest_addr(&self) -> GuestPhysAddr {
        GuestPhysAddr::from(self.addr as usize)
    }

    /// Set the next flag
    pub fn set_next(&mut self, has_next: bool) {
        if has_next {
            self.flags |= VIRTQ_DESC_F_NEXT;
        } else {
            self.flags &= !VIRTQ_DESC_F_NEXT;
        }
    }

    /// Set the write flag
    pub fn set_write(&mut self, is_write: bool) {
        if is_write {
            self.flags |= VIRTQ_DESC_F_WRITE;
        } else {
            self.flags &= !VIRTQ_DESC_F_WRITE;
        }
    }

    /// Set the write flag (alias for compatibility)
    pub fn set_write_only(&mut self, is_write: bool) {
        self.set_write(is_write);
    }

    /// Check if this descriptor is write-only (alias for compatibility)
    pub fn is_write_only(&self) -> bool {
        self.is_write()
    }

    /// Set the indirect flag
    pub fn set_indirect(&mut self, is_indirect: bool) {
        if is_indirect {
            self.flags |= VIRTQ_DESC_F_INDIRECT;
        } else {
            self.flags &= !VIRTQ_DESC_F_INDIRECT;
        }
    }
}

/// Descriptor table management
#[derive(Debug, Clone)]
pub struct DescriptorTable<T: AddressTranslator + Clone> {
    /// Base address of the descriptor table
    pub base_addr: GuestPhysAddr,
    /// Number of descriptors
    pub size: u16,
    /// Guest memory accessor
    memory: GuestMemoryAccessor<T>,
}

impl<T: AddressTranslator + Clone> DescriptorTable<T> {
    /// Create a new descriptor table
    pub fn new(base_addr: GuestPhysAddr, size: u16, memory: GuestMemoryAccessor<T>) -> Self {
        Self {
            base_addr,
            size,
            memory,
        }
    }

    /// Get the address of a specific descriptor
    pub fn desc_addr(&self, index: u16) -> Option<GuestPhysAddr> {
        if index >= self.size {
            return None;
        }

        let offset = index as usize * core::mem::size_of::<VirtqDesc>();
        Some(self.base_addr + offset)
    }

    /// Calculate the total size of the descriptor table
    pub fn total_size(&self) -> usize {
        self.size as usize * core::mem::size_of::<VirtqDesc>()
    }

    /// Check if the descriptor table is valid
    pub fn is_valid(&self) -> bool {
        self.base_addr.as_usize() != 0 && self.size > 0
    }

    /// Read a descriptor from the table
    pub fn read_desc(&self, index: u16) -> VirtioResult<VirtqDesc> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let desc_addr = self.desc_addr(index).ok_or(VirtioError::InvalidQueue)?;

        self.memory.read_obj(desc_addr)
    }

    /// Write a descriptor to the table
    pub fn write_desc(&self, index: u16, desc: &VirtqDesc) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let desc_addr = self.desc_addr(index).ok_or(VirtioError::InvalidQueue)?;

        self.memory.write_obj(desc_addr, *desc)?;

        Ok(())
    }

    /// Follow a descriptor chain starting from the given index
    pub fn follow_chain(&self, head_index: u16) -> VirtioResult<Vec<VirtqDesc>> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let mut descriptors = Vec::new();
        let mut current_index = head_index;

        loop {
            if current_index >= self.size {
                return Err(VirtioError::InvalidQueue);
            }

            let desc = self.read_desc(current_index)?;
            descriptors.push(desc);

            if !desc.has_next() {
                break;
            }

            current_index = desc.next;

            // Prevent infinite loops
            if descriptors.len() > self.size as usize {
                return Err(VirtioError::InvalidQueue);
            }
        }

        Ok(descriptors)
    }

    /// Get the total length of a descriptor chain
    pub fn chain_length(&self, head_index: u16) -> VirtioResult<u32> {
        let descriptors = self.follow_chain(head_index)?;
        Ok(descriptors.iter().map(|desc| desc.len).sum())
    }

    /// Check if a descriptor chain is valid
    pub fn validate_chain(&self, head_index: u16) -> VirtioResult<bool> {
        let descriptors = self.follow_chain(head_index)?;

        // Basic validation: at least one descriptor
        if descriptors.is_empty() {
            return Ok(false);
        }

        // Check for proper flag usage
        for (i, desc) in descriptors.iter().enumerate() {
            // Last descriptor should not have NEXT flag
            if i == descriptors.len() - 1 && desc.has_next() {
                return Ok(false);
            }

            // Non-last descriptors should have NEXT flag
            if i < descriptors.len() - 1 && !desc.has_next() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get data buffer descriptors (excluding first and last)
    pub fn get_data_buffers(
        &self,
        head_index: u16,
        device_type: VirtioDeviceType,
    ) -> VirtioResult<Vec<(GuestPhysAddr, usize, bool)>> {
        let descriptors = self.follow_chain(head_index)?;

        if descriptors.len() < 2 && device_type == VirtioDeviceType::Block {
            return Ok(Vec::new());
        }

        let mut buffers = Vec::new();
        if device_type == VirtioDeviceType::Block {
            for desc in &descriptors[1..descriptors.len() - 1] {
                buffers.push((
                    GuestPhysAddr::from(desc.addr as usize),
                    desc.len as usize,
                    desc.is_write(),
                ));
            }
        } else {
            for desc in &descriptors {
                buffers.push((
                    GuestPhysAddr::from(desc.addr as usize),
                    desc.len as usize,
                    desc.is_write(),
                ));
            }
        }

        Ok(buffers)
    }

    /// Get the status descriptor address (last descriptor)
    pub fn get_status_addr(&self, head_index: u16) -> VirtioResult<GuestPhysAddr> {
        let descriptors = self.follow_chain(head_index)?;

        if descriptors.is_empty() {
            return Err(VirtioError::InvalidQueue);
        }

        let status_desc = &descriptors[descriptors.len() - 1];
        if !status_desc.is_write() {
            return Err(VirtioError::InvalidQueue);
        }

        Ok(GuestPhysAddr::from(status_desc.addr as usize))
    }
}

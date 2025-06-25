mod available;
mod descriptor;
mod used;

pub use available::{AvailableRing, VirtqAvail};
pub use descriptor::{DescriptorTable, VirtqDesc};
pub use used::{UsedRing, VirtqUsed, VirtqUsedElem};

use crate::error::{VirtioError, VirtioResult};
use alloc::vec::Vec;
use axaddrspace::GuestPhysAddr;

/// VirtIO queue implementation
#[derive(Debug, Clone)]
pub struct VirtioQueue {
    /// Queue index
    pub index: u16,
    /// Maximum queue size
    pub max_size: u16,
    /// Current queue size
    pub size: u16,
    /// Queue ready flag
    pub ready: bool,
    /// Descriptor table address
    pub desc_table_addr: GuestPhysAddr,
    /// Available ring address
    pub avail_ring_addr: GuestPhysAddr,
    /// Used ring address
    pub used_ring_addr: GuestPhysAddr,
    /// Next available index
    pub next_avail: u16,
    /// Next used index
    pub next_used: u16,
    /// Event index enabled
    pub event_idx_enabled: bool,
    /// Descriptor table management
    pub desc_table: Option<DescriptorTable>,
    /// Available ring management
    pub avail_ring: Option<AvailableRing>,
    /// Used ring management
    pub used_ring: Option<UsedRing>,
}

impl VirtioQueue {
    /// Create a new VirtIO queue
    pub fn new(index: u16, max_size: u16) -> Self {
        Self {
            index,
            max_size,
            size: max_size,
            ready: false,
            desc_table_addr: GuestPhysAddr::from(0),
            avail_ring_addr: GuestPhysAddr::from(0),
            used_ring_addr: GuestPhysAddr::from(0),
            next_avail: 0,
            next_used: 0,
            event_idx_enabled: false,
            desc_table: None,
            avail_ring: None,
            used_ring: None,
        }
    }

    /// Set queue size
    pub fn set_size(&mut self, size: u16) -> VirtioResult<()> {
        if size == 0 || size > self.max_size || (size & (size - 1)) != 0 {
            return Err(VirtioError::InvalidQueue);
        }
        self.size = size;
        Ok(())
    }

    /// Set descriptor table address
    pub fn set_desc_table_addr(&mut self, addr: GuestPhysAddr) -> VirtioResult<()> {
        self.desc_table_addr = addr;
        if addr.as_usize() != 0 {
            self.desc_table = Some(DescriptorTable::new(addr, self.size));
        }
        Ok(())
    }

    /// Set available ring address
    pub fn set_avail_ring_addr(&mut self, addr: GuestPhysAddr) -> VirtioResult<()> {
        self.avail_ring_addr = addr;
        if addr.as_usize() != 0 {
            self.avail_ring = Some(AvailableRing::new(addr, self.size));
        }
        Ok(())
    }

    /// Set used ring address
    pub fn set_used_ring_addr(&mut self, addr: GuestPhysAddr) -> VirtioResult<()> {
        self.used_ring_addr = addr;
        if addr.as_usize() != 0 {
            self.used_ring = Some(UsedRing::new(addr, self.size));
        }
        Ok(())
    }

    /// Mark queue as ready
    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;
    }

    /// Check if queue is valid and ready
    pub fn is_valid(&self) -> bool {
        self.ready
            && self.desc_table_addr.as_usize() != 0
            && self.avail_ring_addr.as_usize() != 0
            && self.used_ring_addr.as_usize() != 0
    }

    /// Reset the queue
    pub fn reset(&mut self) {
        self.ready = false;
        self.desc_table_addr = GuestPhysAddr::from(0);
        self.avail_ring_addr = GuestPhysAddr::from(0);
        self.used_ring_addr = GuestPhysAddr::from(0);
        self.next_avail = 0;
        self.next_used = 0;
        self.desc_table = None;
        self.avail_ring = None;
        self.used_ring = None;
    }

    /// Read available ring index
    pub fn read_avail_idx(&self) -> VirtioResult<u16> {
        if let Some(ref avail_ring) = self.avail_ring {
            avail_ring.get_avail_idx()
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Add a used buffer to the used ring
    pub fn add_used(&mut self, desc_index: u16, len: u32) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        // Use the UsedRing to properly manage the used ring
        if let Some(ref mut used_ring) = self.used_ring {
            used_ring.add_used(desc_index as u32, len)?;
            self.next_used = used_ring.get_used_idx();
        } else {
            // Fallback: just update the index
            self.next_used = (self.next_used + 1) % self.size;
        }

        Ok(())
    }

    /// Get next available descriptor
    pub fn pop_avail(&mut self) -> VirtioResult<Option<u16>> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        // In a real implementation, this would read from guest memory
        // For now, return None to indicate no available descriptors
        Ok(None)
    }

    /// Get the used ring reference
    pub fn get_used_ring(&self) -> Option<&UsedRing> {
        self.used_ring.as_ref()
    }

    /// Get the used ring mutable reference
    pub fn get_used_ring_mut(&mut self) -> Option<&mut UsedRing> {
        self.used_ring.as_mut()
    }

    /// Get the available ring reference
    pub fn get_avail_ring(&self) -> Option<&AvailableRing> {
        self.avail_ring.as_ref()
    }

    /// Get the descriptor table reference
    pub fn get_desc_table(&self) -> Option<&DescriptorTable> {
        self.desc_table.as_ref()
    }

    /// Read available ring entry
    pub fn read_avail_entry(&self, ring_index: u16) -> VirtioResult<u16> {
        if let Some(ref avail_ring) = self.avail_ring {
            avail_ring.read_avail_ring_entry(ring_index)
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Update last available index
    pub fn update_last_avail_idx(&mut self, idx: u16) {
        if let Some(ref mut avail_ring) = self.avail_ring {
            avail_ring.update_last_avail_idx(idx);
        }
    }

    /// Validate VirtIO block chain
    pub fn validate_virtio_block_chain(
        &self,
        head_index: u16,
        min_length: usize,
    ) -> VirtioResult<bool> {
        if let Some(ref desc_table) = self.desc_table {
            let descriptors = desc_table.follow_chain(head_index)?;
            Ok(descriptors.len() >= min_length)
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Parse VirtIO block header
    pub fn parse_virtio_block_header(&self, head_index: u16) -> VirtioResult<(u32, u64)> {
        if let Some(ref desc_table) = self.desc_table {
            let descriptors = desc_table.follow_chain(head_index)?;
            if descriptors.is_empty() {
                return Err(VirtioError::InvalidDescriptor);
            }

            // For now, return dummy values
            // In a real implementation, this would read the header from guest memory
            Ok((0, 0)) // (request_type, sector)
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Get data buffers from descriptor chain
    pub fn get_data_buffers(
        &self,
        head_index: u16,
    ) -> VirtioResult<Vec<(axaddrspace::GuestPhysAddr, usize, bool)>> {
        if let Some(ref desc_table) = self.desc_table {
            desc_table.get_data_buffers(head_index)
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Get status address from descriptor chain
    pub fn get_status_addr(&self, head_index: u16) -> VirtioResult<axaddrspace::GuestPhysAddr> {
        if let Some(ref desc_table) = self.desc_table {
            desc_table.get_status_addr(head_index)
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Check if should notify
    pub fn should_notify(&self) -> VirtioResult<bool> {
        if let Some(ref used_ring) = self.used_ring {
            used_ring.should_notify()
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }
}

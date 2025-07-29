use crate::constants::*;
use crate::error::{VirtioError, VirtioResult};
use crate::memory::GuestMemoryAccess;
use axaddrspace::GuestPhysAddr;

/// VirtIO used ring element
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtqUsedElem {
    /// Index of start of used descriptor chain
    pub id: u32,
    /// Total length of the descriptor chain which was used
    pub len: u32,
}

impl VirtqUsedElem {
    /// Create a new used element
    pub fn new(id: u32, len: u32) -> Self {
        Self { id, len }
    }
}

/// VirtIO used ring structure
///
/// This is followed by:
/// - ring[queue_size]: VirtqUsedElem array
/// - avail_event: u16 (if event_idx feature is enabled)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtqUsed {
    /// Flags
    pub flags: u16,
    /// Index of the next used element
    pub idx: u16,
    // Ring of used elements (variable length)
}

impl VirtqUsed {
    /// Create a new used ring header
    pub fn new() -> Self {
        Self { flags: 0, idx: 0 }
    }

    /// Check if notifications are disabled
    pub fn no_notify(&self) -> bool {
        (self.flags & VIRTQ_USED_F_NO_NOTIFY) != 0
    }

    /// Set the no notify flag
    pub fn set_no_notify(&mut self, no_notify: bool) {
        if no_notify {
            self.flags |= VIRTQ_USED_F_NO_NOTIFY;
        } else {
            self.flags &= !VIRTQ_USED_F_NO_NOTIFY;
        }
    }
}

/// Used ring management
#[derive(Debug, Clone)]
pub struct UsedRing<M: GuestMemoryAccess> {
    /// Base address of the used ring
    pub base_addr: GuestPhysAddr,
    /// Queue size
    pub size: u16,
    /// Current used index
    pub used_idx: u16,
    /// Guest memory accessor
    memory: M,
}

impl<M: GuestMemoryAccess> UsedRing<M> {
    /// Create a new used ring
    pub fn new(base_addr: GuestPhysAddr, size: u16, memory: M) -> Self {
        Self {
            base_addr,
            size,
            used_idx: 0,
            memory,
        }
    }

    /// Get the address of the used ring header
    pub fn header_addr(&self) -> GuestPhysAddr {
        self.base_addr
    }

    /// Get the address of the ring array
    pub fn ring_addr(&self) -> GuestPhysAddr {
        self.base_addr + core::mem::size_of::<VirtqUsed>()
    }

    /// Get the address of a specific ring entry
    pub fn ring_entry_addr(&self, index: u16) -> Option<GuestPhysAddr> {
        if index >= self.size {
            return None;
        }

        let offset = core::mem::size_of::<VirtqUsed>()
            + (index as usize * core::mem::size_of::<VirtqUsedElem>());
        Some(self.base_addr + offset)
    }

    /// Get the address of the available event field (if event_idx is enabled)
    pub fn avail_event_addr(&self) -> GuestPhysAddr {
        let offset = core::mem::size_of::<VirtqUsed>()
            + (self.size as usize * core::mem::size_of::<VirtqUsedElem>());
        self.base_addr + offset
    }

    /// Calculate the total size of the used ring
    pub fn total_size(&self) -> usize {
        core::mem::size_of::<VirtqUsed>()
            + (self.size as usize * core::mem::size_of::<VirtqUsedElem>())
            + 2
    }

    /// Check if the used ring is valid
    pub fn is_valid(&self) -> bool {
        self.base_addr.as_usize() != 0 && self.size > 0
    }

    /// Add a used element to the ring
    pub fn add_used(&mut self, id: u32, len: u32) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        // Calculate the address of the used element to write
        let ring_index = self.used_idx % self.size;
        let elem_addr = self
            .ring_entry_addr(ring_index)
            .ok_or(VirtioError::InvalidQueue)?;

        // Create the used element
        let used_elem = VirtqUsedElem::new(id, len);

        // Write the used element to guest memory using injected memory accessor
        self.memory.write_obj(elem_addr, used_elem)?;

        // Update the used index
        self.used_idx = self.used_idx.wrapping_add(1);

        // Update the used ring header index
        self.write_used_idx()?;

        Ok(())
    }

    /// Write the used index to the used ring header
    pub fn write_used_idx(&self) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        // Write the used index to the header (offset 2 bytes for flags)
        let idx_addr = self.base_addr + 2;
        self.memory.write_obj(idx_addr, self.used_idx)?;

        Ok(())
    }

    /// Read the used ring header
    pub fn read_used_header(&self) -> VirtioResult<VirtqUsed> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        self.memory.read_obj(self.base_addr)
    }

    /// Write the used ring header
    pub fn write_used_header(&self, header: &VirtqUsed) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        self.memory.write_obj(self.base_addr, *header)
    }

    /// Get the current used index
    pub fn get_used_idx(&self) -> u16 {
        self.used_idx
    }

    /// Set the used index
    pub fn set_used_idx(&mut self, idx: u16) {
        self.used_idx = idx;
    }

    /// Check if notifications should be suppressed
    pub fn should_notify(&self) -> VirtioResult<bool> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let header = self.read_used_header()?;
        Ok(!header.no_notify())
    }

    /// Set notification suppression
    pub fn set_notification(&self, suppress: bool) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let mut header = self.read_used_header()?;
        header.set_no_notify(suppress);
        self.write_used_header(&header)?;

        Ok(())
    }
}

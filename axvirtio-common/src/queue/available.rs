use crate::constants::*;
use crate::error::{VirtioError, VirtioResult};
use axaddrspace::GuestPhysAddr;

/// VirtIO available ring structure
///
/// This is followed by:
/// - ring[queue_size]: u16 array of descriptor indices
/// - used_event: u16 (if event_idx feature is enabled)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtqAvail {
    /// Flags
    pub flags: u16,
    /// Index of the next available descriptor
    pub idx: u16,
    // Ring of available descriptor indices (variable length)
}

impl VirtqAvail {
    /// Create a new available ring header
    pub fn new() -> Self {
        Self { flags: 0, idx: 0 }
    }

    /// Check if interrupts are disabled
    pub fn no_interrupt(&self) -> bool {
        (self.flags & VIRTQ_AVAIL_F_NO_INTERRUPT) != 0
    }

    /// Set the no interrupt flag
    pub fn set_no_interrupt(&mut self, no_interrupt: bool) {
        if no_interrupt {
            self.flags |= VIRTQ_AVAIL_F_NO_INTERRUPT;
        } else {
            self.flags &= !VIRTQ_AVAIL_F_NO_INTERRUPT;
        }
    }
}

/// Available ring management
#[derive(Debug, Clone)]
pub struct AvailableRing {
    /// Base address of the available ring
    pub base_addr: GuestPhysAddr,
    /// Queue size
    pub size: u16,
    /// Last seen available index
    pub last_avail_idx: u16,
}

impl AvailableRing {
    /// Create a new available ring
    pub fn new(base_addr: GuestPhysAddr, size: u16) -> Self {
        Self {
            base_addr,
            size,
            last_avail_idx: 0,
        }
    }

    /// Get the address of the available ring header
    pub fn header_addr(&self) -> GuestPhysAddr {
        self.base_addr
    }

    /// Get the address of the ring array
    pub fn ring_addr(&self) -> GuestPhysAddr {
        self.base_addr + core::mem::size_of::<VirtqAvail>()
    }

    /// Get the address of a specific ring entry
    pub fn ring_entry_addr(&self, index: u16) -> Option<GuestPhysAddr> {
        if index >= self.size {
            return None;
        }

        let offset = core::mem::size_of::<VirtqAvail>() + (index as usize * 2);
        Some(self.base_addr + offset)
    }

    /// Get the address of the used event field (if event_idx is enabled)
    pub fn used_event_addr(&self) -> GuestPhysAddr {
        let offset = core::mem::size_of::<VirtqAvail>() + (self.size as usize * 2);
        self.base_addr + offset
    }

    /// Calculate the total size of the available ring
    pub fn total_size(&self) -> usize {
        core::mem::size_of::<VirtqAvail>() + (self.size as usize * 2) + 2
    }

    /// Check if the available ring is valid
    pub fn is_valid(&self) -> bool {
        self.base_addr.as_usize() != 0 && self.size > 0
    }

    /// Check if there are new available descriptors
    pub fn has_new_avail(&self, current_idx: u16) -> bool {
        current_idx != self.last_avail_idx
    }

    /// Update the last seen available index
    pub fn update_last_avail_idx(&mut self, idx: u16) {
        self.last_avail_idx = idx;
    }

    /// Read the available ring header
    pub fn read_avail_header(&self) -> VirtioResult<VirtqAvail> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        unsafe {
            let header_ptr = self.base_addr.as_usize() as *const VirtqAvail;
            Ok(core::ptr::read_volatile(header_ptr))
        }
    }

    /// Write the available ring header
    pub fn write_avail_header(&self, header: &VirtqAvail) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        unsafe {
            let header_ptr = self.base_addr.as_usize() as *mut VirtqAvail;
            core::ptr::write_volatile(header_ptr, *header);
        }

        Ok(())
    }

    /// Read the current available index from guest memory
    pub fn read_avail_idx(&self) -> VirtioResult<u16> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        // Read the idx field from the header (offset 2 bytes for flags)
        let idx_addr = self.base_addr + 2;
        unsafe {
            let idx_ptr = idx_addr.as_usize() as *const u16;
            Ok(core::ptr::read_volatile(idx_ptr))
        }
    }

    /// Get the available index for external access
    pub fn get_avail_idx(&self) -> VirtioResult<u16> {
        self.read_avail_idx()
    }

    /// Read a descriptor index from the available ring
    pub fn read_avail_ring_entry(&self, ring_index: u16) -> VirtioResult<u16> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let entry_addr = self
            .ring_entry_addr(ring_index % self.size)
            .ok_or(VirtioError::InvalidQueue)?;

        unsafe {
            let entry_ptr = entry_addr.as_usize() as *const u16;
            Ok(core::ptr::read_volatile(entry_ptr))
        }
    }

    /// Write a descriptor index to the available ring
    pub fn write_avail_ring_entry(&self, ring_index: u16, desc_index: u16) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let entry_addr = self
            .ring_entry_addr(ring_index % self.size)
            .ok_or(VirtioError::InvalidQueue)?;

        unsafe {
            let entry_ptr = entry_addr.as_usize() as *mut u16;
            core::ptr::write_volatile(entry_ptr, desc_index);
        }

        Ok(())
    }

    /// Get the number of available descriptors since last check
    pub fn get_available_count(&self) -> VirtioResult<u16> {
        let current_idx = self.read_avail_idx()?;
        Ok(current_idx.wrapping_sub(self.last_avail_idx))
    }

    /// Check if interrupts are suppressed
    pub fn interrupts_suppressed(&self) -> VirtioResult<bool> {
        let header = self.read_avail_header()?;
        Ok(header.no_interrupt())
    }

    /// Set interrupt suppression
    pub fn set_interrupt_suppression(&self, suppress: bool) -> VirtioResult<()> {
        let mut header = self.read_avail_header()?;
        header.set_no_interrupt(suppress);
        self.write_avail_header(&header)?;
        Ok(())
    }

    /// Read the used event field (for event_idx feature)
    pub fn read_used_event(&self) -> VirtioResult<u16> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let event_addr = self.used_event_addr();
        unsafe {
            let event_ptr = event_addr.as_usize() as *const u16;
            Ok(core::ptr::read_volatile(event_ptr))
        }
    }

    /// Write the used event field (for event_idx feature)
    pub fn write_used_event(&self, event: u16) -> VirtioResult<()> {
        if !self.is_valid() {
            return Err(VirtioError::QueueNotReady);
        }

        let event_addr = self.used_event_addr();
        unsafe {
            let event_ptr = event_addr.as_usize() as *mut u16;
            core::ptr::write_volatile(event_ptr, event);
        }

        Ok(())
    }
}

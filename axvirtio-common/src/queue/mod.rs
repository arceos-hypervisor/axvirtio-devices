mod available;
mod descriptor;
mod used;

pub use available::{AvailableRing, VirtQueueAvail};
pub use descriptor::{DescriptorTable, VirtQueueDesc};
use log::trace;
pub use used::{UsedRing, VirtQueueUsed, VirtqUsedElem};

use crate::{
    VirtioDeviceID,
    error::{VirtioError, VirtioResult},
};
use alloc::{sync::Arc, vec::Vec};
use axaddrspace::{GuestMemoryAccessor, GuestPhysAddr};

/// VirtIO queue implementation
#[derive(Debug, Clone)]
pub struct VirtioQueue<T: GuestMemoryAccessor + Clone> {
    /// Queue index
    pub index: u16,
    /// Queue size
    pub size: u16,
    /// Descriptor table
    pub desc_table: Option<DescriptorTable<T>>,
    /// Available ring
    avail_ring: Option<AvailableRing<T>>,
    /// Used ring
    used_ring: Option<UsedRing<T>>,
    /// Guest memory accessor
    accessor: Arc<T>,
    /// Maximum queue size
    pub max_size: u16,
    /// Queue ready flag
    pub ready: bool,
    /// Descriptor table address (guest physical)
    pub desc_table_addr: GuestPhysAddr,
    /// Available ring address (guest physical)
    pub avail_ring_addr: GuestPhysAddr,
    /// Used ring address (guest physical)
    pub used_ring_addr: GuestPhysAddr,
    /// Next available index
    next_avail: u16,
    /// Next used index
    next_used: u16,
    /// Event index enabled (VIRTIO_F_RING_EVENT_IDX)
    pub event_idx_enabled: bool,
    /// Last used index when we signalled an interrupt (for EVENT_IDX)
    signalled_used: u16,
    /// Whether signalled_used is valid (have we ever signalled?)
    signalled_used_valid: bool,
}

impl<T: GuestMemoryAccessor + Clone> VirtioQueue<T> {
    /// Create a new VirtIO queue
    pub fn new(index: u16, size: u16, accessor: Arc<T>) -> Self {
        Self {
            index,
            size,
            desc_table: None,
            avail_ring: None,
            used_ring: None,
            accessor,
            max_size: size,
            ready: false,
            desc_table_addr: GuestPhysAddr::from(0),
            avail_ring_addr: GuestPhysAddr::from(0),
            used_ring_addr: GuestPhysAddr::from(0),
            next_avail: 0,
            next_used: 0,
            event_idx_enabled: false,
            signalled_used: 0,
            signalled_used_valid: false,
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
        if self.desc_table_addr.as_usize() != 0 {
            return Err(VirtioError::InvalidConfig);
        }
        self.desc_table_addr = addr;
        if addr.as_usize() != 0 {
            self.desc_table = Some(DescriptorTable::new(addr, self.size, self.accessor.clone()));
        }
        Ok(())
    }

    /// Set available ring address
    pub fn set_avail_ring_addr(&mut self, addr: GuestPhysAddr) -> VirtioResult<()> {
        if self.avail_ring_addr.as_usize() != 0 {
            return Err(VirtioError::InvalidConfig);
        }
        self.avail_ring_addr = addr;
        if addr.as_usize() != 0 {
            self.avail_ring = Some(AvailableRing::new(addr, self.size, self.accessor.clone()));
        }
        Ok(())
    }

    /// Set used ring address
    pub fn set_used_ring_addr(&mut self, addr: GuestPhysAddr) -> VirtioResult<()> {
        if self.used_ring_addr.as_usize() != 0 {
            return Err(VirtioError::InvalidConfig);
        }
        self.used_ring_addr = addr;
        if addr.as_usize() != 0 {
            self.used_ring = Some(UsedRing::new(addr, self.size, self.accessor.clone()));
        }
        Ok(())
    }

    /// Mark queue as ready and initialize used ring in guest memory
    pub fn set_ready(&mut self, ready: bool) {
        self.ready = ready;

        // When queue becomes ready, initialize used ring in guest memory
        if ready {
            if let Some(ref used_ring) = self.used_ring {
                // Initialize used_ring->flags to 0 (no notification suppression)
                // This is critical for non-EVENT_IDX mode (flags mode)
                if let Err(e) = used_ring.write_used_header(&VirtQueueUsed::new()) {
                    log::warn!(
                        "[VirtioQueue] Failed to initialize used ring header: {:?}",
                        e
                    );
                }
                // Initialize avail_event to 0 for EVENT_IDX mode
                if self.event_idx_enabled {
                    if let Err(e) = used_ring.write_avail_event(0) {
                        log::warn!("[VirtioQueue] Failed to initialize avail_event: {:?}", e);
                    }
                }
                log::info!(
                    "[VirtioQueue] Queue {} initialized: used_ring flags=0, idx=0",
                    self.index
                );
            }
        }
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
        self.event_idx_enabled = false;
        self.signalled_used = 0;
        self.signalled_used_valid = false;
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
    pub fn get_used_ring(&self) -> Option<&UsedRing<T>> {
        self.used_ring.as_ref()
    }

    /// Get the used ring mutable reference
    pub fn get_used_ring_mut(&mut self) -> Option<&mut UsedRing<T>> {
        self.used_ring.as_mut()
    }

    /// Get the available ring reference
    pub fn get_avail_ring(&self) -> Option<&AvailableRing<T>> {
        self.avail_ring.as_ref()
    }

    /// Get the descriptor table reference
    pub fn get_desc_table(&self) -> Option<&DescriptorTable<T>> {
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
        } else {
            self.next_avail = idx % self.size;
        }
    }

    /// Get last available index
    pub fn get_last_avail_idx(&self) -> u16 {
        if let Some(avail_ring) = &self.avail_ring {
            avail_ring.last_avail_idx
        } else {
            self.next_avail
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

    /// Get data buffers from descriptor chain
    pub fn get_data_buffers(
        &self,
        head_index: u16,
        device_type: VirtioDeviceID,
    ) -> VirtioResult<Vec<(axaddrspace::GuestPhysAddr, usize, bool)>> {
        if let Some(ref desc_table) = self.desc_table {
            desc_table.get_data_buffers(head_index, device_type)
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

    /// Check if should notify (send interrupt to guest)
    ///
    /// According to VirtIO spec section 2.6.8:
    /// - If VIRTIO_F_RING_EVENT_IDX is NOT negotiated: check avail->flags for
    ///   VIRTQ_AVAIL_F_NO_INTERRUPT. If set, don't send interrupt.
    /// - If VIRTIO_F_RING_EVENT_IDX IS negotiated: check used_event from avail ring.
    ///   Device should notify if: (u16)(new - used_event - 1) < (u16)(new - old)
    ///   where new is current used_idx, old is used_idx when we last signalled.
    pub fn should_notify(&mut self) -> VirtioResult<bool> {
        // Get current used index
        let new_idx = if let Some(ref used_ring) = self.used_ring {
            used_ring.get_used_idx()
        } else {
            return Err(VirtioError::QueueNotReady);
        };

        if self.event_idx_enabled {
            // EVENT_IDX mode: check used_event threshold
            if let Some(ref avail_ring) = self.avail_ring {
                let used_event = avail_ring.read_used_event()?;
                let old_idx = self.signalled_used;
                let was_valid = self.signalled_used_valid;

                // Update signalled_used for next time
                self.signalled_used = new_idx;
                self.signalled_used_valid = true;

                // If we've never signalled, always notify
                if !was_valid {
                    log::trace!(
                        "[VirtioQueue] should_notify (EVENT_IDX, first): new_idx={}, used_event={}, returning true",
                        new_idx,
                        used_event
                    );
                    return Ok(true);
                }

                // VirtIO spec formula: notify if (new - used_event - 1) < (new - old)
                // This is true when we've "crossed" the used_event threshold
                let should = new_idx.wrapping_sub(used_event).wrapping_sub(1)
                    < new_idx.wrapping_sub(old_idx);

                log::trace!(
                    "[VirtioQueue] should_notify (EVENT_IDX): new_idx={}, old_idx={}, used_event={}, returning {}",
                    new_idx,
                    old_idx,
                    used_event,
                    should
                );

                Ok(should)
            } else {
                Err(VirtioError::QueueNotReady)
            }
        } else {
            // Non-EVENT_IDX mode: check avail->flags for VIRTQ_AVAIL_F_NO_INTERRUPT
            if let Some(ref avail_ring) = self.avail_ring {
                let suppressed = avail_ring.interrupts_suppressed()?;
                log::trace!(
                    "[VirtioQueue] should_notify (flags): interrupts_suppressed={}, returning {}",
                    suppressed,
                    !suppressed
                );
                Ok(!suppressed)
            } else {
                Err(VirtioError::QueueNotReady)
            }
        }
    }

    /// Set event index feature enabled/disabled
    pub fn set_event_idx_enabled(&mut self, enabled: bool) {
        self.event_idx_enabled = enabled;
        if enabled {
            // Reset signalled state when enabling
            self.signalled_used_valid = false;
        }
        log::info!("[VirtioQueue] event_idx_enabled set to {}", enabled);
    }

    /// Write status byte to the status buffer of a descriptor chain
    ///
    /// This method writes the status byte to the last descriptor in the chain,
    /// which should be a write-only descriptor according to VirtIO specification.
    pub fn write_status_byte(&self, head_index: u16, status: u8) -> VirtioResult<()> {
        // Get the status descriptor address (last descriptor in chain)
        let status_addr_guest = self.get_status_addr(head_index)?;

        trace!(
            "[VirtioQueue] Writing status byte {} to guest address 0x{:x} for descriptor chain {}",
            status,
            status_addr_guest.as_usize(),
            head_index
        );

        // Write the status byte to guest memory using the new memory access interface
        self.accessor
            .write_obj(status_addr_guest, status)
            .map_err(|_| VirtioError::InvalidAddress)?;

        // Verify the status byte was written correctly
        let readback: u8 = self
            .accessor
            .read_obj(status_addr_guest)
            .map_err(|_| VirtioError::InvalidAddress)?;
        trace!(
            "[VirtioQueue] Status byte verified: wrote={}, readback={}",
            status, readback
        );

        Ok(())
    }

    /// Write avail_event to used ring to tell guest when to send notifications
    ///
    /// When EVENT_IDX is enabled, the device should write avail_event to indicate
    /// when the guest should send the next QUEUE_NOTIFY. Setting avail_event to
    /// the current avail_idx tells the guest to notify on the next request.
    pub fn write_avail_event(&self, event: u16) -> VirtioResult<()> {
        if !self.event_idx_enabled {
            // Only write avail_event when EVENT_IDX is enabled
            return Ok(());
        }

        if let Some(ref used_ring) = self.used_ring {
            used_ring.write_avail_event(event)
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }

    /// Read avail_event from used ring
    pub fn read_avail_event(&self) -> VirtioResult<u16> {
        if let Some(ref used_ring) = self.used_ring {
            used_ring.read_avail_event()
        } else {
            Err(VirtioError::QueueNotReady)
        }
    }
}

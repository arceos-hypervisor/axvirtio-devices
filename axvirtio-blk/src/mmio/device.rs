use alloc::boxed::Box;
use alloc::vec::Vec;
use axaddrspace::{device::AccessWidth, GuestPhysAddr};
use axerrno::AxResult;

use log::{error, info, trace, warn};
use memory_addr::MemoryAddr;
use spin::Mutex;

use crate::backend::{create_default_backend, BlockBackend};
use crate::block::config::VirtioBlockConfig;
use crate::block::request::BlockRequestType;
use crate::block::BlockRequest;
use crate::constants::*;
use axvirtio_common::{
     MmioTransport, VirtioConfig, VirtioError, VirtioQueue, VirtioResult,
};

/// VirtIO MMIO device state
pub struct VirtioMmioDevice {
    /// Base IPA address
    pub(crate) base_ipa: GuestPhysAddr,
    /// MMIO region length
    pub(crate) length: usize,
    /// Device configuration
    config: VirtioConfig,
    /// Block device configuration
    block_config: VirtioBlockConfig,
    /// Device status
    status: Mutex<u32>,
    /// Device features selected by driver
    driver_features: Mutex<u64>,
    /// Device features selector
    device_features_sel: Mutex<u32>,
    /// Driver features selector
    driver_features_sel: Mutex<u32>,
    /// Current queue selector
    queue_sel: Mutex<u16>,
    /// VirtIO queues
    queues: Mutex<Vec<VirtioQueue>>,
    /// Interrupt status
    interrupt_status: Mutex<u32>,
    /// Configuration generation
    config_generation: Mutex<u32>,
    /// Block backend
    backend: Box<dyn BlockBackend>,
}

impl VirtioMmioDevice {
    /// Create a new VirtIO MMIO device with device index
    pub fn new(device_index: usize) -> VirtioResult<Self> {
        let config = VirtioConfig::new_block_device(device_index);
        let mut queues = Vec::new();

        // Create default queue
        queues.push(VirtioQueue::new(0, config.max_queue_size));

        // Get the actual device MMIO address based on device_index
        let base_ipa = config.get_device_mmio_addr();
        let length = config.total_mmio_size;

        // Create backend
        let backend = create_default_backend(device_index)?;

        Ok(Self {
            base_ipa,
            length,
            config,
            block_config: VirtioBlockConfig::default(),
            status: Mutex::new(0),
            driver_features: Mutex::new(VIRTIO_BLK_FEATURES.try_into().unwrap()),
            device_features_sel: Mutex::new(0),
            driver_features_sel: Mutex::new(0),
            queue_sel: Mutex::new(0),
            queues: Mutex::new(queues),
            interrupt_status: Mutex::new(0),
            config_generation: Mutex::new(0),
            backend,
        })
    }

    /// Check if device index is valid
    pub fn is_enabled(&self) -> bool {
        self.config.is_valid_device_index()
    }

    /// Check if an address is within this device's MMIO range
    pub fn is_address_in_range(&self, addr: GuestPhysAddr) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let (start, end) = self.config.get_mmio_range();
        addr >= start && addr < end
    }

    /// Get device status
    pub fn get_status(&self) -> u32 {
        *self.status.lock()
    }

    /// Set device status
    pub fn set_status(&self, status: u32) {
        *self.status.lock() = status;
    }

    /// Check if device is ready
    pub fn is_device_ready(&self) -> bool {
        let status = self.get_status();
        (status & VIRTIO_STATUS_DRIVER_OK) != 0
    }

    /// Handle MMIO read operations
    pub fn mmio_read(&self, addr: GuestPhysAddr, width: AccessWidth) -> AxResult<usize> {
        // Check if device is enabled
        if !self.is_enabled() {
            return Ok(0);
        }

        // Check if address is within the overall MMIO range (self.config.base_addr - self.config.base_addr + self.config.total_mmio_size)
        let max_mmio = GuestPhysAddr::from(self.config.base_addr.add(self.config.total_mmio_size));
        if addr < self.config.base_addr || addr >= max_mmio {
            return Ok(0);
        }

        // Check if address is within this device's specific range based on device_index
        let device_start =
            self.config.base_addr + (self.config.device_index * self.config.mmio_size);
        let device_end = device_start + self.config.mmio_size;
        if addr < device_start || addr >= device_end {
            // Return 0 for addresses outside this device's range
            return Ok(0);
        }

        // Validate access width
        MmioTransport::validate_access_width(width)?;

        // Calculate offset from this device's base address
        let offset = MmioTransport::calculate_offset(addr, device_start);

        let value = match offset {
            VIRTIO_MMIO_MAGIC_VALUE => MMIO_MAGIC_VALUE,
            VIRTIO_MMIO_VERSION => MMIO_VERSION,
            VIRTIO_MMIO_DEVICE_ID => self.config.device_id,
            VIRTIO_MMIO_VENDOR_ID => self.config.vendor_id,
            VIRTIO_MMIO_DEVICE_FEATURES => {
                let sel = *self.device_features_sel.lock();
                match sel {
                    0 => self.config.device_features as u32,
                    1 => (self.config.device_features >> 32) as u32,
                    _ => 0,
                }
            }
            VIRTIO_MMIO_DEVICE_FEATURES_SEL => *self.device_features_sel.lock(),
            VIRTIO_MMIO_DRIVER_FEATURES => {
                let sel = *self.driver_features_sel.lock();
                let features = *self.driver_features.lock();
                match sel {
                    0 => features as u32,
                    1 => (features >> 32) as u32,
                    _ => 0,
                }
            }
            VIRTIO_MMIO_DRIVER_FEATURES_SEL => *self.driver_features_sel.lock(),
            VIRTIO_MMIO_QUEUE_SEL => *self.queue_sel.lock() as u32,
            VIRTIO_MMIO_QUEUE_NUM_MAX => self.config.max_queue_size as u32,
            VIRTIO_MMIO_QUEUE_NUM => {
                let queue_sel = *self.queue_sel.lock();
                let queues = self.queues.lock();
                if let Some(queue) = queues.get(queue_sel as usize) {
                    queue.size as u32
                } else {
                    0
                }
            }
            VIRTIO_MMIO_QUEUE_READY => {
                let queue_sel = *self.queue_sel.lock();
                let queues = self.queues.lock();
                if let Some(queue) = queues.get(queue_sel as usize) {
                    if queue.ready {
                        1
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            VIRTIO_MMIO_INTERRUPT_STATUS => *self.interrupt_status.lock(),
            VIRTIO_MMIO_STATUS => *self.status.lock(),
            VIRTIO_MMIO_CONFIG_GENERATION => *self.config_generation.lock(),
            _ => {
                // Check if this is a config space read (offset >= 0x100)
                if offset >= VIRTIO_MMIO_CONFIG {
                    self.read_config_space((offset - VIRTIO_MMIO_CONFIG) as u64, width)? as u32
                } else {
                    return Err(VirtioError::InvalidRegister.into());
                }
            }
        };

        Ok(value as usize)
    }

    /// Handle MMIO write operations
    pub fn mmio_write(&self, addr: GuestPhysAddr, width: AccessWidth, val: usize) -> AxResult<()> {
        // Check if device is enabled
        if !self.is_enabled() {
            return Ok(());
        }

        // Check if address is within the overall MMIO range
        let base_mmio = GuestPhysAddr::from(VIRTIO_MMIO_BASE);
        let max_mmio = GuestPhysAddr::from(VIRTIO_MMIO_BASE + VIRTIO_MMIO_TOTAL_SIZE);
        if addr < base_mmio || addr >= max_mmio {
            return Ok(());
        }

        // Check if address is within this device's specific range based on device_index
        let device_start = base_mmio + (self.config.device_index * VIRTIO_MMIO_DEVICE_SIZE);
        let device_end = device_start + VIRTIO_MMIO_DEVICE_SIZE;
        if addr < device_start || addr >= device_end {
            // Ignore writes to addresses outside this device's range
            return Ok(());
        }

        // Validate access width
        MmioTransport::validate_access_width(width)?;

        // Calculate offset from this device's base address
        let offset = MmioTransport::calculate_offset(addr, device_start);
        let val = val as u32;

        match offset {
            VIRTIO_MMIO_DEVICE_FEATURES_SEL => {
                *self.device_features_sel.lock() = val;
            }
            VIRTIO_MMIO_DRIVER_FEATURES => {
                let sel = *self.driver_features_sel.lock();
                let mut features = self.driver_features.lock();
                match sel {
                    0 => {
                        *features = (*features & 0xFFFF_FFFF_0000_0000) | (val as u64);
                    }
                    1 => {
                        *features = (*features & 0x0000_0000_FFFF_FFFF) | ((val as u64) << 32);
                    }
                    _ => {} // Ignore invalid selector
                }
            }
            VIRTIO_MMIO_DRIVER_FEATURES_SEL => {
                *self.driver_features_sel.lock() = val;
            }
            VIRTIO_MMIO_QUEUE_SEL => {
                let queue_sel = val as u16;
                if (queue_sel as usize) < self.queues.lock().len() {
                    *self.queue_sel.lock() = queue_sel;
                }
            }
            VIRTIO_MMIO_QUEUE_NUM => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let _ = queue.set_size(val as u16);
                }
            }
            VIRTIO_MMIO_QUEUE_READY => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    queue.set_ready(val != 0);
                }
            }
            VIRTIO_MMIO_QUEUE_NOTIFY => {
                // Handle queue notification
                self.handle_queue_notify(val as u16);
            }
            VIRTIO_MMIO_INTERRUPT_ACK => {
                let mut interrupt_status = self.interrupt_status.lock();
                *interrupt_status &= !val;
            }
            VIRTIO_MMIO_STATUS => {
                self.handle_status_write(val);
            }
            VIRTIO_MMIO_QUEUE_DESC_LOW => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let high = (queue.desc_table_addr.as_usize() >> 32) as u32;
                    let addr = ((high as u64) << 32) | (val as u64);
                    let _ = queue.set_desc_table_addr(GuestPhysAddr::from(addr as usize));
                }
            }
            VIRTIO_MMIO_QUEUE_DESC_HIGH => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let low = queue.desc_table_addr.as_usize() as u32;
                    let addr = ((val as u64) << 32) | (low as u64);
                    let _ = queue.set_desc_table_addr(GuestPhysAddr::from(addr as usize));
                }
            }
            VIRTIO_MMIO_QUEUE_AVAIL_LOW => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let high = (queue.avail_ring_addr.as_usize() >> 32) as u32;
                    let addr = ((high as u64) << 32) | (val as u64);
                    let _ = queue.set_avail_ring_addr(GuestPhysAddr::from(addr as usize));
                }
            }
            VIRTIO_MMIO_QUEUE_AVAIL_HIGH => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let low = queue.avail_ring_addr.as_usize() as u32;
                    let addr = ((val as u64) << 32) | (low as u64);
                    let _ = queue.set_avail_ring_addr(GuestPhysAddr::from(addr as usize));
                }
            }
            VIRTIO_MMIO_QUEUE_USED_LOW => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let high = (queue.used_ring_addr.as_usize() >> 32) as u32;
                    let addr = ((high as u64) << 32) | (val as u64);
                    let _ = queue.set_used_ring_addr(GuestPhysAddr::from(addr as usize));
                }
            }
            VIRTIO_MMIO_QUEUE_USED_HIGH => {
                let queue_sel = *self.queue_sel.lock();
                let mut queues = self.queues.lock();
                if let Some(queue) = queues.get_mut(queue_sel as usize) {
                    let low = queue.used_ring_addr.as_usize() as u32;
                    let addr = ((val as u64) << 32) | (low as u64);
                    let _ = queue.set_used_ring_addr(GuestPhysAddr::from(addr as usize));
                }
            }
            _ => {
                return Err(VirtioError::InvalidRegister.into());
            }
        }

        Ok(())
    }

    /// Handle queue notification
    fn handle_queue_notify(&self, queue_index: u16) {
        trace!(
            "Queue {} notified for device {}",
            queue_index,
            self.config.device_index
        );

        // Check if device is ready
        if !self.is_device_ready() {
            warn!("Device not ready, ignoring queue notification");
            return;
        }

        // Get a copy of the queue to avoid holding the lock during processing
        let queue_copy = {
            let queues = self.queues.lock();
            match queues.get(queue_index as usize) {
                Some(q) if q.ready => q.clone(),
                Some(_) => {
                    warn!("Queue {} not ready", queue_index);
                    return;
                }
                None => {
                    warn!("Invalid queue index: {}", queue_index);
                    return;
                }
            }
        }; // Lock is released here

        // Check if queue addresses are set
        if queue_copy.desc_table_addr.as_usize() == 0
            || queue_copy.avail_ring_addr.as_usize() == 0
            || queue_copy.used_ring_addr.as_usize() == 0
        {
            warn!("Queue {} addresses not properly set", queue_index);
            return;
        }

        // Process available requests in the queue
        self.process_queue_requests(&queue_copy);
    }

    /// Process requests in the queue
    fn process_queue_requests(&self, queue: &VirtioQueue) {
        // Read the available ring index to see if there are new requests
        let avail_idx = match queue.read_avail_idx() {
            Ok(idx) => idx,
            Err(e) => {
                error!("Failed to read available index: {:?}", e);
                return;
            }
        };

        trace!(
            "Available index: {}, next_avail: {}",
            avail_idx,
            queue.next_avail
        );

        // Process new available descriptors
        let mut current_avail = queue.next_avail;
        let mut processed_requests = Vec::new();

        while current_avail != avail_idx {
            // Get descriptor index from available ring
            let ring_index = current_avail % queue.size;
            let desc_index = match queue.read_avail_entry(ring_index) {
                Ok(idx) => idx,
                Err(e) => {
                    error!(
                        "Failed to read available ring entry {}: {:?}",
                        ring_index,
                        e
                    );
                    current_avail = current_avail.wrapping_add(1);
                    continue;
                }
            };

            trace!(
                "Processing descriptor chain starting at index {}",
                desc_index
            );

            // Process the descriptor chain
            match self.process_descriptor_chain(queue, desc_index) {
                Ok(()) => {
                    // Request processed successfully, will be handled in process_descriptor_chain
                }
                Err(e) => {
                    error!("Failed to process descriptor chain {}: {:?}", desc_index, e);
                    // Store error request for later processing
                    processed_requests.push((desc_index, 0, 1)); // Status = 1 (error)
                }
            }

            current_avail = current_avail.wrapping_add(1);
        }

        // Update next_avail in the queue and handle any error requests
        if current_avail != queue.next_avail || !processed_requests.is_empty() {
            let processed_count = current_avail.wrapping_sub(queue.next_avail);
            trace!("Processed {} requests", processed_count);

            // Update the queue's next_avail index and handle error requests
            let mut queues = self.queues.lock();
            if let Some(queue_mut) = queues.get_mut(queue.index as usize) {
                queue_mut.update_last_avail_idx(current_avail);

                // Handle any error requests
                for (desc_index, len, _status) in processed_requests {
                    if let Err(e) = queue_mut.add_used(desc_index, len) {
                        error!("Failed to add used buffer for error request: {:?}", e);
                    }
                }
            }
        }
    }

    /// Process a descriptor chain
    fn process_descriptor_chain(&self, queue: &VirtioQueue, head_index: u16) -> AxResult<()> {
        // Parse the descriptor chain to extract the request
        let request = self.parse_virtio_request(queue, head_index)?;

        // Execute the request
        let status = self.execute_block_request(&request)?;

        // Add the completed request to the used ring
        self.add_used_buffer(queue, head_index, request.size() as u32, status);

        Ok(())
    }

    /// Parse VirtIO block request from descriptor chain
    fn parse_virtio_request(&self, queue: &VirtioQueue, head_index: u16) -> AxResult<BlockRequest> {
        // Validate the descriptor chain
        match queue.validate_virtio_block_chain(head_index, MIN_DESCRIPTOR_CHAIN_LENGTH) {
            Ok(true) => {}
            Ok(false) => {
                error!("Invalid VirtIO block descriptor chain");
                return Err(VirtioError::InvalidQueue.into());
            }
            Err(e) => {
                error!("Failed to validate descriptor chain: {:?}", e);
                return Err(VirtioError::InvalidQueue.into());
            }
        }

        // Parse the request header
        let header = match queue.parse_virtio_block_header(head_index) {
            Ok(header) => header,
            Err(e) => {
                error!("Failed to parse VirtIO block header: {:?}", e);
                return Err(VirtioError::InvalidQueue.into());
            }
        };

        // Get data buffers
        let buffers = match queue.get_data_buffers(head_index) {
            Ok(buffers) => buffers,
            Err(e) => {
                error!("Failed to get data buffers: {:?}", e);
                return Err(VirtioError::InvalidQueue.into());
            }
        };

        // Get status address
        let status_addr = match queue.get_status_addr(head_index) {
            Ok(addr) => addr,
            Err(e) => {
                error!("Failed to get status address: {:?}", e);
                return Err(VirtioError::InvalidQueue.into());
            }
        };

        // Create request object
        let request = BlockRequest::new_virtio(
            BlockRequestType::from(header.request_type),
            header.sector,
            buffers,
            status_addr,
        );

        Ok(request)
    }

    /// Execute a block request
    fn execute_block_request(&self, request: &BlockRequest) -> AxResult<u8> {
        // Execute the request using the backend
        let result = request.execute(self.backend.as_ref());

        Ok(result.status)
    }

    /// Add a used buffer to the used ring
    fn add_used_buffer(&self, queue: &VirtioQueue, desc_index: u16, len: u32, status: u8) {
        trace!(
            "Completing request: desc_index={}, len={}, status={}",
            desc_index,
            len,
            status
        );

        // Write the status byte to the status buffer first
        // This is typically the last descriptor in the chain
        if let Err(e) = queue.write_status_byte(desc_index, status) {
            error!("Failed to write status byte: {:?}", e);
            return;
        }

        // Get a mutable reference to the queue to update the used ring
        let mut queues = self.queues.lock();
        if let Some(queue_mut) = queues.get_mut(queue.index as usize) {
            // Add the used buffer to the used ring
            if let Err(e) = queue_mut.add_used(desc_index, len) {
                error!("Failed to add used buffer: {:?}", e);
                return;
            }

            // Check if we should notify the driver
            match queue_mut.should_notify() {
                Ok(should_notify) => {
                    if should_notify {
                        // Release the lock before triggering interrupt to avoid potential deadlock
                        drop(queues);
                        self.trigger_interrupt();
                    }
                }
                Err(e) => {
                    error!("Failed to check notification requirement: {:?}", e);
                }
            }
        } else {
            error!("Invalid queue index: {}", queue.index);
        }
    }



    /// Trigger an interrupt to notify the driver
    fn trigger_interrupt(&self) {
        // Set the used buffer notification bit
        let mut interrupt_status = self.interrupt_status.lock();
        *interrupt_status |= VIRTIO_MMIO_INT_VRING;

        trace!("Triggered interrupt for used buffer notification");

        // In a real implementation, this would trigger an actual interrupt
        // to the guest VM through the interrupt controller
    }

    /// Handle device status write
    fn handle_status_write(&self, status: u32) {
        let mut current_status = self.status.lock();

        // Handle device reset
        if status == 0 {
            *current_status = 0;
            self.reset_device();
            return;
        }

        // Update status
        *current_status = status;

        // Handle status transitions
        if (status & VIRTIO_STATUS_FAILED) != 0 {
            warn!("VirtIO device failed");
        }

        if (status & VIRTIO_STATUS_DRIVER_OK) != 0 {
            info!("VirtIO device driver OK");
        }
    }

    /// Reset the device
    fn reset_device(&self) {
        *self.driver_features.lock() = 0;
        *self.device_features_sel.lock() = 0;
        *self.driver_features_sel.lock() = 0;
        *self.queue_sel.lock() = 0;
        *self.interrupt_status.lock() = 0;
        *self.config_generation.lock() = 0;

        // Reset all queues
        let mut queues = self.queues.lock();
        for queue in queues.iter_mut() {
            queue.reset();
        }
    }

    /// Get the selected queue
    pub fn get_selected_queue(&self) -> Option<u16> {
        let queue_sel = *self.queue_sel.lock();
        let queue_count = self.queues.lock().len();
        if (queue_sel as usize) < queue_count {
            Some(queue_sel)
        } else {
            None
        }
    }

    /// Get queue by index
    pub fn get_queue(&self, index: u16) -> Option<VirtioQueue> {
        let queues = self.queues.lock();
        queues.get(index as usize).cloned()
    }

    /// Read from device configuration space
    fn read_config_space(&self, offset: u64, width: AccessWidth) -> AxResult<usize> {
        // Validate access width - config space typically uses 32-bit accesses
        MmioTransport::validate_access_width(width)?;

        // Read from block device configuration based on VirtIO specification layout
        let value = match offset {
            VIRTIO_BLK_CFG_CAPACITY_LOW => self.block_config.capacity as u32, // capacity (low 32 bits)
            VIRTIO_BLK_CFG_CAPACITY_HIGH => (self.block_config.capacity >> 32) as u32, // capacity (high 32 bits)
            VIRTIO_BLK_CFG_SIZE_MAX => self.block_config.size_max,                     // size_max
            VIRTIO_BLK_CFG_SEG_MAX => self.block_config.seg_max,                       // seg_max
            VIRTIO_BLK_CFG_GEOMETRY => {
                // Geometry: cylinders (16 bits) + heads (8 bits) + sectors (8 bits)
                (self.block_config.cylinders as u32)
                    | ((self.block_config.heads as u32) << 16)
                    | ((self.block_config.sectors as u32) << 24)
            }
            VIRTIO_BLK_CFG_BLK_SIZE => self.block_config.blk_size, // blk_size
            VIRTIO_BLK_CFG_PHYSICAL_BLOCK_EXP => self.block_config.physical_block_exp as u32, // physical_block_exp
            VIRTIO_BLK_CFG_ALIGNMENT_OFFSET => self.block_config.alignment_offset as u32, // alignment_offset
            VIRTIO_BLK_CFG_MIN_IO_SIZE => self.block_config.min_io_size as u32, // min_io_size
            VIRTIO_BLK_CFG_OPT_IO_SIZE => self.block_config.opt_io_size,        // opt_io_size
            _ => {
                // For unknown offsets in config space, return 0
                0
            }
        };

        Ok(value as usize)
    }
}

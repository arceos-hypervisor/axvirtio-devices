use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::vec;
use axaddrspace::{device::AccessWidth, GuestPhysAddr};
use axerrno::AxResult;
use log::{error, info, trace};
use spin::Mutex;

use axvirtio_common::{constants::*, mmio::MmioTransport, VirtioConfig, VirtioQueue, VirtioResult};

use crate::backend::{create_default_backend, ConsoleBackend};
use crate::console::config::VirtioConsoleConfig;
use crate::constants::*;

/// VirtIO Console MMIO device
pub struct VirtioConsoleDevice {
    /// Base IPA address
    pub(crate) base_ipa: GuestPhysAddr,
    /// MMIO region length
    pub(crate) length: usize,
    /// Device configuration
    config: VirtioConfig,
    /// Console device configuration
    console_config: Mutex<VirtioConsoleConfig>,
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
    /// VirtIO queues (RX, TX, and optionally control queues)
    queues: Mutex<Vec<VirtioQueue>>,
    /// Interrupt status
    interrupt_status: Mutex<u32>,
    /// Configuration generation
    config_generation: Mutex<u32>,
    /// Console backend
    backend: Box<dyn ConsoleBackend>,
}

impl VirtioConsoleDevice {
    /// Create a new VirtIO console device with device index
    pub fn new(base_ipa: usize, device_index: usize, length: usize) -> VirtioResult<Self> {
        info!(
            "Creating VirtIO console device: base_ipa={:#x}, device_index={}",
            base_ipa, device_index
        );
        let config = VirtioConfig::new_console_device(base_ipa, device_index);
        let mut queues = Vec::new();

        // Create RX and TX queues for port 0
        queues.push(VirtioQueue::new(
            VIRTIO_CONSOLE_RX_QUEUE,
            config.max_queue_size,
        ));
        queues.push(VirtioQueue::new(
            VIRTIO_CONSOLE_TX_QUEUE,
            config.max_queue_size,
        ));

        // Get the actual device MMIO address based on device_index
        let base_ipa = config.get_device_mmio_addr();

        // Create backend
        let backend = create_default_backend(device_index)?;

        // Create console configuration
        let (cols, rows) = backend.get_size();
        let console_config = VirtioConsoleConfig::with_size(cols, rows);

        Ok(Self {
            base_ipa,
            length,
            config,
            console_config: Mutex::new(console_config),
            status: Mutex::new(0),
            driver_features: Mutex::new(0),
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

    /// Handle MMIO read operations
    pub fn mmio_read(&self, addr: GuestPhysAddr, width: AccessWidth) -> AxResult<usize> {
        // Validate access and get offset
        let offset =
            match MmioTransport::validate_read_access(addr, width, self.base_ipa, self.length) {
                Ok(offset) => offset,
                Err(_) => return Ok(0),
            };

        if !self.is_address_in_range(addr) {
            return Ok(0);
        }

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
            _ if offset >= VIRTIO_MMIO_CONFIG => {
                // Configuration space access
                let config_offset = offset - VIRTIO_MMIO_CONFIG as usize;
                let console_config = self.console_config.lock();
                console_config.read_config(config_offset as u64, 4)
            }
            _ => 0,
        };

        trace!(
            "Console device {}: MMIO read at offset 0x{:x} = 0x{:x}",
            self.config.device_index,
            offset,
            value
        );

        Ok(value as usize)
    }

    /// Handle MMIO write operations
    pub fn mmio_write(&self, addr: GuestPhysAddr, width: AccessWidth, val: usize) -> AxResult<()> {
        // Validate access and get offset
        let offset =
            match MmioTransport::validate_write_access(addr, width, self.base_ipa, self.length) {
                Ok(offset) => offset,
                Err(_) => return Ok(()),
            };

        if !self.is_address_in_range(addr) {
            return Ok(());
        }

        trace!(
            "Console device {}: MMIO write at offset 0x{:x} = 0x{:x}",
            self.config.device_index,
            offset,
            val
        );

        match offset {
            VIRTIO_MMIO_DEVICE_FEATURES_SEL => {
                *self.device_features_sel.lock() = val as u32;
            }
            VIRTIO_MMIO_DRIVER_FEATURES => {
                let sel = *self.driver_features_sel.lock();
                let mut features = self.driver_features.lock();
                match sel {
                    0 => {
                        *features = (*features & 0xFFFFFFFF00000000) | (val as u64);
                    }
                    1 => {
                        *features = (*features & 0x00000000FFFFFFFF) | ((val as u64) << 32);
                    }
                    _ => {}
                }
            }
            VIRTIO_MMIO_DRIVER_FEATURES_SEL => {
                *self.driver_features_sel.lock() = val as u32;
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
                self.handle_queue_notify(val as u16);
            }
            VIRTIO_MMIO_INTERRUPT_ACK => {
                let mut interrupt_status = self.interrupt_status.lock();
                *interrupt_status &= !(val as u32);
            }
            VIRTIO_MMIO_STATUS => {
                *self.status.lock() = val as u32;
                if val == 0 {
                    self.reset_device();
                }
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
                    let low: u32 = queue.desc_table_addr.as_usize() as u32;
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
            _ if offset >= VIRTIO_MMIO_CONFIG => {
                // Configuration space access
                let config_offset = offset - VIRTIO_MMIO_CONFIG as usize;
                let mut console_config = self.console_config.lock();
                console_config.write_config(config_offset as u64, 4, val as u32);

                // Handle emergency write
                if config_offset == VIRTIO_CONSOLE_CFG_EMERG_WR as usize {
                    if let Err(e) = self.backend.emergency_write(val as u8) {
                        log::error!("Emergency write failed: {:?}", e);
                    }
                }
            }
            _ => {
                log::debug!(
                    "Console device {}: Unhandled MMIO write at offset 0x{:x}",
                    self.config.device_index,
                    offset
                );
            }
        }

        Ok(())
    }

    /// Handle queue notification
    fn handle_queue_notify(&self, queue_index: u16) {
        trace!(
            "Console device {}: Queue {} notification",
            self.config.device_index,
            queue_index
        );

        match queue_index {
            VIRTIO_CONSOLE_RX_QUEUE => {
                // Handle RX queue notification (guest providing receive buffers)
                self.handle_rx_queue();
            }
            VIRTIO_CONSOLE_TX_QUEUE => {
                // Handle TX queue notification (guest sending data)
                self.handle_tx_queue();
            }
            _ => {
                log::warn!(
                    "Console device {}: Unknown queue index {}",
                    self.config.device_index,
                    queue_index
                );
            }
        }
    }

    /// Handle RX queue (receive buffers from guest)
    fn handle_rx_queue(&self) {
        trace!(
            "Console device {}: Processing RX queue",
            self.config.device_index
        );

        // First, poll for new input from host console if using stdio backend
        #[cfg(feature = "stdio-backend")]
        {
            use crate::backend::StdioConsoleBackend;
            if let Some(stdio_backend) = self.backend.as_any().downcast_ref::<StdioConsoleBackend>() {
                stdio_backend.poll_host_input();
            }
        }

        // Check if device is ready
        if !self.is_device_ready() {
            log::warn!("Console device not ready, ignoring RX queue notification");
            return;
        }

        // Get a copy of the TX queue to avoid holding the lock during processing
        let queue_copy = {
            let queues = self.queues.lock();
            match queues.get(VIRTIO_CONSOLE_RX_QUEUE as usize) {
                Some(q) if q.ready => q.clone(),
                Some(_) => {
                    log::warn!("TX queue not ready");
                    return;
                }
                None => {
                    log::warn!("Invalid TX queue index");
                    return;
                }
            }
        }; // Lock is released here

        // Check if queue addresses are set
        if queue_copy.desc_table_addr.as_usize() == 0
            || queue_copy.avail_ring_addr.as_usize() == 0
            || queue_copy.used_ring_addr.as_usize() == 0
        {
            log::warn!("TX queue addresses not properly set");
            return;
        }

        // Process available requests in the queue
        self.process_rx_queue_requests(&queue_copy);
    }

    /// Handle TX queue (data to output from guest)
    fn handle_tx_queue(&self) {
        trace!(
            "Console device {}: Processing TX queue",
            self.config.device_index
        );

        // Check if device is ready
        if !self.is_device_ready() {
            log::warn!("Console device not ready, ignoring TX queue notification");
            return;
        }

        // Get a copy of the TX queue to avoid holding the lock during processing
        let queue_copy = {
            let queues = self.queues.lock();
            match queues.get(VIRTIO_CONSOLE_TX_QUEUE as usize) {
                Some(q) if q.ready => q.clone(),
                Some(_) => {
                    log::warn!("TX queue not ready");
                    return;
                }
                None => {
                    log::warn!("Invalid TX queue index");
                    return;
                }
            }
        }; // Lock is released here

        // Check if queue addresses are set
        if queue_copy.desc_table_addr.as_usize() == 0
            || queue_copy.avail_ring_addr.as_usize() == 0
            || queue_copy.used_ring_addr.as_usize() == 0
        {
            log::warn!("TX queue addresses not properly set");
            return;
        }

        // Process available requests in the queue
        self.process_tx_queue_requests(&queue_copy);
    }

    /// Process TX queue requests
    fn process_tx_queue_requests(&self, queue: &axvirtio_common::VirtioQueue) {
        use log::{error, trace};

        // Read the available ring index to see if there are new requests
        let avail_idx = match queue.read_avail_idx() {
            Ok(idx) => idx,
            Err(e) => {
                error!("Failed to read available index: {:?}", e);
                return;
            }
        };

        trace!(
            "TX queue available index: {}, next_avail: {}",
            avail_idx,
            queue.get_last_avail_idx()
        );

        // Process new available descriptors
        let mut current_avail = queue.get_last_avail_idx();
        let mut processed_requests = Vec::new();

        while current_avail != avail_idx {
            // Get descriptor index from available ring
            let ring_index = current_avail % queue.size;
            let desc_index = match queue.read_avail_entry(ring_index) {
                Ok(idx) => idx,
                Err(e) => {
                    error!(
                        "Failed to read available ring entry {}: {:?}",
                        ring_index, e
                    );
                    current_avail = current_avail.wrapping_add(1);
                    continue;
                }
            };

            trace!(
                "Processing TX descriptor chain starting at index {}",
                desc_index
            );

            // Process the descriptor chain
            match self.process_tx_descriptor_chain(queue, desc_index) {
                Ok(bytes_written) => {
                    // Store successful request for later processing
                    processed_requests.push((desc_index, bytes_written as u32, 0));
                    // Status = 0 (success)
                }
                Err(e) => {
                    error!(
                        "Failed to process TX descriptor chain {}: {:?}",
                        desc_index, e
                    );
                    // Store error request for later processing
                    processed_requests.push((desc_index, 0, 1)); // Status = 1 (error)
                }
            }

            current_avail = current_avail.wrapping_add(1);
        }

        // Update next_avail in the queue and handle processed requests
        if current_avail != queue.get_last_avail_idx() || !processed_requests.is_empty() {
            let processed_count = current_avail.wrapping_sub(queue.get_last_avail_idx());
            trace!("Processed {} TX requests", processed_count);

            // Update the queue's next_avail index and handle processed requests
            let mut queues = self.queues.lock();
            if let Some(queue_mut) = queues.get_mut(VIRTIO_CONSOLE_TX_QUEUE as usize) {
                queue_mut.update_last_avail_idx(current_avail);

                // Handle processed requests
                for (desc_index, len, status) in processed_requests {
                    self.add_used_tx_buffer(queue_mut, desc_index, len, status as u8);
                }
            }
        }
    }

    /// Process RX queue requests
    fn process_rx_queue_requests(&self, queue: &axvirtio_common::VirtioQueue) {
        use log::{error, trace, warn};
        let mut input_buffer = vec![0u8; 512];

        let bytes_read = match self.backend.read(&mut input_buffer) {
            Ok(0) => {
                trace!("No input data available from backend");
                return; // No data available
            }
            Ok(n) => {
                input_buffer.truncate(n);
                trace!("Read {} bytes from console backend", n);
                n
            }
            Err(e) => {
                error!("Failed to read from console backend: {:?}", e);
                return;
            }
        };

        // Read the available ring index to see if there are new requests
        let avail_idx = match queue.read_avail_idx() {
            Ok(idx) => idx,
            Err(e) => {
                error!("Failed to read available index: {:?}", e);
                return;
            }
        };

        trace!(
            "RX queue available index: {}, next_avail: {}",
            avail_idx,
            queue.get_last_avail_idx()
        );

        // Process available receive buffers and fill them with input data
        let mut current_avail = queue.get_last_avail_idx();
        let mut data_offset = 0;
        let mut processed_requests = Vec::new();

        while current_avail != avail_idx && data_offset < bytes_read {
            // Get descriptor index from available ring
            let ring_index = current_avail % queue.size;
            let desc_index = match queue.read_avail_entry(ring_index) {
                Ok(idx) => idx,
                Err(e) => {
                    error!(
                        "Failed to read available ring entry {}: {:?}",
                        ring_index, e
                    );
                    current_avail = current_avail.wrapping_add(1);
                    continue;
                }
            };

            trace!(
                "Processing RX descriptor chain starting at index {}",
                desc_index
            );

            // Process the descriptor chain and fill with input data
            match self.process_rx_descriptor_chain(
                queue,
                desc_index,
                &input_buffer[data_offset..bytes_read],
            ) {
                Ok(bytes_consumed) => {
                    // Store successful request for later processing
                    processed_requests.push((desc_index, bytes_consumed as u32, 0)); // Status = 0 (success)
                    data_offset += bytes_consumed;
                    trace!("Filled RX buffer with {} bytes", bytes_consumed);
                }
                Err(e) => {
                    error!(
                        "Failed to process RX descriptor chain {}: {:?}",
                        desc_index, e
                    );
                    // Store error request for later processing
                    processed_requests.push((desc_index, 0, 1)); // Status = 1 (error)
                }
            }

            current_avail = current_avail.wrapping_add(1);
        }

        // Update next_avail in the queue and handle processed requests
        if current_avail != queue.get_last_avail_idx() || !processed_requests.is_empty() {
            let processed_count = current_avail.wrapping_sub(queue.get_last_avail_idx());
            trace!("Processed {} RX requests", processed_count);

            // Update the queue's next_avail index and handle processed requests
            let mut queues = self.queues.lock();
            if let Some(queue_mut) = queues.get_mut(VIRTIO_CONSOLE_RX_QUEUE as usize) {
                queue_mut.update_last_avail_idx(current_avail);

                // Handle processed requests
                for (desc_index, len, status) in processed_requests {
                    self.add_used_rx_buffer(queue_mut, desc_index, len, status as u8);
                }
            }
        }

        // If we have remaining data, we might want to trigger another processing cycle
        if data_offset < bytes_read {
            warn!(
            "Still have {} bytes of input data remaining after processing all available RX buffers",
            bytes_read - data_offset
        );
            // In a real implementation, you might want to cache this data for the next RX notification
        }
    }

    /// Process a single TX descriptor chain
    fn process_tx_descriptor_chain(
        &self,
        queue: &axvirtio_common::VirtioQueue,
        head_index: u16,
    ) -> axerrno::AxResult<usize> {
        use log::{error, trace};

        // Get data buffers from the descriptor chain
        let buffers = match queue.get_data_buffers(head_index, self.config.device_type) {
            Ok(buffers) => buffers,
            Err(e) => {
                error!("Failed to get data buffers: {:?}", e);
                return Err(axvirtio_common::VirtioError::InvalidQueue.into());
            }
        };

        trace!("TX descriptor chain has {} data buffers", buffers.len());

        let mut total_bytes_written = 0;

        // Process each data buffer
        for (addr, size, is_write) in buffers {
            // For TX queue, buffers should be read-only (not write)
            if is_write {
                log::warn!("TX buffer should be read-only, but found write flag");
                continue;
            }

            // Read data from guest memory
            let mut buffer = Vec::with_capacity(size);
            buffer.resize(size, 0u8);

            if let Err(e) = axvirtio_common::memory::read_guest_buffer(addr, &mut buffer) {
                error!("Failed to read guest memory at {:?}: {:?}", addr, e);
                return Err(axvirtio_common::VirtioError::MemoryError.into());
            }

            // Send data to the backend for output
            match self.backend.write(&buffer) {
                Ok(bytes_written) => {
                    total_bytes_written += bytes_written;
                    trace!("Wrote {} bytes to console backend", bytes_written);
                }
                Err(e) => {
                    error!("Failed to write to console backend: {:?}", e);
                    return Err(axvirtio_common::VirtioError::BackendError.into());
                }
            }
        }

        // Flush the backend to ensure data is output
        if let Err(e) = self.backend.flush() {
            error!("Failed to flush console backend: {:?}", e);
            return Err(axvirtio_common::VirtioError::BackendError.into());
        }

        Ok(total_bytes_written)
    }

    /// Process a single RX descriptor chain and fill it with input data
    fn process_rx_descriptor_chain(
        &self,
        queue: &axvirtio_common::VirtioQueue,
        head_index: u16,
        input_data: &[u8],
    ) -> axerrno::AxResult<usize> {
        use log::{error, trace, warn};

        if input_data.is_empty() {
            return Ok(0);
        }

        // Get receive buffers from the descriptor chain
        let buffers = match queue.get_data_buffers(head_index, self.config.device_type) {
            Ok(buffers) => buffers,
            Err(e) => {
                error!("Failed to get data buffers: {:?}", e);
                return Err(axvirtio_common::VirtioError::InvalidQueue.into());
            }
        };

        trace!("RX descriptor chain has {} receive buffers", buffers.len());

        let mut total_bytes_written = 0;
        let mut data_offset = 0;

        // Process each receive buffer
        for (addr, size, is_write) in buffers {
            // For RX queue, buffers should be write-only (guest provides empty buffers to fill)
            if !is_write {
                warn!("RX buffer should be write-only, but found read flag");
                continue;
            }

            if data_offset >= input_data.len() {
                break; // No more input data to write
            }

            // Calculate how much data to copy to this buffer
            let remaining_input = input_data.len() - data_offset;
            let bytes_to_copy = size.min(remaining_input);

            if bytes_to_copy == 0 {
                break;
            }

            // Write data to guest memory
            let data_slice = &input_data[data_offset..data_offset + bytes_to_copy];
            if let Err(e) = axvirtio_common::memory::write_guest_buffer(addr, data_slice) {
                error!("Failed to write guest memory at {:?}: {:?}", addr, e);
                return Err(axvirtio_common::VirtioError::MemoryError.into());
            }

            total_bytes_written += bytes_to_copy;
            data_offset += bytes_to_copy;
            trace!("Wrote {} bytes to RX buffer at {:?}", bytes_to_copy, addr);

            // If this buffer is full and we still have data, continue to next buffer
            if bytes_to_copy == size && data_offset < input_data.len() {
                continue;
            } else {
                // This buffer is not full or we've consumed all input data
                break;
            }
        }

        trace!(
            "Total bytes written to RX descriptor chain: {}",
            total_bytes_written
        );
        Ok(total_bytes_written)
    }

    /// Add a used buffer to the TX used ring
    fn add_used_tx_buffer(
        &self,
        queue: &mut axvirtio_common::VirtioQueue,
        desc_index: u16,
        len: u32,
        status: u8,
    ) {
        use log::{error, trace};

        trace!(
            "Completing TX request: desc_index={}, len={}, status={}",
            desc_index,
            len,
            status
        );

        // Write the status byte to the status buffer if there's a status descriptor
        // Note: Console TX typically doesn't have a separate status descriptor like block devices
        // The status is conveyed through the used ring entry

        // Add the used buffer to the used ring
        if let Err(e) = queue.add_used(desc_index, len) {
            error!("Failed to add used buffer: {:?}", e);
            return;
        }

        // Check if we should notify the driver
        match queue.should_notify() {
            Ok(should_notify) => {
                if should_notify {
                    self.trigger_tx_interrupt();
                }
            }
            Err(e) => {
                error!("Failed to check notification requirement: {:?}", e);
            }
        }
    }

    /// Add a used buffer to the RX used ring
    fn add_used_rx_buffer(
        &self,
        queue: &mut axvirtio_common::VirtioQueue,
        desc_index: u16,
        len: u32,
        status: u8,
    ) {
        use log::{error, trace};

        trace!(
            "Completing RX request: desc_index={}, len={}, status={}",
            desc_index,
            len,
            status
        );

        // For console RX, the length indicates how much data was actually written to the buffer
        // Add the used buffer to the used ring
        if let Err(e) = queue.add_used(desc_index, len) {
            error!("Failed to add used buffer: {:?}", e);
            return;
        }

        // Check if we should notify the driver
        match queue.should_notify() {
            Ok(should_notify) => {
                if should_notify {
                    self.trigger_rx_interrupt();
                }
            }
            Err(e) => {
                error!("Failed to check notification requirement: {:?}", e);
            }
        }
    }

    /// Trigger an interrupt to notify the driver about TX completion
    fn trigger_tx_interrupt(&self) {
        use axvirtio_common::constants::*;

        // Set the used buffer notification bit
        {
            let mut interrupt_status = self.interrupt_status.lock();
            *interrupt_status |= VIRTIO_MMIO_INT_VRING;
        }

        // In a real implementation, this would trigger an actual interrupt
        // For now, we just log the event
        trace!(
            "Console device {}: TX interrupt triggered",
            self.config.device_index
        );
    }

    /// Trigger an interrupt to notify the driver about RX data availability
    fn trigger_rx_interrupt(&self) {
        use axvirtio_common::constants::*;

        // Set the used buffer notification bit
        {
            let mut interrupt_status = self.interrupt_status.lock();
            *interrupt_status |= VIRTIO_MMIO_INT_VRING;
        }

        // In a real implementation, this would trigger an actual interrupt
        // For now, we just log the event
        trace!(
            "Console device {}: RX interrupt triggered",
            self.config.device_index
        );
    }

    /// Check if the device is ready for operations
    fn is_device_ready(&self) -> bool {
        const DEVICE_READY_STATUS: u32 = axvirtio_common::constants::VIRTIO_STATUS_ACKNOWLEDGE
            | axvirtio_common::constants::VIRTIO_STATUS_DRIVER
            | axvirtio_common::constants::VIRTIO_STATUS_DRIVER_OK;

        let status = *self.status.lock();
        (status & DEVICE_READY_STATUS) == DEVICE_READY_STATUS
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

        // Reset backend
        if let Err(e) = self.backend.reset() {
            log::error!("Failed to reset console backend: {:?}", e);
        }
    }
}

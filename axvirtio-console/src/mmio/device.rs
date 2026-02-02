use alloc::{sync::Arc, vec::Vec};
use axaddrspace::GuestMemoryAccessor;
use axaddrspace::{GuestPhysAddr, device::AccessWidth};

use axvirtio_common::mmio::transport;
use spin::Mutex;

use crate::backend::ConsoleBackend;
use crate::console::config::VirtioConsoleConfig;
use crate::constants::*;
use axvirtio_common::{VirtioConfig, VirtioDeviceID, VirtioError, VirtioQueue, VirtioResult};

/// VirtIO MMIO Console Device
///
/// This is a complete VirtIO MMIO console device implementation that follows the VirtIO 1.1 specification.
/// The device provides a simple serial console interface for guest VMs.
///
/// # Architecture
/// ```text
/// Guest Driver <--MMIO--> VirtIO Device <--Backend--> Host Terminal
///      |                      |                           |
///   virtqueue              Ring Buffers            Console Backend
/// ```
///
/// # Queues
/// - receiveq (0): Data from host to guest
/// - transmitq (1): Data from guest to host
///
/// # Generic Parameters
/// - `B`: Console backend implementation that handles actual I/O operations
/// - `T`: Guest memory accessor with address translation capabilities
pub struct VirtioMmioConsoleDevice<B: ConsoleBackend, T: GuestMemoryAccessor + Clone> {
    /// Base IPA address
    base_ipa: GuestPhysAddr,
    /// MMIO region length
    length: usize,
    /// Device configuration
    config: VirtioConfig,
    /// Console-specific configuration
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
    /// VirtIO queues: [receiveq, transmitq]
    queues: Mutex<Vec<VirtioQueue<T>>>,
    /// Interrupt status
    interrupt_status: Mutex<u32>,
    /// Configuration generation
    config_generation: Mutex<u32>,
    /// Console backend for actual I/O
    backend: Arc<B>,
    /// Guest memory accessor
    accessor: Arc<T>,
}

impl<B: ConsoleBackend, T: GuestMemoryAccessor + Clone> VirtioMmioConsoleDevice<B, T> {
    /// Create a new VirtIO MMIO console device
    ///
    /// # Arguments
    /// * `base_ipa` - Base guest physical address of the MMIO region
    /// * `length` - Length of the MMIO region
    /// * `backend` - Console backend for I/O operations
    /// * `console_config` - Console configuration
    /// * `guest_memory` - Guest memory accessor for DMA operations
    pub fn new(
        base_ipa: GuestPhysAddr,
        length: usize,
        backend: B,
        console_config: VirtioConsoleConfig,
        guest_memory: T,
    ) -> VirtioResult<Self> {
        let accessor = Arc::new(guest_memory);
        let config = VirtioConfig::new(
            base_ipa,
            VIRTIO_CONSOLE_DEFAULT_FEATURES,
            VIRTIO_CONSOLE_QUEUE_COUNT_SINGLE as u16,
            VirtioDeviceID::Console,
        );

        // Create the two queues: receiveq and transmitq
        let mut queues = Vec::new();
        queues.push(VirtioQueue::new(
            VIRTIO_CONSOLE_RECEIVEQ,
            DEFAULT_CONSOLE_QUEUE_SIZE,
            accessor.clone(),
        ));
        queues.push(VirtioQueue::new(
            VIRTIO_CONSOLE_TRANSMITQ,
            DEFAULT_CONSOLE_QUEUE_SIZE,
            accessor.clone(),
        ));

        info!(
            "[VirtioConsole] Creating device at {:#x}, size={}x{}",
            base_ipa.as_usize(),
            console_config.cols,
            console_config.rows
        );

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
            backend: Arc::new(backend),
            accessor,
        })
    }

    /// Get the base IPA address
    pub fn base_ipa(&self) -> GuestPhysAddr {
        self.base_ipa
    }

    /// Get the MMIO region length
    pub fn length(&self) -> usize {
        self.length
    }

    /// Get the interrupt status
    pub fn get_interrupt_status(&self) -> u32 {
        *self.interrupt_status.lock()
    }

    /// Check if device is ready
    pub fn is_device_ready(&self) -> bool {
        let status = *self.status.lock();
        (status & VIRTIO_STATUS_DRIVER_OK) != 0
    }

    /// Handle MMIO read operation
    pub fn mmio_read(&self, addr: GuestPhysAddr, width: AccessWidth) -> VirtioResult<usize> {
        let offset = match transport::validate_read_access(addr, width, self.base_ipa, self.length)
        {
            Ok(offset) => offset,
            Err(_) => return Ok(0),
        };

        let value = match offset {
            VIRTIO_MMIO_MAGIC_VALUE => MMIO_MAGIC_VALUE,
            VIRTIO_MMIO_VERSION => MMIO_VERSION,
            VIRTIO_MMIO_DEVICE_ID => self.config.device_type.to_device_id(),
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
                    if queue.ready { 1 } else { 0 }
                } else {
                    0
                }
            }
            VIRTIO_MMIO_INTERRUPT_STATUS => {
                let status = *self.interrupt_status.lock();
                if status != 0 {
                    trace!("[VirtioConsole] INTERRUPT_STATUS read: {}", status);
                }
                status
            }
            VIRTIO_MMIO_STATUS => *self.status.lock(),
            VIRTIO_MMIO_CONFIG_GENERATION => *self.config_generation.lock(),
            _ => {
                // Check if this is a config space read (offset >= 0x100)
                if offset >= VIRTIO_MMIO_CONFIG_OFFSET {
                    self.read_config_space((offset - VIRTIO_MMIO_CONFIG_OFFSET) as u64, width)?
                        as u32
                } else {
                    return Err(VirtioError::InvalidRegister);
                }
            }
        };

        Ok(value as usize)
    }

    /// Handle MMIO write operation
    pub fn mmio_write(
        &self,
        addr: GuestPhysAddr,
        width: AccessWidth,
        val: usize,
    ) -> VirtioResult<()> {
        let offset = match transport::validate_write_access(addr, width, self.base_ipa, self.length)
        {
            Ok(offset) => offset,
            Err(_) => return Ok(()),
        };
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
                    _ => {}
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
                    if val != 0 {
                        info!("[VirtioConsole] Queue {} is now ready", queue_sel);
                    }
                }
            }
            VIRTIO_MMIO_QUEUE_NOTIFY => {
                self.handle_queue_notify(val as u16);
            }
            VIRTIO_MMIO_INTERRUPT_ACK => {
                trace!("[VirtioConsole] INTERRUPT_ACK write: {}", val);
                let mut interrupt_status = self.interrupt_status.lock();
                *interrupt_status &= !val;
                trace!(
                    "[VirtioConsole] After ACK: interrupt_status={}",
                    *interrupt_status
                );
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
                // Check if this is a config space write (offset >= 0x100)
                if offset >= VIRTIO_MMIO_CONFIG_OFFSET {
                    self.write_config_space(
                        (offset - VIRTIO_MMIO_CONFIG_OFFSET) as u64,
                        val as u64,
                        width,
                    )?;
                } else {
                    return Err(VirtioError::InvalidRegister);
                }
            }
        }

        Ok(())
    }

    /// Handle queue notification
    fn handle_queue_notify(&self, queue_index: u16) {
        trace!(
            "[VirtioConsole] handle_queue_notify: queue_index={}",
            queue_index
        );

        if !self.is_device_ready() {
            warn!("[VirtioConsole] Device not ready, ignoring queue notification");
            return;
        }

        match queue_index {
            VIRTIO_CONSOLE_RECEIVEQ => {
                // Receiveq: Guest has provided new receive buffers
                // Immediately poll for pending input and push to guest
                trace!("[VirtioConsole] Receiveq notified, no pending input");
            }
            VIRTIO_CONSOLE_TRANSMITQ => {
                // Transmitq: Guest has data to send
                trace!("[VirtioConsole] Transmitq notified, processing...");
                self.process_transmit_queue();
            }
            _ => {
                warn!("[VirtioConsole] Invalid queue index: {}", queue_index);
            }
        }
    }

    /// Forward pending input from the backend to the guest.
    ///
    /// Reads data from the backend (e.g., UART) and pushes it to the guest's
    /// VirtIO receiveq. This should be called when UART input is available.
    ///
    /// Returns the total number of bytes forwarded to the guest.
    pub fn forward_backend_input(&self) -> usize {
        if !self.is_device_ready() {
            return 0;
        }

        // Check if backend has pending input
        if !self.backend.has_pending_input() {
            return 0;
        }

        // Read data from backend in chunks
        let mut buffer = [0u8; 256];
        let mut total_forwarded = 0;

        loop {
            let bytes_read = match self.backend.read(&mut buffer) {
                Ok(n) if n > 0 => n,
                _ => break,
            };

            match self.push_input(&buffer[..bytes_read]) {
                Ok(written) if written > 0 => {
                    trace!(
                        "[VirtioConsole] forward_backend_input: {} bytes to guest",
                        written
                    );
                    total_forwarded += written;
                }
                Ok(_) => {
                    // No available buffers in guest
                    trace!("[VirtioConsole] forward_backend_input: no available buffers");
                    break;
                }
                Err(e) => {
                    error!("[VirtioConsole] forward_backend_input: failed: {:?}", e);
                    break;
                }
            }

            // Check if there's more pending input
            if !self.backend.has_pending_input() {
                break;
            }
        }

        total_forwarded
    }

    /// Process the transmit queue (guest to host)
    fn process_transmit_queue(&self) {
        // OPTIMIZATION: Process ALL available buffers in one batch, then trigger
        // a single interrupt at the end. This reduces interrupt overhead from
        // O(n) to O(1) for n buffers, which is critical for console performance.
        let mut processed_count = 0u32;
        let mut need_interrupt = false;

        loop {
            // Get a fresh copy of the queue state
            let queue_copy = {
                let queues = self.queues.lock();
                match queues.get(VIRTIO_CONSOLE_TRANSMITQ as usize) {
                    Some(q) if q.ready => q.clone(),
                    _ => {
                        warn!("[VirtioConsole] Transmitq not ready");
                        break;
                    }
                }
            };

            // Read current available ring index from guest memory
            let avail_idx = match queue_copy.read_avail_idx() {
                Ok(idx) => idx,
                Err(e) => {
                    error!("[VirtioConsole] Failed to read avail_idx: {:?}", e);
                    break;
                }
            };

            let current_avail = queue_copy.get_last_avail_idx();

            // If no new buffers, we're done
            if current_avail == avail_idx {
                break;
            }

            trace!(
                "[VirtioConsole] process_transmit_queue: avail_idx={}, last_avail={}",
                avail_idx, current_avail
            );

            let ring_index = current_avail % queue_copy.size;
            let desc_index = match queue_copy.read_avail_entry(ring_index) {
                Ok(idx) => idx,
                Err(e) => {
                    error!("[VirtioConsole] Failed to read avail entry: {:?}", e);
                    // Update last_avail to skip this broken entry
                    let mut queues = self.queues.lock();
                    if let Some(queue_mut) = queues.get_mut(VIRTIO_CONSOLE_TRANSMITQ as usize) {
                        queue_mut.update_last_avail_idx(current_avail.wrapping_add(1));
                    }
                    continue;
                }
            };

            trace!(
                "[VirtioConsole] Processing descriptor chain at index {}",
                desc_index
            );

            // Process the descriptor chain for console output
            let total_len = self.process_transmit_chain(&queue_copy, desc_index);
            trace!("[VirtioConsole] Processed {} bytes", total_len);

            // Update used ring (but DON'T trigger interrupt yet - we'll do it at the end)
            let mut queues = self.queues.lock();
            if let Some(queue_mut) = queues.get_mut(VIRTIO_CONSOLE_TRANSMITQ as usize) {
                queue_mut.update_last_avail_idx(current_avail.wrapping_add(1));

                // Now add to used ring (this updates used_idx which makes completion visible)
                if let Err(e) = queue_mut.add_used(desc_index, total_len) {
                    error!("[VirtioConsole] Failed to add used: {:?}", e);
                }

                processed_count += 1;

                // Check if we should notify - we'll do it once at the end
                if let Ok(should) = queue_mut.should_notify() {
                    if should {
                        need_interrupt = true;
                    }
                }
            }
        }

        // OPTIMIZATION: Trigger interrupt only ONCE after processing ALL buffers
        if processed_count > 0 && need_interrupt {
            trace!(
                "[VirtioConsole] Triggering single interrupt after processing {} buffers",
                processed_count
            );
            self.trigger_interrupt();
        }
    }

    /// Process a transmit descriptor chain
    fn process_transmit_chain(&self, queue: &VirtioQueue<T>, head_index: u16) -> u32 {
        let desc_table = match &queue.desc_table {
            Some(dt) => dt,
            None => {
                warn!("[VirtioConsole] No desc_table");
                return 0;
            }
        };

        let descriptors = match desc_table.follow_chain(head_index) {
            Ok(descs) => descs,
            Err(e) => {
                warn!("[VirtioConsole] Failed to follow chain: {:?}", e);
                return 0;
            }
        };

        trace!(
            "[VirtioConsole] process_transmit_chain: {} descriptors",
            descriptors.len()
        );
        let mut total_len = 0u32;

        for desc in &descriptors {
            if desc.is_write() {
                continue; // Skip write-only descriptors for transmit
            }

            // Read data from guest memory
            let mut buffer = alloc::vec![0u8; desc.len as usize];
            if let Some((host_addr, _)) = self.accessor.translate_and_get_limit(desc.guest_addr()) {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        host_addr.as_usize() as *const u8,
                        buffer.as_mut_ptr(),
                        desc.len as usize,
                    );
                }

                trace!("[VirtioConsole] Writing {} bytes to backend", buffer.len());
                // Write to backend (host terminal)
                match self.backend.write(&buffer) {
                    Ok(written) => {
                        total_len += written as u32;
                        trace!("[VirtioConsole] Backend wrote {} bytes", written);
                    }
                    Err(e) => {
                        warn!("[VirtioConsole] Backend write error: {:?}", e);
                    }
                }
            } else {
                warn!(
                    "[VirtioConsole] Failed to translate guest addr {:?}",
                    desc.guest_addr()
                );
            }
        }

        total_len
    }

    /// Trigger an interrupt
    fn trigger_interrupt(&self) {
        // Check if interrupts are disabled
        let console_config = self.console_config.lock();
        if console_config.disable_interrupts {
            trace!("[VirtioConsole] Interrupts disabled, skipping interrupt trigger");
            return;
        }
        drop(console_config);

        let mut interrupt_status = self.interrupt_status.lock();
        *interrupt_status |= VIRTIO_MMIO_INT_VRING;
        trace!("[VirtioConsole] Triggered interrupt");
    }

    /// Handle status write
    fn handle_status_write(&self, status: u32) {
        let mut current_status = self.status.lock();

        // Handle device reset
        if status == 0 {
            *current_status = 0;
            self.reset_device();
            return;
        }

        let mut new_status = status;

        // If driver sets FEATURES_OK for the first time, validate negotiated features
        if (new_status & VIRTIO_STATUS_FEATURES_OK) != 0
            && (*current_status & VIRTIO_STATUS_FEATURES_OK) == 0
        {
            let driver_feats = *self.driver_features.lock();
            if (driver_feats & !self.config.device_features) != 0 {
                new_status &= !VIRTIO_STATUS_FEATURES_OK;
                new_status |= VIRTIO_STATUS_FAILED;
            } else {
                let event_idx_enabled = (driver_feats & VIRTIO_F_RING_EVENT_IDX) != 0;
                info!(
                    "[VirtioConsole] FEATURES_OK: driver_feats={:#x}, event_idx={}",
                    driver_feats, event_idx_enabled
                );
                let mut queues = self.queues.lock();
                for queue in queues.iter_mut() {
                    queue.set_event_idx_enabled(event_idx_enabled);
                }
            }
        }

        *current_status = new_status;

        if (new_status & VIRTIO_STATUS_DRIVER_OK) != 0 {
            info!("[VirtioConsole] Device driver OK");
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

        let mut queues = self.queues.lock();
        for queue in queues.iter_mut() {
            queue.reset();
        }
    }

    /// Read from console configuration space
    fn read_config_space(&self, offset: u64, _width: AccessWidth) -> VirtioResult<usize> {
        let console_config = self.console_config.lock();

        let value = match offset {
            0x00 => console_config.cols as u32,  // cols
            0x02 => console_config.rows as u32,  // rows
            0x04 => console_config.max_nr_ports, // max_nr_ports
            0x08 => console_config.emerg_wr,     // emerg_wr
            _ => 0,
        };

        Ok(value as usize)
    }

    /// Write to console configuration space
    fn write_config_space(&self, offset: u64, value: u64, _width: AccessWidth) -> VirtioResult<()> {
        let mut console_config = self.console_config.lock();

        match offset {
            0x08 => {
                // Emergency write
                console_config.emerg_wr = value as u32;
                let ch = (value & 0xFF) as u8;
                let buf = [ch];
                let _ = self.backend.write(&buf);
            }
            _ => {}
        }

        Ok(())
    }

    /// Push data to the receive queue (host to guest)
    ///
    /// This can be called when there is input from the host terminal
    pub fn push_input(&self, data: &[u8]) -> VirtioResult<usize> {
        if !self.is_device_ready() || data.is_empty() {
            return Ok(0);
        }

        // Get a copy of the receiveq
        let queue_copy = {
            let queues = self.queues.lock();
            match queues.get(VIRTIO_CONSOLE_RECEIVEQ as usize) {
                Some(q) if q.ready => q.clone(),
                _ => return Ok(0),
            }
        };

        // Read available ring
        let avail_idx = queue_copy.read_avail_idx()?;
        let last_avail = queue_copy.get_last_avail_idx();

        if avail_idx == last_avail {
            return Ok(0); // No available buffers
        }

        let ring_index = last_avail % queue_copy.size;
        let desc_index = queue_copy.read_avail_entry(ring_index)?;

        // Write data to guest
        let written = self.write_to_receive_chain(&queue_copy, desc_index, data);

        if written > 0 {
            // Update used ring
            let mut queues = self.queues.lock();
            if let Some(queue_mut) = queues.get_mut(VIRTIO_CONSOLE_RECEIVEQ as usize) {
                queue_mut.update_last_avail_idx(last_avail.wrapping_add(1));
                if let Err(e) = queue_mut.add_used(desc_index, written as u32) {
                    error!("[VirtioConsole] Failed to add used: {:?}", e);
                }

                // Write avail_event to current guest avail_idx so guest kicks on next buffer
                let current_guest_avail_idx = queue_mut.read_avail_idx().unwrap_or(0);
                if let Err(e) = queue_mut.write_avail_event(current_guest_avail_idx) {
                    error!("[VirtioConsole] Failed to write avail_event: {:?}", e);
                }

                if queue_mut.should_notify().unwrap_or(false) {
                    drop(queues);
                    self.trigger_interrupt();
                }
            }
        }

        Ok(written)
    }

    /// Write data to receive chain
    fn write_to_receive_chain(
        &self,
        queue: &VirtioQueue<T>,
        head_index: u16,
        data: &[u8],
    ) -> usize {
        let desc_table = match &queue.desc_table {
            Some(dt) => dt,
            None => return 0,
        };

        let descriptors = match desc_table.follow_chain(head_index) {
            Ok(descs) => descs,
            Err(_) => return 0,
        };

        let mut written = 0;

        for desc in &descriptors {
            if !desc.is_write() {
                continue; // Skip read-only descriptors for receive
            }

            let to_write = core::cmp::min(desc.len as usize, data.len() - written);
            if to_write == 0 {
                break;
            }

            if let Some((host_addr, _)) = self.accessor.translate_and_get_limit(desc.guest_addr()) {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        data[written..].as_ptr(),
                        host_addr.as_usize() as *mut u8,
                        to_write,
                    );
                }
                written += to_write;
            }
        }

        written
    }
}

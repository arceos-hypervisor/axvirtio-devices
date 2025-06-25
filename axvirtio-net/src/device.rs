use alloc::boxed::Box;
use alloc::vec::Vec;
use axaddrspace::{device::AccessWidth, GuestPhysAddr};
use axerrno::AxResult;
use spin::Mutex;

use axvirtio_common::{constants::*, mmio::MmioTransport, VirtioConfig, VirtioQueue, VirtioResult};

use crate::backend::{create_default_backend, NetworkBackend};
use crate::constants::*;
use crate::net::config::VirtioNetConfig;

/// VirtIO Network MMIO device
pub struct VirtioNetDevice {
    /// Base IPA address
    pub(crate) base_ipa: GuestPhysAddr,
    /// MMIO region length
    pub(crate) length: usize,
    /// Device configuration
    config: VirtioConfig,
    /// Network device configuration
    net_config: VirtioNetConfig,
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
    /// VirtIO queues (RX, TX, and optionally control)
    queues: Mutex<Vec<VirtioQueue>>,
    /// Interrupt status
    interrupt_status: Mutex<u32>,
    /// Configuration generation
    config_generation: Mutex<u32>,
    /// Network backend
    #[allow(dead_code)]
    backend: Box<dyn NetworkBackend>,
}

impl VirtioNetDevice {
    /// Create a new VirtIO network device with device index
    pub fn new(device_index: usize) -> VirtioResult<Self> {
        let config = VirtioConfig::new_network_device(device_index);
        let mut queues = Vec::new();

        // Create RX and TX queues
        queues.push(VirtioQueue::new(VIRTIO_NET_RX_QUEUE, config.max_queue_size));
        queues.push(VirtioQueue::new(VIRTIO_NET_TX_QUEUE, config.max_queue_size));

        // Get the actual device MMIO address based on device_index
        let base_ipa = config.get_device_mmio_addr();
        let length = config.total_mmio_size;

        // Create backend
        let backend = create_default_backend(device_index)?;

        // Create network configuration with backend MAC address
        let mac_address = backend.get_mac_address();
        let net_config = VirtioNetConfig::new(mac_address);

        Ok(Self {
            base_ipa,
            length,
            config,
            net_config,
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
                self.net_config.read_config(config_offset as u64, 4)
            }
            _ => 0,
        };

        log::debug!(
            "Net device {}: MMIO read at offset 0x{:x} = 0x{:x}",
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

        log::debug!(
            "Net device {}: MMIO write at offset 0x{:x} = 0x{:x}",
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
            _ if offset >= VIRTIO_MMIO_CONFIG => {
                // Configuration space writes are typically read-only for network devices
                log::debug!(
                    "Net device {}: Ignoring config space write at offset 0x{:x}",
                    self.config.device_index,
                    offset
                );
            }
            _ => {
                log::debug!(
                    "Net device {}: Unhandled MMIO write at offset 0x{:x}",
                    self.config.device_index,
                    offset
                );
            }
        }

        Ok(())
    }

    /// Handle queue notification
    fn handle_queue_notify(&self, queue_index: u16) {
        log::debug!(
            "Net device {}: Queue {} notification",
            self.config.device_index,
            queue_index
        );

        match queue_index {
            VIRTIO_NET_RX_QUEUE => {
                // Handle RX queue notification (guest providing receive buffers)
                self.handle_rx_queue();
            }
            VIRTIO_NET_TX_QUEUE => {
                // Handle TX queue notification (guest sending packets)
                self.handle_tx_queue();
            }
            _ => {
                log::warn!(
                    "Net device {}: Unknown queue index {}",
                    self.config.device_index,
                    queue_index
                );
            }
        }
    }

    /// Handle RX queue (receive buffers from guest)
    fn handle_rx_queue(&self) {
        log::debug!(
            "Net device {}: Processing RX queue",
            self.config.device_index
        );
        // In a real implementation, this would:
        // 1. Check for available receive buffers in the RX queue
        // 2. Try to receive packets from the backend
        // 3. Copy received packets to guest buffers
        // 4. Update the used ring
    }

    /// Handle TX queue (packets to transmit from guest)
    fn handle_tx_queue(&self) {
        log::debug!(
            "Net device {}: Processing TX queue",
            self.config.device_index
        );
        // In a real implementation, this would:
        // 1. Read packets from the TX queue descriptors
        // 2. Parse VirtIO network headers
        // 3. Send packets via the backend
        // 4. Update the used ring with transmission status
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
}

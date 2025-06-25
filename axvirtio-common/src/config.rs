use crate::constants::*;
use axaddrspace::GuestPhysAddr;

/// Configuration for VirtIO devices with device index mapping
#[derive(Debug, Clone)]
pub struct VirtioConfig {
    /// Base MMIO address for the device
    pub base_addr: GuestPhysAddr,
    /// Size of the MMIO region per device
    pub mmio_size: usize,
    /// Total MMIO size for all devices
    pub total_mmio_size: usize,
    /// Device ID (varies by device type)
    pub device_id: u32,
    /// Vendor ID (0x1AF4 for Red Hat/QEMU)
    pub vendor_id: u32,
    /// Maximum queue size
    pub max_queue_size: u16,
    /// Number of queues supported
    pub num_queues: u16,
    /// Device features supported
    pub device_features: u64,
    /// Device index (0-31, determines MMIO address offset)
    pub device_index: usize,
}

impl VirtioConfig {
    /// Create a new VirtIO configuration with device index and device ID
    pub fn new(device_index: usize, device_id: u32, device_features: u64, num_queues: u16) -> Self {
        Self {
            base_addr: GuestPhysAddr::from(VIRTIO_MMIO_BASE),
            mmio_size: VIRTIO_MMIO_DEVICE_SIZE,
            total_mmio_size: VIRTIO_MMIO_TOTAL_SIZE,
            device_id,
            vendor_id: VIRTIO_VENDOR_ID,
            max_queue_size: DEFAULT_QUEUE_SIZE,
            num_queues,
            device_features,
            device_index,
        }
    }

    /// Create a new block device configuration
    pub fn new_block_device(device_index: usize) -> Self {
        // Block device specific features
        let features = VIRTIO_F_VERSION_1 | VIRTIO_F_RING_EVENT_IDX;
        Self::new(device_index, VIRTIO_DEVICE_ID_BLOCK, features, 1)
    }

    /// Create a new network device configuration
    pub fn new_network_device(device_index: usize) -> Self {
        // Network device specific features
        let features = VIRTIO_F_VERSION_1 | VIRTIO_F_RING_EVENT_IDX;
        Self::new(device_index, VIRTIO_DEVICE_ID_NET, features, 2) // RX and TX queues
    }

    /// Create a new console device configuration
    pub fn new_console_device(device_index: usize) -> Self {
        // Console device specific features
        let features = VIRTIO_F_VERSION_1;
        Self::new(device_index, VIRTIO_DEVICE_ID_CONSOLE, features, 2) // Input and output queues
    }

    /// Get the actual MMIO address for this device based on device_index
    pub fn get_device_mmio_addr(&self) -> GuestPhysAddr {
        let offset = self.device_index * VIRTIO_MMIO_DEVICE_SIZE;
        self.base_addr + offset
    }

    /// Get the MMIO range for this device
    pub fn get_mmio_range(&self) -> (GuestPhysAddr, GuestPhysAddr) {
        let start_addr = self.get_device_mmio_addr();
        let end_addr = start_addr + self.mmio_size;
        (start_addr, end_addr)
    }

    /// Check if device index is valid
    pub fn is_valid_device_index(&self) -> bool {
        self.device_index < VIRTIO_MAX_DEVICES
    }

    /// Get the device-specific file path for this device
    pub fn get_device_path(&self, prefix: &str, suffix: &str) -> alloc::string::String {
        alloc::format!("/guest/{}_{}.{}", prefix, self.device_index, suffix)
    }

    /// Get the disk file path for block devices
    pub fn get_disk_path(&self) -> alloc::string::String {
        self.get_device_path("vm", "img")
    }

    /// Get the network interface name for network devices
    pub fn get_network_interface(&self) -> alloc::string::String {
        alloc::format!("tap{}", self.device_index)
    }

    /// Get the console device path for console devices
    pub fn get_console_path(&self) -> alloc::string::String {
        self.get_device_path("console", "sock")
    }
}

impl Default for VirtioConfig {
    fn default() -> Self {
        Self::new_block_device(0)
    }
}

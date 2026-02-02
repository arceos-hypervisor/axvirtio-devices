//! Integration tests for VirtIO Console Device
//!
//! These tests verify the complete functionality of the VirtIO console device
//! including MMIO operations, queue handling, and data transmission.

use axaddrspace::{GuestMemoryAccessor, GuestPhysAddr, device::AccessWidth};
use axvirtio_common::VirtioDeviceID;
use axvirtio_console::{NullConsoleBackend, VirtioConsoleConfig, VirtioMmioConsoleDevice};

/// Mock guest memory accessor for testing
struct MockGuestMemoryAccessor {
    base: usize,
    size: usize,
}

impl MockGuestMemoryAccessor {
    fn new(base: usize, size: usize) -> Self {
        Self { base, size }
    }
}

impl Clone for MockGuestMemoryAccessor {
    fn clone(&self) -> Self {
        Self {
            base: self.base,
            size: self.size,
        }
    }
}

impl GuestMemoryAccessor for MockGuestMemoryAccessor {
    fn translate_and_get_limit(
        &self,
        addr: GuestPhysAddr,
    ) -> Option<(memory_addr::PhysAddr, usize)> {
        let offset = addr.as_usize();
        if offset >= self.base && offset < self.base + self.size {
            // Return a mock host address
            let host_addr = memory_addr::PhysAddr::from(offset - self.base + 0x10000);
            Some((host_addr, self.size - (offset - self.base)))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn create_test_device() -> VirtioMmioConsoleDevice<NullConsoleBackend, MockGuestMemoryAccessor>
    {
        let base_ipa = GuestPhysAddr::from(0x1000_0000usize);
        let length = 0x1000;
        let backend = NullConsoleBackend;
        let config = VirtioConsoleConfig {
            cols: 80,
            rows: 25,
            max_nr_ports: 1,
            emerg_wr: 0,
            disable_interrupts: false,
        };
        let accessor = MockGuestMemoryAccessor::new(0, 0x10000);

        VirtioMmioConsoleDevice::new(base_ipa, length, backend, config, accessor).unwrap()
    }

    #[test]
    fn test_device_creation() {
        let device = create_test_device();
        assert_eq!(device.base_ipa().as_usize(), 0x1000_0000);
        assert_eq!(device.length(), 0x1000);
        assert!(!device.is_device_ready());
    }

    #[test]
    fn test_mmio_read_magic_value() {
        let device = create_test_device();
        let addr = GuestPhysAddr::from(0x1000_0000usize); // Base + 0x000
        let result = device.mmio_read(addr, AccessWidth::Dword);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x7472_6976); // "virt"
    }

    #[test]
    fn test_mmio_read_version() {
        let device = create_test_device();
        let addr = GuestPhysAddr::from(0x1000_0004usize); // Base + 0x004
        let result = device.mmio_read(addr, AccessWidth::Dword);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2); // Modern version (2)
    }

    #[test]
    fn test_mmio_read_device_id() {
        let device = create_test_device();
        let addr = GuestPhysAddr::from(0x1000_0008usize); // Base + 0x008
        let result = device.mmio_read(addr, AccessWidth::Dword);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), VirtioDeviceID::Console.to_device_id() as usize);
    }

    #[test]
    fn test_device_initialization_sequence() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Step 1: ACKNOWLEDGE
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070), // STATUS register
            AccessWidth::Dword,
            0x01, // ACKNOWLEDGE
        );

        // Step 2: DRIVER
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070),
            AccessWidth::Dword,
            0x02, // DRIVER
        );

        // Step 3: Read device features
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x014), // DEVICE_FEATURES_SEL
            AccessWidth::Dword,
            0,
        );
        let features = device.mmio_read(
            GuestPhysAddr::from(base + 0x010), // DEVICE_FEATURES
            AccessWidth::Dword,
        );
        assert!(features.is_ok());

        // Step 4: Write driver features
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x020), // DRIVER_FEATURES_SEL
            AccessWidth::Dword,
            0,
        );
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x024), // DRIVER_FEATURES
            AccessWidth::Dword,
            features.unwrap() as usize,
        );

        // Step 5: FEATURES_OK
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070),
            AccessWidth::Dword,
            0x08, // FEATURES_OK
        );

        // Step 6: Verify FEATURES_OK is set
        let status = device.mmio_read(GuestPhysAddr::from(base + 0x070), AccessWidth::Dword);
        assert!(status.is_ok());
        assert_eq!(status.unwrap() & 0x08, 0x08);

        // Step 7: DRIVER_OK
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070),
            AccessWidth::Dword,
            0x04, // DRIVER_OK
        );

        // Verify device is ready
        assert!(device.is_device_ready());
    }

    #[test]
    fn test_queue_configuration() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Initialize device first
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070),
            AccessWidth::Dword,
            0x04, // DRIVER_OK
        );

        // Select queue 0 (receiveq)
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x030), // QUEUE_SEL
            AccessWidth::Dword,
            0,
        );

        // Set queue size
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x038), // QUEUE_NUM
            AccessWidth::Dword,
            8,
        );

        // Verify queue size
        let queue_num = device.mmio_read(GuestPhysAddr::from(base + 0x038), AccessWidth::Dword);
        assert!(queue_num.is_ok());
        assert_eq!(queue_num.unwrap(), 8);

        // Mark queue as ready
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x044), // QUEUE_READY
            AccessWidth::Dword,
            1,
        );

        // Verify queue is ready
        let queue_ready = device.mmio_read(GuestPhysAddr::from(base + 0x044), AccessWidth::Dword);
        assert!(queue_ready.is_ok());
        assert_eq!(queue_ready.unwrap(), 1);
    }

    #[test]
    fn test_device_reset() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Initialize device
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070),
            AccessWidth::Dword,
            0x04,
        );
        assert!(device.is_device_ready());

        // Reset device
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x070),
            AccessWidth::Dword,
            0, // Reset
        );

        // Verify device is not ready
        assert!(!device.is_device_ready());
    }

    #[test]
    fn test_config_space_read() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Read cols (offset 0x100)
        let cols = device.mmio_read(GuestPhysAddr::from(base + 0x100), AccessWidth::Dword);
        assert!(cols.is_ok());
        assert_eq!(cols.unwrap(), 80);

        // Read rows (offset 0x102)
        let rows = device.mmio_read(GuestPhysAddr::from(base + 0x102), AccessWidth::Dword);
        assert!(rows.is_ok());
        assert_eq!(rows.unwrap(), 25);
    }

    #[test]
    fn test_interrupt_status() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Initial interrupt status should be 0
        let status = device.mmio_read(GuestPhysAddr::from(base + 0x060), AccessWidth::Dword);
        assert!(status.is_ok());
        assert_eq!(status.unwrap(), 0);

        // Get interrupt status directly
        assert_eq!(device.get_interrupt_status(), 0);
    }

    #[test]
    fn test_invalid_register_access() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Try to write to an invalid offset (0x0fd is in the gap before config space)
        let result = device.mmio_write(
            GuestPhysAddr::from(base + 0x0fd), // Invalid offset
            AccessWidth::Dword,
            0xDEADBEEF,
        );

        // Should return an error
        assert!(result.is_err());
    }

    #[test]
    fn test_queue_notify_before_ready() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Try to notify queue before device is ready
        let _ = device.mmio_write(
            GuestPhysAddr::from(base + 0x050), // QUEUE_NOTIFY
            AccessWidth::Dword,
            1, // transmitq
        );

        // Should not crash, just ignore the notification
        assert!(!device.is_device_ready());
    }

    #[test]
    fn test_max_queue_size() {
        let device = create_test_device();
        let base = 0x1000_0000usize;

        // Read max queue size
        let max_size = device.mmio_read(
            GuestPhysAddr::from(base + 0x034), // QUEUE_NUM_MAX
            AccessWidth::Dword,
        );

        assert!(max_size.is_ok());
        assert!(max_size.unwrap() > 0);
    }
}

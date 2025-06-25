#[cfg(test)]
mod tests {
    use super::super::device::VirtioMmioDevice;
    use crate::queue::{
        AvailableRing, DescriptorTable, UsedRing, VirtioQueue, VirtqDesc, VirtqUsedElem,
    };
    use axaddrspace::{GuestPhysAddr, device::AccessWidth};

    #[test]
    fn test_config_space_read() {
        // Create a device with index 0
        let device = VirtioMmioDevice::new(0);

        // Base address for device 0 should be 0x0a00_0000
        let base_addr = GuestPhysAddr::from(0x0a00_0000);

        // Test reading capacity (low 32 bits) at offset 0x100
        let capacity_low_addr = base_addr + 0x100;
        let result = device.mmio_read(capacity_low_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let capacity_low = result.unwrap();

        // Test reading capacity (high 32 bits) at offset 0x104
        let capacity_high_addr = base_addr + 0x104;
        let result = device.mmio_read(capacity_high_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let capacity_high = result.unwrap();

        // Reconstruct the full capacity
        let full_capacity = (capacity_high as u64) << 32 | (capacity_low as u64);

        // Should match the default capacity from VirtioBlockConfig
        assert_eq!(full_capacity, 2048); // DEFAULT_CAPACITY_SECTORS

        // Test reading size_max at offset 0x108
        let size_max_addr = base_addr + 0x108;
        let result = device.mmio_read(size_max_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let size_max = result.unwrap();
        assert_eq!(size_max, 65536); // size_max

        // Test reading seg_max at offset 0x10c
        let seg_max_addr = base_addr + 0x10c;
        let result = device.mmio_read(seg_max_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let seg_max = result.unwrap();
        assert_eq!(seg_max, 128); // seg_max

        // Test reading geometry at offset 0x110
        let geometry_addr = base_addr + 0x110;
        let result = device.mmio_read(geometry_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let geometry = result.unwrap();
        // Default geometry should be 0 (no geometry info)
        assert_eq!(geometry, 0);

        // Test reading blk_size at offset 0x114
        let blk_size_addr = base_addr + 0x114;
        let result = device.mmio_read(blk_size_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let blk_size = result.unwrap();
        assert_eq!(blk_size, 512); // blk_size (SECTOR_SIZE)

        // Test reading physical_block_exp at offset 0x118
        let physical_block_exp_addr = base_addr + 0x118;
        let result = device.mmio_read(physical_block_exp_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let physical_block_exp = result.unwrap();
        assert_eq!(physical_block_exp, 0); // physical_block_exp
    }

    #[test]
    fn test_config_space_out_of_range() {
        let device = VirtioMmioDevice::new(0);
        let base_addr = GuestPhysAddr::from(0x0a00_0000);

        // Test reading beyond the known config space
        let unknown_addr = base_addr + 0x200; // Beyond config space
        let result = device.mmio_read(unknown_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value, 0); // Should return 0 for unknown offsets
    }

    #[test]
    fn test_device_index_addressing() {
        // Test device with index 1
        let device = VirtioMmioDevice::new(1);

        // Base address for device 1 should be 0x0a00_0200
        let base_addr = GuestPhysAddr::from(0x0a00_0200);

        // Test reading capacity at the correct address for device 1
        let capacity_low_addr = base_addr + 0x100;
        let result = device.mmio_read(capacity_low_addr, AccessWidth::Dword);
        assert!(result.is_ok());

        // Test that reading from device 0's address returns 0 (out of range)
        let wrong_addr = GuestPhysAddr::from(0x0a00_0000 + 0x100);
        let result = device.mmio_read(wrong_addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value, 0); // Should return 0 for addresses outside this device's range
    }

    #[test]
    fn test_disabled_device() {
        // Test device with invalid index (>= 32)
        let device = VirtioMmioDevice::new(32);

        // Any read should return 0 for disabled device
        let addr = GuestPhysAddr::from(0x0a00_0000 + 0x100);
        let result = device.mmio_read(addr, AccessWidth::Dword);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn test_used_ring_functionality() {
        // Test UsedRing creation and basic operations
        let base_addr = GuestPhysAddr::from(0x1000);
        let queue_size = 16;
        let used_ring = UsedRing::new(base_addr, queue_size);

        // Test initial state
        assert_eq!(used_ring.get_used_idx(), 0);
        assert!(used_ring.is_valid());
        assert_eq!(used_ring.header_addr(), base_addr);
        assert_eq!(
            used_ring.ring_addr(),
            base_addr + core::mem::size_of::<crate::queue::VirtqUsed>()
        );

        // Test address calculations
        let entry_addr = used_ring.ring_entry_addr(0);
        assert!(entry_addr.is_some());
        let entry_addr = used_ring.ring_entry_addr(queue_size);
        assert!(entry_addr.is_none()); // Out of bounds

        // Test total size calculation
        let expected_size = core::mem::size_of::<crate::queue::VirtqUsed>()
            + (queue_size as usize * core::mem::size_of::<VirtqUsedElem>())
            + 2;
        assert_eq!(used_ring.total_size(), expected_size);
    }

    #[test]
    fn test_virtio_queue_used_ring_integration() {
        // Test VirtioQueue integration with UsedRing
        let mut queue = VirtioQueue::new(0, 16);

        // Initially, used ring should be None
        assert!(queue.get_used_ring().is_none());

        // Set used ring address
        let used_addr = GuestPhysAddr::from(0x3000);
        queue.set_used_ring_addr(used_addr);

        // Now used ring should be initialized
        assert!(queue.get_used_ring().is_some());
        let used_ring = queue.get_used_ring().unwrap();
        assert_eq!(used_ring.base_addr, used_addr);
        assert_eq!(used_ring.size, 16);

        // Test reset clears used ring
        queue.reset();
        assert!(queue.get_used_ring().is_none());
    }

    #[test]
    fn test_virtq_used_elem() {
        // Test VirtqUsedElem creation and accessors
        let elem = VirtqUsedElem::new(42, 1024);
        assert_eq!(elem.id(), 42);
        assert_eq!(elem.len(), 1024);
    }

    #[test]
    fn test_descriptor_table_functionality() {
        // Test DescriptorTable creation and basic operations
        let base_addr = GuestPhysAddr::from(0x2000);
        let table_size = 16;
        let desc_table = DescriptorTable::new(base_addr, table_size);

        // Test initial state
        assert!(desc_table.is_valid());
        assert_eq!(desc_table.base_addr, base_addr);
        assert_eq!(desc_table.size, table_size);

        // Test address calculations
        let desc_addr = desc_table.desc_addr(0);
        assert!(desc_addr.is_some());
        assert_eq!(desc_addr.unwrap(), base_addr);

        let desc_addr = desc_table.desc_addr(1);
        assert!(desc_addr.is_some());
        assert_eq!(
            desc_addr.unwrap(),
            base_addr + core::mem::size_of::<VirtqDesc>()
        );

        // Test out of bounds
        let desc_addr = desc_table.desc_addr(table_size);
        assert!(desc_addr.is_none());

        // Test total size calculation
        let expected_size = table_size as usize * core::mem::size_of::<VirtqDesc>();
        assert_eq!(desc_table.total_size(), expected_size);
    }

    #[test]
    fn test_available_ring_functionality() {
        // Test AvailableRing creation and basic operations
        let base_addr = GuestPhysAddr::from(0x4000);
        let ring_size = 16;
        let avail_ring = AvailableRing::new(base_addr, ring_size);

        // Test initial state
        assert!(avail_ring.is_valid());
        assert_eq!(avail_ring.base_addr, base_addr);
        assert_eq!(avail_ring.size, ring_size);
        assert_eq!(avail_ring.last_avail_idx, 0);

        // Test address calculations
        assert_eq!(avail_ring.header_addr(), base_addr);
        assert_eq!(
            avail_ring.ring_addr(),
            base_addr + core::mem::size_of::<crate::queue::VirtqAvail>()
        );

        // Test ring entry addresses
        let entry_addr = avail_ring.ring_entry_addr(0);
        assert!(entry_addr.is_some());
        let entry_addr = avail_ring.ring_entry_addr(ring_size);
        assert!(entry_addr.is_none()); // Out of bounds

        // Test total size calculation
        let expected_size =
            core::mem::size_of::<crate::queue::VirtqAvail>() + (ring_size as usize * 2) + 2;
        assert_eq!(avail_ring.total_size(), expected_size);
    }

    #[test]
    fn test_virtio_queue_full_integration() {
        // Test VirtioQueue integration with all ring types
        let mut queue = VirtioQueue::new(0, 16);

        // Initially, all rings should be None
        assert!(queue.get_desc_table().is_none());
        assert!(queue.get_avail_ring().is_none());
        assert!(queue.get_used_ring().is_none());

        // Set descriptor table address
        let desc_addr = GuestPhysAddr::from(0x1000);
        queue.set_desc_table_addr(desc_addr);
        assert!(queue.get_desc_table().is_some());
        let desc_table = queue.get_desc_table().unwrap();
        assert_eq!(desc_table.base_addr, desc_addr);

        // Set available ring address
        let avail_addr = GuestPhysAddr::from(0x2000);
        queue.set_avail_ring_addr(avail_addr);
        assert!(queue.get_avail_ring().is_some());
        let avail_ring = queue.get_avail_ring().unwrap();
        assert_eq!(avail_ring.base_addr, avail_addr);

        // Set used ring address
        let used_addr = GuestPhysAddr::from(0x3000);
        queue.set_used_ring_addr(used_addr);
        assert!(queue.get_used_ring().is_some());
        let used_ring = queue.get_used_ring().unwrap();
        assert_eq!(used_ring.base_addr, used_addr);

        // Test reset clears all rings
        queue.reset();
        assert!(queue.get_desc_table().is_none());
        assert!(queue.get_avail_ring().is_none());
        assert!(queue.get_used_ring().is_none());
    }

    #[test]
    fn test_virtq_desc_flags() {
        // Test VirtqDesc flag methods
        let mut desc = VirtqDesc {
            addr: 0x1000,
            len: 512,
            flags: 0,
            next: 0,
        };

        // Test initial state
        assert!(!desc.has_next());
        assert!(!desc.is_write_only());
        assert!(!desc.is_indirect());

        // Test setting flags
        desc.set_next(true);
        assert!(desc.has_next());

        desc.set_write_only(true);
        assert!(desc.is_write_only());

        desc.set_indirect(true);
        assert!(desc.is_indirect());

        // Test clearing flags
        desc.set_next(false);
        assert!(!desc.has_next());

        desc.set_write_only(false);
        assert!(!desc.is_write_only());

        desc.set_indirect(false);
        assert!(!desc.is_indirect());
    }
}

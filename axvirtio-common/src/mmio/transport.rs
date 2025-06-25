use axaddrspace::{GuestPhysAddr, device::AccessWidth};
use axerrno::AxResult;

use crate::error::VirtioError;

/// MMIO transport layer utilities
pub struct MmioTransport;

impl MmioTransport {
    /// Validate MMIO access width
    pub fn validate_access_width(width: AccessWidth) -> AxResult<()> {
        // VirtIO MMIO requires 32-bit accesses for registers
        if width != AccessWidth::Dword {
            return Err(VirtioError::InvalidAccessWidth.into());
        }
        Ok(())
    }

    /// Calculate register offset from base address
    pub fn calculate_offset(addr: GuestPhysAddr, base_addr: GuestPhysAddr) -> usize {
        addr.as_usize() - base_addr.as_usize()
    }

    /// Check if address is within device range
    pub fn is_address_in_range(
        addr: GuestPhysAddr,
        base_addr: GuestPhysAddr,
        size: usize,
    ) -> bool {
        let offset = addr.as_usize().saturating_sub(base_addr.as_usize());
        offset < size
    }

    /// Validate MMIO read access
    pub fn validate_read_access(
        addr: GuestPhysAddr,
        width: AccessWidth,
        base_addr: GuestPhysAddr,
        size: usize,
    ) -> AxResult<usize> {
        // Check if address is in range
        if !Self::is_address_in_range(addr, base_addr, size) {
            return Ok(0); // Return 0 for out-of-range reads
        }

        // Validate access width for configuration registers
        let offset = Self::calculate_offset(addr, base_addr);
        if offset < 0x100 {
            // Configuration registers require 32-bit access
            Self::validate_access_width(width)?;
        }

        Ok(offset)
    }

    /// Validate MMIO write access
    pub fn validate_write_access(
        addr: GuestPhysAddr,
        width: AccessWidth,
        base_addr: GuestPhysAddr,
        size: usize,
    ) -> AxResult<usize> {
        // Check if address is in range
        if !Self::is_address_in_range(addr, base_addr, size) {
            return Ok(0); // Ignore out-of-range writes
        }

        // Validate access width for configuration registers
        let offset = Self::calculate_offset(addr, base_addr);
        if offset < 0x100 {
            // Configuration registers require 32-bit access
            Self::validate_access_width(width)?;
        }

        Ok(offset)
    }

    /// Convert value to bytes based on width
    pub fn value_to_bytes(val: usize, width: AccessWidth) -> [u8; 8] {
        let mut data = [0u8; 8];
        match width {
            AccessWidth::Byte => data[0] = val as u8,
            AccessWidth::Word => data[..2].copy_from_slice(&(val as u16).to_le_bytes()),
            AccessWidth::Dword => data[..4].copy_from_slice(&(val as u32).to_le_bytes()),
            AccessWidth::Qword => data[..8].copy_from_slice(&(val as u64).to_le_bytes()),
        }
        data
    }

    /// Convert bytes to value based on width
    pub fn bytes_to_value(data: &[u8], width: AccessWidth) -> usize {
        match width {
            AccessWidth::Byte => data[0] as usize,
            AccessWidth::Word => u16::from_le_bytes([data[0], data[1]]) as usize,
            AccessWidth::Dword => {
                u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize
            }
            AccessWidth::Qword => {
                u64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]) as usize
            }
        }
    }
}

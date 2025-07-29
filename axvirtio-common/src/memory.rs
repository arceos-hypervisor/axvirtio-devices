//! Unified guest memory access interface for VirtIO devices
//! 
//! This module provides a safe and consistent way to access guest memory
//! from VirtIO device implementations, handling address translation and
//! memory safety concerns.

use axaddrspace::GuestPhysAddr;
use memory_addr::PhysAddr;
use crate::error::{VirtioError, VirtioResult};

/// Trait for safe guest memory access operations
pub trait GuestMemoryAccess {
    /// Translate a guest physical address to host physical address
    fn translate_guest_to_host(&self, guest_addr: GuestPhysAddr) -> Option<PhysAddr>;
    
    /// Read a value of type T from guest memory
    fn read_obj<T: Copy>(&self, guest_addr: GuestPhysAddr) -> VirtioResult<T>;
    
    /// Write a value of type T to guest memory
    fn write_obj<T: Copy>(&self, guest_addr: GuestPhysAddr, val: T) -> VirtioResult<()>;
    
    /// Read a buffer from guest memory
    fn read_buffer(&self, guest_addr: GuestPhysAddr, buffer: &mut [u8]) -> VirtioResult<()>;
    
    /// Write a buffer to guest memory
    fn write_buffer(&self, guest_addr: GuestPhysAddr, buffer: &[u8]) -> VirtioResult<()>;
    
    /// Read a volatile value from guest memory (for device registers)
    fn read_volatile<T: Copy>(&self, guest_addr: GuestPhysAddr) -> VirtioResult<T>;
    
    /// Write a volatile value to guest memory (for device registers)
    fn write_volatile<T: Copy>(&self, guest_addr: GuestPhysAddr, val: T) -> VirtioResult<()>;
}

/// Default implementation of guest memory access using the current VM context
pub struct DefaultGuestMemoryAccess;

impl GuestMemoryAccess for DefaultGuestMemoryAccess {
    fn translate_guest_to_host(&self, _guest_addr: GuestPhysAddr) -> Option<PhysAddr> {
        // axvisor_api::guest_memory::translate_to_phys(
        //     axvisor_api::vmm::current_vm_id(),
        //     axvisor_api::vmm::current_vcpu_id(),
        //     guest_addr
        // )
        None
    }
    
    fn read_obj<T: Copy>(&self, guest_addr: GuestPhysAddr) -> VirtioResult<T> {
        let host_addr = self.translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;
        
        unsafe {
            let ptr = host_addr.as_usize() as *const T;
            Ok(core::ptr::read_volatile(ptr))
        }
    }
    
    fn write_obj<T: Copy>(&self, guest_addr: GuestPhysAddr, val: T) -> VirtioResult<()> {
        let host_addr = self.translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;
        
        unsafe {
            let ptr = host_addr.as_usize() as *mut T;
            core::ptr::write_volatile(ptr, val);
        }
        Ok(())
    }
    
    fn read_buffer(&self, guest_addr: GuestPhysAddr, buffer: &mut [u8]) -> VirtioResult<()> {
        let host_addr = self.translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;
        
        unsafe {
            let src_ptr = host_addr.as_usize() as *const u8;
            core::ptr::copy_nonoverlapping(src_ptr, buffer.as_mut_ptr(), buffer.len());
        }
        Ok(())
    }
    
    fn write_buffer(&self, guest_addr: GuestPhysAddr, buffer: &[u8]) -> VirtioResult<()> {
        let host_addr = self.translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;
        
        unsafe {
            let dst_ptr = host_addr.as_usize() as *mut u8;
            core::ptr::copy_nonoverlapping(buffer.as_ptr(), dst_ptr, buffer.len());
        }
        Ok(())
    }
    
    fn read_volatile<T: Copy>(&self, guest_addr: GuestPhysAddr) -> VirtioResult<T> {
        self.read_obj(guest_addr)
    }
    
    fn write_volatile<T: Copy>(&self, guest_addr: GuestPhysAddr, val: T) -> VirtioResult<()> {
        self.write_obj(guest_addr, val)
    }
}

/// Global instance for convenient access
pub static DEFAULT_GUEST_MEMORY: DefaultGuestMemoryAccess = DefaultGuestMemoryAccess;

/// Convenience functions for backward compatibility
pub fn read_guest_obj<T: Copy>(guest_addr: GuestPhysAddr) -> VirtioResult<T> {
    DEFAULT_GUEST_MEMORY.read_obj(guest_addr)
}

pub fn write_guest_obj<T: Copy>(guest_addr: GuestPhysAddr, val: T) -> VirtioResult<()> {
    DEFAULT_GUEST_MEMORY.write_obj(guest_addr, val)
}

pub fn read_guest_buffer(guest_addr: GuestPhysAddr, buffer: &mut [u8]) -> VirtioResult<()> {
    DEFAULT_GUEST_MEMORY.read_buffer(guest_addr, buffer)
}

pub fn write_guest_buffer(guest_addr: GuestPhysAddr, buffer: &[u8]) -> VirtioResult<()> {
    DEFAULT_GUEST_MEMORY.write_buffer(guest_addr, buffer)
}

/// Address validation utilities
pub mod validation {
    use super::*;
    
    /// Check if a guest address range is valid and doesn't overflow
    pub fn validate_guest_range(addr: GuestPhysAddr, len: usize) -> VirtioResult<()> {
        if len == 0 {
            return Ok(());
        }
        
        let end_addr = addr.as_usize().checked_add(len - 1)
            .ok_or(VirtioError::InvalidAddress)?;
        
        // Basic sanity check - ensure we don't wrap around
        if end_addr < addr.as_usize() {
            return Err(VirtioError::InvalidAddress);
        }
        
        Ok(())
    }
    
    /// Check if an address is properly aligned for type T
    pub fn check_alignment<T>(addr: GuestPhysAddr) -> VirtioResult<()> {
        let alignment = core::mem::align_of::<T>();
        if addr.as_usize() % alignment != 0 {
            return Err(VirtioError::InvalidAddress);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_address_validation() {
        use validation::*;
        
        // Test valid range
        let addr = GuestPhysAddr::from(0x1000);
        assert!(validate_guest_range(addr, 0x100).is_ok());
        
        // Test zero length
        assert!(validate_guest_range(addr, 0).is_ok());
        
        // Test alignment
        assert!(check_alignment::<u32>(GuestPhysAddr::from(0x1000)).is_ok());
        assert!(check_alignment::<u32>(GuestPhysAddr::from(0x1001)).is_err());
    }
}

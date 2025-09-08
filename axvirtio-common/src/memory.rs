//! Unified guest memory access interface for VirtIO devices
//!
//! This module provides a safe and consistent way to access guest memory
//! from VirtIO device implementations, handling address translation and
//! memory safety concerns.

use crate::error::{VirtioError, VirtioResult};
use axaddrspace::GuestPhysAddr;
use memory_addr::PhysAddr;

/// Trait for address translation
pub trait AddressTranslator {
    /// Translate a guest physical address to host physical address
    fn translate_guest_to_host(&self, guest_addr: GuestPhysAddr) -> Option<PhysAddr>;

    /// Read a value of type V from guest memory
    fn read_obj<V: Copy>(&self, guest_addr: GuestPhysAddr) -> VirtioResult<V> {
        let host_addr = self
            .translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;

        unsafe {
            let ptr = host_addr.as_usize() as *const V;
            Ok(core::ptr::read_volatile(ptr))
        }
    }

    /// Write a value of type V to guest memory
    fn write_obj<V: Copy>(&self, guest_addr: GuestPhysAddr, val: V) -> VirtioResult<()> {
        let host_addr = self
            .translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;

        unsafe {
            let ptr = host_addr.as_usize() as *mut V;
            core::ptr::write_volatile(ptr, val);
        }
        Ok(())
    }

    /// Read a buffer from guest memory
    fn read_buffer(&self, guest_addr: GuestPhysAddr, buffer: &mut [u8]) -> VirtioResult<()> {
        let host_addr = self
            .translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;

        unsafe {
            let src_ptr = host_addr.as_usize() as *const u8;
            core::ptr::copy_nonoverlapping(src_ptr, buffer.as_mut_ptr(), buffer.len());
        }
        Ok(())
    }

    /// Write a buffer to guest memory
    fn write_buffer(&self, guest_addr: GuestPhysAddr, buffer: &[u8]) -> VirtioResult<()> {
        let host_addr = self
            .translate_guest_to_host(guest_addr)
            .ok_or(VirtioError::InvalidAddress)?;

        unsafe {
            let dst_ptr = host_addr.as_usize() as *mut u8;
            core::ptr::copy_nonoverlapping(buffer.as_ptr(), dst_ptr, buffer.len());
        }
        Ok(())
    }

    /// Read a volatile value from guest memory (for device registers)
    fn read_volatile<V: Copy>(&self, guest_addr: GuestPhysAddr) -> VirtioResult<V> {
        self.read_obj(guest_addr)
    }

    /// Write a volatile value to guest memory (for device registers)
    fn write_volatile<V: Copy>(&self, guest_addr: GuestPhysAddr, val: V) -> VirtioResult<()> {
        self.write_obj(guest_addr, val)
    }
}

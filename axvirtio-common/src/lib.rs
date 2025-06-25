#![no_std]

extern crate alloc;

pub mod config;
pub mod constants;
pub mod error;
pub mod memory;
pub mod mmio;
pub mod queue;

use axaddrspace::GuestPhysAddr;
// Re-export commonly used types
pub use config::VirtioConfig;
pub use error::{VirtioError, VirtioResult};
use memory_addr::PhysAddr;
pub use memory::GuestMemoryAccess;
pub use mmio::transport::MmioTransport;
pub use queue::VirtioQueue;

// Re-export commonly used constants
pub use constants::*;

/// Legacy function for backward compatibility
/// Use GuestMemoryAccess for new code
pub fn translate_to_phys(addr: GuestPhysAddr) -> Option<PhysAddr> {
    axvisor_api::guest_memory::translate_to_phys(axvisor_api::vmm::current_vm_id(), axvisor_api::vmm::current_vcpu_id(), addr)
}

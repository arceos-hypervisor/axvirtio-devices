#![no_std]

extern crate alloc;

pub mod config;
pub mod constants;
mod device_type;
pub mod error;
pub mod memory;
pub mod mmio;
pub mod queue;

// Re-export commonly used types
pub use config::VirtioConfig;
pub use device_type::VirtioDeviceType;
pub use error::{VirtioError, VirtioResult};
pub use memory::GuestMemoryAccess;
pub use mmio::transport::MmioTransport;
pub use queue::VirtioQueue;

// Re-export commonly used constants
pub use constants::*;

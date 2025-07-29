#![no_std]

extern crate alloc;

pub mod config;
pub mod constants;
pub mod error;
pub mod memory;
pub mod mmio;
pub mod queue;
mod device_type;

// Re-export commonly used types
pub use config::VirtioConfig;
pub use error::{VirtioError, VirtioResult};
pub use memory::GuestMemoryAccess;
pub use mmio::transport::MmioTransport;
pub use queue::VirtioQueue;
pub use device_type::VirtioDeviceType;

// Re-export commonly used constants
pub use constants::*;

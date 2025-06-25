#![no_std]

extern crate alloc;

pub mod config;
pub mod constants;
pub mod error;
pub mod mmio;
pub mod queue;

// Re-export commonly used types
pub use config::VirtioConfig;
pub use error::{VirtioError, VirtioResult};
pub use mmio::transport::MmioTransport;
pub use queue::VirtioQueue;

// Re-export commonly used constants
pub use constants::*;

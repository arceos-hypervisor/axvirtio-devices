#![no_std]

#[macro_use]
extern crate alloc;

mod backend;
mod block;
mod constants;
mod mmio;

// Re-export from axvirtio-common
pub use axvirtio_common::{MmioTransport, VirtioConfig, VirtioError, VirtioQueue, VirtioResult};

// Re-export device-specific types
pub use backend::BlockBackend;
pub use mmio::VirtioMmioDevice;

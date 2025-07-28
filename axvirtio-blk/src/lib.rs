#![no_std]

#[macro_use]
extern crate alloc;

extern crate axstd as std;

mod backend;
mod block;
mod constants;
mod devops_impl;
mod mmio;

// Re-export from axvirtio-common
pub use axvirtio_common::{MmioTransport, VirtioConfig, VirtioError, VirtioQueue, VirtioResult};

// Re-export device-specific types
pub use mmio::VirtioMmioDevice;

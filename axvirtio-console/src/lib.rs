#![no_std]

extern crate alloc;

pub mod backend;
pub mod console;
pub mod constants;
pub mod device;
pub mod devops_impl;

// Re-export commonly used types
pub use axvirtio_common::{VirtioConfig, VirtioError, VirtioResult};
pub use console::config::VirtioConsoleConfig;
pub use device::VirtioConsoleDevice;

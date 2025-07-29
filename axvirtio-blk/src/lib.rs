//! # AxVirtIO Block Device Library
//!
//! This crate provides a VirtIO block device implementation for the AxVirtIO framework.
//! It includes MMIO transport, block device backend traits, and request handling
//! for VirtIO block devices according to the VirtIO specification.
//!
//! ## Features
//!
//! - VirtIO block device MMIO implementation
//! - Pluggable block backend support
//! - Guest memory access abstraction
//! - VirtIO queue management for block operations
//!
//! ## Usage
//!
//! ```rust,no_run
//! use axvirtio_blk::{VirtioMmioDevice, BlockBackend};
//!
//! // Implement your block backend
//! struct MyBlockBackend;
//! impl BlockBackend for MyBlockBackend {
//!     // ... implementation
//! }
//!
//! // Create and use the VirtIO block device
//! let backend = MyBlockBackend;
//! let device = VirtioMmioDevice::new(0x0a000000, 0x200, backend, memory_accessor)?;
//! ```

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

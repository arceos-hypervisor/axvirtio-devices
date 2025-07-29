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
//! use axvirtio_blk::{VirtioMmioDevice, BlockBackend, VirtioResult};
//! use axvirtio_common::AddressTranslator;
//! use axaddrspace::GuestPhysAddr;
//! use memory_addr::PhysAddr;
//!
//! // Implement your block backend
//! struct MyBlockBackend;
//! impl BlockBackend for MyBlockBackend {
//!     fn read(&self, _sector: u64, _buffer: &mut [u8]) -> VirtioResult<usize> {
//!         Ok(0)
//!     }
//!     fn write(&self, _sector: u64, _buffer: &[u8]) -> VirtioResult<usize> {
//!         Ok(0)
//!     }
//!     fn flush(&self) -> VirtioResult<()> {
//!         Ok(())
//!     }
//! }
//!
//! #[derive(Clone)]
//! struct MyTranslator;
//! impl AddressTranslator for MyTranslator {
//!     fn translate_guest_to_host(&self, guest_addr: GuestPhysAddr) -> Option<PhysAddr> {
//!         None
//!     }
//! }
//!
//! // Create and use the VirtIO block device
//! let backend = MyBlockBackend;
//! let translator = MyTranslator;
//! let device = VirtioMmioDevice::new(0x0a000000, 0x200, backend, translator);
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

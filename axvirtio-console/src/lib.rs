//! # AxVirtIO Console Device Library
//!
//! This crate provides a VirtIO console device implementation for the AxVirtIO framework.
//! It includes MMIO transport, console backend traits, and data handling
//! for VirtIO console devices according to the VirtIO specification.
//!
//! ## Features
//!
//! - VirtIO console device MMIO implementation
//! - Pluggable console backend support (read/write to host terminal)
//! - Guest memory access abstraction
//! - VirtIO queue management for console operations
//!
//! ## VirtIO Console Overview
//!
//! The VirtIO console device provides a simple serial console interface.
//! In single-port mode (default), it has two queues:
//! - receiveq (queue 0): Data from host to guest
//! - transmitq (queue 1): Data from guest to host
//!
//! ## Usage
//!
//! ```rust,no_run
//! use axvirtio_console::{VirtioMmioConsoleDevice, ConsoleBackend, VirtioConsoleConfig, VirtioResult};
//! use axaddrspace::GuestMemoryAccessor;
//! use axaddrspace::GuestPhysAddr;
//! use memory_addr::PhysAddr;
//!
//! // Implement your console backend
//! struct MyConsoleBackend;
//! impl ConsoleBackend for MyConsoleBackend {
//!     fn read(&self, buffer: &mut [u8]) -> VirtioResult<usize> {
//!         Ok(0)
//!     }
//!     fn write(&self, buffer: &[u8]) -> VirtioResult<usize> {
//!         // Output to host terminal
//!         Ok(buffer.len())
//!     }
//! }
//!
//! #[derive(Clone)]
//! struct MyTranslator;
//! impl GuestMemoryAccessor for MyTranslator {
//!     fn translate_and_get_limit(&self, guest_addr: GuestPhysAddr) -> Option<(PhysAddr, usize)> {
//!         None
//!     }
//! }
//!
//! // Create and use the VirtIO console device
//! let backend = MyConsoleBackend;
//! let translator = MyTranslator;
//! let console_config = VirtioConsoleConfig::default();
//! let device = VirtioMmioConsoleDevice::new(
//!     GuestPhysAddr::from(0x0a001000),
//!     0x200,
//!     backend,
//!     console_config,
//!     translator
//! );
//! ```

#![no_std]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate log;

mod backend;
mod console;
mod constants;
mod mmio;

// Re-export from axvirtio-common
pub use axvirtio_common::{VirtioConfig, VirtioError, VirtioQueue, VirtioResult};

// Re-export device-specific types
pub use backend::{ConsoleBackend, NullConsoleBackend};
pub use console::config::VirtioConsoleConfig;
pub use mmio::VirtioMmioConsoleDevice;

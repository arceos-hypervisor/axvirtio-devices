//! VirtIO Console Device Constants
//!
//! This module contains console device specific constants and re-exports
//! common VirtIO constants from axvirtio-common.

pub mod console;

// Re-export common VirtIO constants from axvirtio-common
pub use axvirtio_common::constants::*;

// Re-export console device specific constants
pub use console::*;

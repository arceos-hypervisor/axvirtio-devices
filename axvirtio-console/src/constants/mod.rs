//! VirtIO Console Device Constants
//!
//! This module contains constants specific to VirtIO console devices.

pub mod console;

// Re-export console constants
pub use console::*;

// Re-export common VirtIO constants
pub use axvirtio_common::constants::*;

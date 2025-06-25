//! VirtIO Network Device Constants
//!
//! This module contains constants specific to VirtIO network devices.

pub mod net;

// Re-export network constants
pub use net::*;

// Re-export common VirtIO constants
pub use axvirtio_common::constants::*;

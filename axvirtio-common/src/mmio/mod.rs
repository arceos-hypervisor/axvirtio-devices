/// MMIO transport layer for VirtIO devices
pub mod transport;

/// Re-export MmioTransport for convenience
pub use transport::MmioTransport;

#![no_std]

extern crate alloc;

pub mod backend;
pub mod constants;
pub mod device;
pub mod net;
pub mod packet;

// Re-export commonly used types
pub use axvirtio_common::{VirtioConfig, VirtioError, VirtioResult};
pub use device::VirtioNetDevice;
pub use net::config::VirtioNetConfig;
pub use packet::{NetPacket, PacketBuffer};

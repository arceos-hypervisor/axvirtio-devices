pub mod traits;

#[cfg(feature = "memory-backend")]
pub mod memory;

#[cfg(feature = "tap-backend")]
pub mod tap;

use alloc::boxed::Box;
pub use traits::NetworkBackend;

#[cfg(feature = "memory-backend")]
pub use memory::MemoryNetworkBackend;

#[cfg(feature = "tap-backend")]
pub use tap::TapNetworkBackend;

use axvirtio_common::VirtioResult;

/// Create a default network backend based on available features
pub fn create_default_backend(device_index: usize) -> VirtioResult<Box<dyn NetworkBackend>> {
    #[cfg(feature = "tap-backend")]
    {
        log::info!("Creating TAP network backend for device {}", device_index);
        let backend = TapNetworkBackend::new(device_index)?;
        return Ok(Box::new(backend));
    }

    #[cfg(feature = "memory-backend")]
    {
        log::info!(
            "Creating memory network backend for device {}",
            device_index
        );
        let backend = MemoryNetworkBackend::new(device_index);
        return Ok(Box::new(backend));
    }

    #[cfg(not(any(feature = "tap-backend", feature = "memory-backend")))]
    {
        log::warn!("No network backend features enabled, using memory backend");
        let backend = MemoryNetworkBackend::new(device_index);
        Ok(Box::new(backend))
    }
}

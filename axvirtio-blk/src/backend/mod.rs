#[cfg(feature = "file-backend")]
mod file;
#[cfg(feature = "memory-backend")]
mod memory;
mod traits;

use alloc::boxed::Box;
use axvirtio_common::VirtioResult;
 use crate::constants::DEFAULT_CAPACITY_SECTORS;

pub use traits::BlockBackend;

#[cfg(feature = "file-backend")]
pub use file::FileBackend;

#[cfg(feature = "memory-backend")]
pub use memory::MemoryBackend;

/// Create a default backend based on enabled features
pub fn create_default_backend(device_index: usize) -> VirtioResult<Box<dyn BlockBackend>> {
    #[cfg(feature = "file-backend")]
    {
        let disk_path = format!("/guest/vm_{}.img", device_index);
        let backend = FileBackend::new(disk_path, DEFAULT_CAPACITY_SECTORS, false)?; // 10MB default
        Ok(Box::new(backend))
    }

    #[cfg(all(feature = "memory-backend", not(feature = "file-backend")))]
    {
        let backend = MemoryBackend::new(DEFAULT_CAPACITY_SECTORS, false, device_index)?; // 512MB default
        Ok(Box::new(backend))
    }

    #[cfg(not(any(feature = "file-backend", feature = "memory-backend")))]
    {
        compile_error!(
            "At least one backend feature must be enabled: file-backend or memory-backend"
        );
    }
}

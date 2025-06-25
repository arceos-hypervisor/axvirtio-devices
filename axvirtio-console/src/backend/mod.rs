pub mod traits;

#[cfg(feature = "stdio-backend")]
pub mod stdio;

#[cfg(feature = "socket-backend")]
pub mod socket;

#[cfg(feature = "file-backend")]
pub mod file;

use alloc::boxed::Box;
pub use traits::ConsoleBackend;

#[cfg(feature = "stdio-backend")]
pub use stdio::StdioConsoleBackend;

#[cfg(feature = "socket-backend")]
pub use socket::SocketConsoleBackend;

#[cfg(feature = "file-backend")]
pub use file::FileConsoleBackend;

use axvirtio_common::VirtioResult;

/// Create a default console backend based on available features
pub fn create_default_backend(device_index: usize) -> VirtioResult<Box<dyn ConsoleBackend>> {
    #[cfg(feature = "stdio-backend")]
    {
        log::info!("Creating stdio console backend for device {}", device_index);
        let backend = StdioConsoleBackend::new(device_index);
        return Ok(Box::new(backend));
    }

    #[cfg(feature = "socket-backend")]
    {
        log::info!(
            "Creating socket console backend for device {}",
            device_index
        );
        let backend = SocketConsoleBackend::new(device_index)?;
        return Ok(Box::new(backend));
    }

    #[cfg(feature = "file-backend")]
    {
        log::info!("Creating file console backend for device {}", device_index);
        let backend = FileConsoleBackend::new(device_index)?;
        return Ok(Box::new(backend));
    }

    #[cfg(not(any(
        feature = "stdio-backend",
        feature = "socket-backend",
        feature = "file-backend"
    )))]
    {
        log::warn!("No console backend features enabled, using stdio backend");
        let backend = StdioConsoleBackend::new(device_index);
        Ok(Box::new(backend))
    }
}

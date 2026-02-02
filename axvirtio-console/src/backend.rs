use axvirtio_common::VirtioResult;

/// Trait for console device backends
///
/// This trait defines the interface for console backends that handle
/// actual I/O operations with the host system (terminal, serial port, etc.)
pub trait ConsoleBackend: Send + Sync {
    /// Read data from the console (host to guest)
    ///
    /// This is called when the guest wants to receive input.
    /// Returns the number of bytes actually read.
    ///
    /// # Arguments
    /// * `buffer` - Buffer to read data into
    ///
    /// # Returns
    /// Number of bytes read on success, or 0 if no data available
    fn read(&self, buffer: &mut [u8]) -> VirtioResult<usize>;

    /// Write data to the console (guest to host)
    ///
    /// This is called when the guest sends output.
    /// The backend should output this data to the host terminal.
    ///
    /// # Arguments
    /// * `buffer` - Buffer containing data to write
    ///
    /// # Returns
    /// Number of bytes written on success
    fn write(&self, buffer: &[u8]) -> VirtioResult<usize>;

    /// Check if there is data available to read
    ///
    /// Returns true if there is pending input data from the host
    fn has_pending_input(&self) -> bool {
        false
    }

    /// Get the console size (columns, rows)
    ///
    /// Returns the current terminal size, or default (80x25) if unknown
    fn get_size(&self) -> (u16, u16) {
        (80, 25)
    }
}

/// A simple backend that discards output and returns no input
///
/// Useful for testing or when no real console is needed
pub struct NullConsoleBackend;

impl ConsoleBackend for NullConsoleBackend {
    fn read(&self, _buffer: &mut [u8]) -> VirtioResult<usize> {
        Ok(0)
    }

    fn write(&self, buffer: &[u8]) -> VirtioResult<usize> {
        Ok(buffer.len())
    }
}

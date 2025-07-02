use axvirtio_common::VirtioResult;

/// Trait for console device backends
pub trait ConsoleBackend: Send + Sync {
    /// Get a reference to the backend as Any for downcasting
    fn as_any(&self) -> &dyn core::any::Any;
    /// Write data to the console output
    ///
    /// # Arguments
    /// * `data` - The data to write
    ///
    /// # Returns
    /// Number of bytes written on success
    fn write(&self, data: &[u8]) -> VirtioResult<usize>;

    /// Read data from the console input
    ///
    /// # Arguments
    /// * `buffer` - Buffer to store the read data
    ///
    /// # Returns
    /// Number of bytes read, or 0 if no data available
    fn read(&self, buffer: &mut [u8]) -> VirtioResult<usize>;

    /// Check if there is input data available
    ///
    /// # Returns
    /// True if input data is available
    fn has_input(&self) -> bool {
        false
    }

    /// Flush any pending output
    fn flush(&self) -> VirtioResult<()> {
        Ok(())
    }

    /// Get the console size (columns, rows)
    ///
    /// # Returns
    /// (columns, rows) tuple
    fn get_size(&self) -> (u16, u16) {
        (80, 24) // Default VT100 size
    }

    /// Set the console size
    ///
    /// # Arguments
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    fn set_size(&self, cols: u16, rows: u16) -> VirtioResult<()> {
        log::debug!("Console size change requested: {}x{}", cols, rows);
        Ok(())
    }

    /// Check if the console is ready for I/O
    ///
    /// # Returns
    /// True if the console is ready
    fn is_ready(&self) -> bool {
        true
    }

    /// Reset the console state
    fn reset(&self) -> VirtioResult<()> {
        Ok(())
    }

    /// Get console statistics
    ///
    /// # Returns
    /// (bytes_written, bytes_read)
    fn get_statistics(&self) -> (u64, u64) {
        (0, 0)
    }

    /// Reset console statistics
    fn reset_statistics(&self) -> VirtioResult<()> {
        Ok(())
    }

    /// Check if the backend supports emergency write
    ///
    /// # Returns
    /// True if emergency write is supported
    fn supports_emergency_write(&self) -> bool {
        false
    }

    /// Perform emergency write (bypasses normal queuing)
    ///
    /// # Arguments
    /// * `ch` - Character to write
    fn emergency_write(&self, ch: u8) -> VirtioResult<()> {
        if self.supports_emergency_write() {
            self.write(&[ch])?;
        }
        Ok(())
    }

    /// Check if the backend supports multiport
    ///
    /// # Returns
    /// True if multiport is supported
    fn supports_multiport(&self) -> bool {
        false
    }

    /// Get the maximum number of ports supported
    ///
    /// # Returns
    /// Maximum number of ports
    fn max_ports(&self) -> u32 {
        1
    }

    /// Open a specific port
    ///
    /// # Arguments
    /// * `port` - Port number to open
    fn open_port(&self, port: u32) -> VirtioResult<()> {
        if port == 0 {
            Ok(())
        } else {
            Err(axvirtio_common::VirtioError::NotSupported)
        }
    }

    /// Close a specific port
    ///
    /// # Arguments
    /// * `port` - Port number to close
    fn close_port(&self, port: u32) -> VirtioResult<()> {
        if port == 0 {
            Ok(())
        } else {
            Err(axvirtio_common::VirtioError::NotSupported)
        }
    }

    /// Check if a port is open
    ///
    /// # Arguments
    /// * `port` - Port number to check
    ///
    /// # Returns
    /// True if the port is open
    fn is_port_open(&self, port: u32) -> bool {
        port == 0 // Port 0 is always considered open
    }
}

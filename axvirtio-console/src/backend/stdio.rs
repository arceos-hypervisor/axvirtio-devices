use alloc::collections::VecDeque;
use axvirtio_common::VirtioResult;
use spin::Mutex;

use super::traits::ConsoleBackend;
use crate::constants::*;

/// Standard I/O console backend
pub struct StdioConsoleBackend {
    /// Device index
    device_index: usize,
    /// Console size
    size: Mutex<(u16, u16)>,
    /// Input buffer for simulation
    input_buffer: Mutex<VecDeque<u8>>,
    /// Statistics
    stats: Mutex<ConsoleStats>,
}

#[derive(Debug, Default)]
struct ConsoleStats {
    bytes_written: u64,
    bytes_read: u64,
}

impl StdioConsoleBackend {
    /// Create a new stdio console backend
    pub fn new(device_index: usize) -> Self {
        Self {
            device_index,
            size: Mutex::new((VIRTIO_CONSOLE_DEFAULT_COLS, VIRTIO_CONSOLE_DEFAULT_ROWS)),
            input_buffer: Mutex::new(VecDeque::new()),
            stats: Mutex::new(ConsoleStats::default()),
        }
    }

    /// Inject input data for testing
    pub fn inject_input(&self, data: &[u8]) {
        let mut buffer = self.input_buffer.lock();
        for &byte in data {
            buffer.push_back(byte);
        }
    }

    /// Clear input buffer
    pub fn clear_input(&self) {
        self.input_buffer.lock().clear();
    }

    /// Get input buffer length
    pub fn input_len(&self) -> usize {
        self.input_buffer.lock().len()
    }
}

impl ConsoleBackend for StdioConsoleBackend {
    fn write(&self, data: &[u8]) -> VirtioResult<usize> {
        // In a real implementation, this would write to stdout/stderr
        // For now, we just log the output
        if let Ok(text) = core::str::from_utf8(data) {
            log::info!("Console {}: {}", self.device_index, text.trim_end());
        } else {
            log::info!(
                "Console {}: Binary data ({} bytes): {:?}",
                self.device_index,
                data.len(),
                &data[..data.len().min(16)]
            );
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.bytes_written += data.len() as u64;
        }

        Ok(data.len())
    }

    fn read(&self, buffer: &mut [u8]) -> VirtioResult<usize> {
        let mut input_buffer = self.input_buffer.lock();
        let mut bytes_read = 0;

        for i in 0..buffer.len() {
            if let Some(byte) = input_buffer.pop_front() {
                buffer[i] = byte;
                bytes_read += 1;
            } else {
                break;
            }
        }

        if bytes_read > 0 {
            // Update statistics
            drop(input_buffer);
            let mut stats = self.stats.lock();
            stats.bytes_read += bytes_read as u64;

            log::debug!(
                "Console {}: Read {} bytes from input",
                self.device_index,
                bytes_read
            );
        }

        Ok(bytes_read)
    }

    fn has_input(&self) -> bool {
        !self.input_buffer.lock().is_empty()
    }

    fn flush(&self) -> VirtioResult<()> {
        // In a real implementation, this would flush stdout
        log::debug!("Console {}: Flush requested", self.device_index);
        Ok(())
    }

    fn get_size(&self) -> (u16, u16) {
        *self.size.lock()
    }

    fn set_size(&self, cols: u16, rows: u16) -> VirtioResult<()> {
        if cols == 0 || rows == 0 {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        *self.size.lock() = (cols, rows);
        log::info!(
            "Console {}: Size changed to {}x{}",
            self.device_index,
            cols,
            rows
        );

        Ok(())
    }

    fn is_ready(&self) -> bool {
        true // Stdio is always ready
    }

    fn reset(&self) -> VirtioResult<()> {
        self.clear_input();
        *self.size.lock() = (VIRTIO_CONSOLE_DEFAULT_COLS, VIRTIO_CONSOLE_DEFAULT_ROWS);
        log::info!("Console {}: Reset", self.device_index);
        Ok(())
    }

    fn get_statistics(&self) -> (u64, u64) {
        let stats = self.stats.lock();
        (stats.bytes_written, stats.bytes_read)
    }

    fn reset_statistics(&self) -> VirtioResult<()> {
        let mut stats = self.stats.lock();
        *stats = ConsoleStats::default();
        Ok(())
    }

    fn supports_emergency_write(&self) -> bool {
        true
    }

    fn emergency_write(&self, ch: u8) -> VirtioResult<()> {
        // Emergency write bypasses normal buffering
        log::warn!("Console {}: Emergency write: '{}'", self.device_index, ch as char);
        
        // In a real implementation, this would write directly to stderr
        // or use a special emergency output mechanism
        
        Ok(())
    }
}

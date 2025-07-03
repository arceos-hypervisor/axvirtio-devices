use alloc::collections::VecDeque;
use alloc::vec::Vec;
use axvirtio_common::VirtioResult;
use log::{error, info, trace};
use spin::Mutex;

use super::traits::ConsoleBackend;
use crate::constants::*;
use std::format;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

/// Maximum output buffer size in bytes (64KB)
const MAX_OUTPUT_BUFFER_SIZE: usize = 65536;
const MAX_INPUT_CACHE_BUFFER_SIZE: usize = 65536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
enum CmdType {
    AltF1,
    AltF2,
    AltF3,
    None,
}

/// Standard I/O console backend
pub struct StdioConsoleBackend {
    /// Device index
    device_index: usize,
    /// Console size
    size: Mutex<(u16, u16)>,
    /// Input buffer for simulation
    input_buffer: Mutex<VecDeque<u8>>,
    /// Output buffer for caching written data
    output_buffer: Mutex<Vec<u8>>,
    /// Input Cache for commands
    input_cache: Mutex<VecDeque<CmdType>>,
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
        let file_path = format!("/virtio_console_{}", device_index);
        // 新建一个文件, 如果文件存在，则删除再重新创建
        let _ = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path);
        let _ = OpenOptions::new().read(true).open(&file_path);
        Self {
            device_index,
            size: Mutex::new((VIRTIO_CONSOLE_DEFAULT_COLS, VIRTIO_CONSOLE_DEFAULT_ROWS)),
            input_buffer: Mutex::new(VecDeque::new()),
            output_buffer: Mutex::new(Vec::new()),
            input_cache: Mutex::new(VecDeque::new()),
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

    pub fn stdin_read(&self, buf: &mut [u8]) -> usize {
        if self.device_index != axvirtio_common::current_console() {
            return 0; // Skip reading for current console
        }
        let mut read_len = 0;
        while read_len < buf.len() {
            let len = super::pl011::read_bytes(buf);
            if len == 0 {
                break;
            }
            read_len += len;
        }
        read_len
    }

    pub fn stdout_write(&self, buf: &[u8]) -> usize {
        if self.device_index != axvirtio_common::current_console() {
            return buf.len();
        }
        super::pl011::write_bytes(buf);
        buf.len()
    }

    /// Poll host console for input and inject into buffer
    /// This should be called periodically by the hypervisor
    pub fn poll_host_input(&self) {
        // Read from host console (UART/platform console)
        let mut temp_buffer = [0u8; 256];

        // Try to read from stdin (should be non-blocking)
        // Use a timeout or try_lock to avoid deadlocks
        let bytes_read = self.stdin_read(&mut temp_buffer);

        // Only try to acquire lock if we have data to inject
        if bytes_read > 0 {
            // Try to acquire the lock with a timeout to avoid deadlock
            if let Some(mut buffer) = self.input_buffer.try_lock() {
                for &byte in &temp_buffer[..bytes_read] {
                    buffer.push_back(byte);
                }

                trace!(
                    "Console {}: Polled {} bytes from host console",
                    self.device_index,
                    bytes_read
                );
            } else {
                log::warn!(
                    "Console {}: Could not acquire input buffer lock, dropping {} bytes",
                    self.device_index,
                    bytes_read
                );
            }
        }
    }
}

impl ConsoleBackend for StdioConsoleBackend {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn write(&self, data: &[u8]) -> VirtioResult<usize> {
        // Cache the data in output buffer with FIFO overflow handling
        {
            let mut output_buffer = self.output_buffer.lock();

            // Check if we need to remove old data to make room
            if output_buffer.len() + data.len() > MAX_OUTPUT_BUFFER_SIZE {
                let excess = output_buffer.len() + data.len() - MAX_OUTPUT_BUFFER_SIZE;
                output_buffer.drain(..excess);
            }

            // Add new data to the buffer
            output_buffer.extend_from_slice(data);

            // 检查最后 6 个字节
            if output_buffer.len() >= 6 {
                let last_bytes = &output_buffer[output_buffer.len() - 6..];
                let should_truncate = if (last_bytes == b"^[1;3P" || last_bytes == b"^[1;9P") && 0 != axvirtio_common::current_console() {
                    // Alt+F1
                    info!("Console {}: Detected Alt+F1", self.device_index);
                    axvirtio_common::set_current(0);
                    true
                } else if (last_bytes == b"^[1;3Q" || last_bytes == b"^[1;9Q") && 1 != axvirtio_common::current_console() {
                    // Alt+F2
                    info!("Console {}: Detected Alt+F2", self.device_index);
                    axvirtio_common::set_current(1);
                    true
                } else if last_bytes == b"^[1;3R" || last_bytes == b"^[1;9R" {
                    // Alt+F3
                    info!("Console {}: Detected Alt+F3", self.device_index);
                    true
                } else {
                    false
                };
                
                if should_truncate {
                    // 删除末尾的 6 个字节
                    let new_len = output_buffer.len() - 6;
                    output_buffer.truncate(new_len);
                }
            }
        }

        // In a real implementation, this would write to stdout/stderr
        // For now, we just log the output
        if let Ok(text) = core::str::from_utf8(data) {
            self.stdout_write(text.as_bytes());
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.bytes_written += data.len() as u64;
        }

        Ok(data.len())
    }

    fn read(&self, buffer: &mut [u8]) -> VirtioResult<usize> {
        if axvirtio_common::get_status() != 0 && self.device_index == axvirtio_common::current_console() {
            error!("Console {}: Device ready for read", self.device_index);
            axvirtio_common::set_status(0);

            // clear screen
            self.stdout_write(b"\x1b[2J\x1b[H");
            self.stdout_write(self.output_buffer.lock().as_slice());
        }
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
            // drop(input_buffer);
            let mut stats = self.stats.lock();
            stats.bytes_read += bytes_read as u64;

            trace!(
                "Console {}: Read {} bytes from input: {:?}",
                self.device_index,
                bytes_read,
                &buffer[..bytes_read]
            );
            return Ok(bytes_read);
        }
        Ok(1)
    }

    fn has_input(&self) -> bool {
        !self.input_buffer.lock().is_empty()
    }

    fn flush(&self) -> VirtioResult<()> {
        // In a real implementation, this would flush stdout
        log::trace!("Console {}: Flush requested", self.device_index);
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
        log::warn!(
            "Console {}: Emergency write: '{}'",
            self.device_index,
            ch as char
        );

        // In a real implementation, this would write directly to stderr
        // or use a special emergency output mechanism

        Ok(())
    }
}

use crate::constants::*;

/// VirtIO Console Device Configuration
///
/// Layout according to VirtIO specification:
/// - cols (u16): Terminal width in columns
/// - rows (u16): Terminal height in rows
/// - max_nr_ports (u32): Maximum number of ports (multiport mode)
/// - emerg_wr (u32): Emergency write support
#[derive(Debug, Clone)]
pub struct VirtioConsoleConfig {
    /// Terminal width in columns (2 bytes at offset 0x00)
    pub cols: u16,
    /// Terminal height in rows (2 bytes at offset 0x02)
    pub rows: u16,
    /// Maximum number of ports (4 bytes at offset 0x04)
    /// Only meaningful if VIRTIO_CONSOLE_F_MULTIPORT is negotiated
    pub max_nr_ports: u32,
    /// Emergency write value (4 bytes at offset 0x08)
    /// Only meaningful if VIRTIO_CONSOLE_F_EMERG_WRITE is negotiated
    pub emerg_wr: u32,
    /// Disable interrupts for polling mode operation
    /// When true, the device will never trigger interrupts
    pub disable_interrupts: bool,
}

impl Default for VirtioConsoleConfig {
    fn default() -> Self {
        Self {
            cols: DEFAULT_CONSOLE_COLS,
            rows: DEFAULT_CONSOLE_ROWS,
            max_nr_ports: DEFAULT_MAX_NR_PORTS,
            emerg_wr: 0,
            disable_interrupts: false,
        }
    }
}

impl VirtioConsoleConfig {
    /// Create a new console configuration with specified size
    pub fn with_size(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            ..Default::default()
        }
    }

    /// Create a new console configuration with interrupts disabled
    /// This is useful for polling-mode operation where the driver
    /// will check the used ring directly instead of waiting for interrupts
    pub fn with_polling_mode(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            disable_interrupts: true,
            ..Default::default()
        }
    }

    /// Check if interrupts are disabled
    pub fn is_polling_mode(&self) -> bool {
        self.disable_interrupts
    }

    /// Read a configuration field by offset
    pub fn read(&self, offset: u64, width: u32) -> u64 {
        match offset {
            VIRTIO_CONSOLE_CFG_COLS => self.cols as u64,
            VIRTIO_CONSOLE_CFG_ROWS => self.rows as u64,
            VIRTIO_CONSOLE_CFG_MAX_NR_PORTS => self.max_nr_ports as u64,
            VIRTIO_CONSOLE_CFG_EMERG_WR => self.emerg_wr as u64,
            _ => {
                warn!(
                    "[VirtioConsole] Unknown config read: offset={:#x}, width={}",
                    offset, width
                );
                0
            }
        }
    }

    /// Write a configuration field by offset
    pub fn write(&mut self, offset: u64, value: u64, width: u32) {
        match offset {
            VIRTIO_CONSOLE_CFG_COLS => self.cols = value as u16,
            VIRTIO_CONSOLE_CFG_ROWS => self.rows = value as u16,
            VIRTIO_CONSOLE_CFG_MAX_NR_PORTS => self.max_nr_ports = value as u32,
            VIRTIO_CONSOLE_CFG_EMERG_WR => {
                // Emergency write: output the character immediately
                self.emerg_wr = value as u32;
                // The actual output should be handled by the device
            }
            _ => {
                warn!(
                    "[VirtioConsole] Unknown config write: offset={:#x}, value={:#x}, width={}",
                    offset, value, width
                );
            }
        }
    }

    /// Get the total size of the configuration space in bytes
    pub const fn size() -> usize {
        12 // cols(2) + rows(2) + max_nr_ports(4) + emerg_wr(4)
    }
}

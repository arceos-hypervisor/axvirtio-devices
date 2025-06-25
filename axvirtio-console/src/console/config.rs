use crate::constants::*;

/// VirtIO console device configuration space
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioConsoleConfig {
    /// Console columns (2 bytes)
    pub cols: u16,
    /// Console rows (2 bytes)
    pub rows: u16,
    /// Maximum number of ports (4 bytes)
    pub max_nr_ports: u32,
    /// Emergency write character (4 bytes)
    pub emerg_wr: u32,
}

impl VirtioConsoleConfig {
    /// Create a new console device configuration
    pub fn new() -> Self {
        Self {
            cols: VIRTIO_CONSOLE_DEFAULT_COLS,
            rows: VIRTIO_CONSOLE_DEFAULT_ROWS,
            max_nr_ports: VIRTIO_CONSOLE_DEFAULT_MAX_PORTS,
            emerg_wr: 0,
        }
    }

    /// Create a console configuration with specific size
    pub fn with_size(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            max_nr_ports: VIRTIO_CONSOLE_DEFAULT_MAX_PORTS,
            emerg_wr: 0,
        }
    }

    /// Create a multiport console configuration
    pub fn with_multiport(cols: u16, rows: u16, max_ports: u32) -> Self {
        Self {
            cols,
            rows,
            max_nr_ports: max_ports,
            emerg_wr: 0,
        }
    }

    /// Set console size
    pub fn set_size(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
    }

    /// Get console size
    pub fn get_size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Set maximum number of ports
    pub fn set_max_ports(&mut self, max_ports: u32) {
        self.max_nr_ports = max_ports;
    }

    /// Get maximum number of ports
    pub fn get_max_ports(&self) -> u32 {
        self.max_nr_ports
    }

    /// Set emergency write character
    pub fn set_emergency_write(&mut self, ch: u32) {
        self.emerg_wr = ch;
    }

    /// Get emergency write character
    pub fn get_emergency_write(&self) -> u32 {
        self.emerg_wr
    }

    /// Check if multiport is enabled
    pub fn is_multiport(&self) -> bool {
        self.max_nr_ports > 1
    }

    /// Get the configuration space as bytes
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        }
    }

    /// Get the configuration space size
    pub fn size() -> usize {
        core::mem::size_of::<Self>()
    }

    /// Read a field from the configuration space
    pub fn read_config(&self, offset: u64, width: usize) -> u32 {
        let bytes = self.as_bytes();
        let offset = offset as usize;

        if offset + width > bytes.len() {
            return 0;
        }

        match width {
            1 => bytes[offset] as u32,
            2 => u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as u32,
            4 => {
                if offset + 4 <= bytes.len() {
                    u32::from_le_bytes([
                        bytes[offset],
                        bytes[offset + 1],
                        bytes[offset + 2],
                        bytes[offset + 3],
                    ])
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    /// Write a field to the configuration space
    pub fn write_config(&mut self, offset: u64, width: usize, value: u32) {
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                self as *mut Self as *mut u8,
                core::mem::size_of::<Self>(),
            )
        };
        let offset = offset as usize;

        if offset + width > bytes.len() {
            return;
        }

        match offset {
            // Emergency write is the only writable field
            8 if width == 4 => {
                // VIRTIO_CONSOLE_CFG_EMERG_WR
                let value_bytes = value.to_le_bytes();
                bytes[offset..offset + 4].copy_from_slice(&value_bytes);
            }
            _ => {
                // Other fields are read-only
                log::debug!("Ignoring write to read-only console config offset 0x{:x}", offset);
            }
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> bool {
        self.cols > 0 
            && self.rows > 0 
            && self.max_nr_ports > 0 
            && self.max_nr_ports <= VIRTIO_CONSOLE_MAX_PORTS
    }
}

impl Default for VirtioConsoleConfig {
    fn default() -> Self {
        Self::new()
    }
}

use crate::constants::*;

/// VirtIO network device configuration space
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioNetConfig {
    /// MAC address (6 bytes)
    pub mac: [u8; MAC_ADDRESS_SIZE],
    /// Status field (2 bytes)
    pub status: u16,
    /// Maximum number of virtqueue pairs (2 bytes)
    pub max_virtqueue_pairs: u16,
    /// Maximum MTU (2 bytes)
    pub mtu: u16,
}

impl VirtioNetConfig {
    /// Create a new network device configuration
    pub fn new(mac: [u8; MAC_ADDRESS_SIZE]) -> Self {
        Self {
            mac,
            status: VIRTIO_NET_S_LINK_UP,
            max_virtqueue_pairs: VIRTIO_NET_DEFAULT_QUEUE_PAIRS,
            mtu: VIRTIO_NET_DEFAULT_MTU,
        }
    }

    /// Check if link is up
    pub fn is_link_up(&self) -> bool {
        (self.status & VIRTIO_NET_S_LINK_UP) != 0
    }

    /// Set link status
    pub fn set_link_up(&mut self, up: bool) {
        if up {
            self.status |= VIRTIO_NET_S_LINK_UP;
        } else {
            self.status &= !VIRTIO_NET_S_LINK_UP;
        }
    }

    /// Check if announce is set
    pub fn should_announce(&self) -> bool {
        (self.status & VIRTIO_NET_S_ANNOUNCE) != 0
    }

    /// Set announce flag
    pub fn set_announce(&mut self, announce: bool) {
        if announce {
            self.status |= VIRTIO_NET_S_ANNOUNCE;
        } else {
            self.status &= !VIRTIO_NET_S_ANNOUNCE;
        }
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

        match width {
            1 => bytes[offset] = value as u8,
            2 => {
                let value_bytes = (value as u16).to_le_bytes();
                bytes[offset..offset + 2].copy_from_slice(&value_bytes);
            }
            4 => {
                if offset + 4 <= bytes.len() {
                    let value_bytes = value.to_le_bytes();
                    bytes[offset..offset + 4].copy_from_slice(&value_bytes);
                }
            }
            _ => {}
        }
    }
}

impl Default for VirtioNetConfig {
    fn default() -> Self {
        Self::new([0x52, 0x54, 0x00, 0x12, 0x34, 0x56])
    }
}

use alloc::vec::Vec;
use axvirtio_common::VirtioResult;
use crate::constants::*;

/// VirtIO network packet header
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VirtioNetHeader {
    /// Flags
    pub flags: u8,
    /// GSO type
    pub gso_type: u8,
    /// Header length
    pub hdr_len: u16,
    /// GSO size
    pub gso_size: u16,
    /// Checksum start
    pub csum_start: u16,
    /// Checksum offset
    pub csum_offset: u16,
}

impl VirtioNetHeader {
    /// Create a new network header
    pub fn new() -> Self {
        Self {
            flags: 0,
            gso_type: VIRTIO_NET_HDR_GSO_NONE,
            hdr_len: VIRTIO_NET_HDR_SIZE as u16,
            gso_size: 0,
            csum_start: 0,
            csum_offset: 0,
        }
    }

    /// Check if checksum is needed
    pub fn needs_checksum(&self) -> bool {
        (self.flags & VIRTIO_NET_HDR_F_NEEDS_CSUM) != 0
    }

    /// Set checksum needed flag
    pub fn set_needs_checksum(&mut self, needs: bool) {
        if needs {
            self.flags |= VIRTIO_NET_HDR_F_NEEDS_CSUM;
        } else {
            self.flags &= !VIRTIO_NET_HDR_F_NEEDS_CSUM;
        }
    }

    /// Check if data is valid
    pub fn data_valid(&self) -> bool {
        (self.flags & VIRTIO_NET_HDR_F_DATA_VALID) != 0
    }

    /// Set data valid flag
    pub fn set_data_valid(&mut self, valid: bool) {
        if valid {
            self.flags |= VIRTIO_NET_HDR_F_DATA_VALID;
        } else {
            self.flags &= !VIRTIO_NET_HDR_F_DATA_VALID;
        }
    }

    /// Convert to bytes
    pub fn as_bytes(&self) -> &[u8] {
        // Safe because VirtioNetHeader is a simple POD struct with repr(C)
        unsafe {
            core::slice::from_raw_parts(
                self as *const Self as *const u8,
                core::mem::size_of::<Self>(),
            )
        }
    }

    /// Create from bytes
    pub fn from_bytes(bytes: &[u8]) -> VirtioResult<Self> {
        if bytes.len() < core::mem::size_of::<Self>() {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        // SAFETY: This is safe because:
        // 1. VirtioNetHeader is a simple POD struct with repr(C) and no padding
        // 2. We've verified the buffer size is sufficient
        // 3. This is NOT a guest address translation - we're reading from host memory
        // 4. read_unaligned handles potential alignment issues in network packets
        // 5. The lifetime of the pointer is limited to this single read operation
        unsafe {
            let header_ptr = bytes.as_ptr() as *const Self;
            Ok(core::ptr::read_unaligned(header_ptr))
        }
    }
}

impl Default for VirtioNetHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Network packet buffer
pub struct PacketBuffer {
    /// Packet data including header
    data: Vec<u8>,
}

impl PacketBuffer {
    /// Create a new packet buffer
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
        }
    }

    /// Create a packet buffer with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Create a packet buffer from data
    pub fn from_data(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Get the packet data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable packet data
    pub fn data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    /// Get the packet length
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if packet is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Clear the packet buffer
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Reserve space for additional data
    pub fn reserve(&mut self, additional: usize) {
        self.data.reserve(additional);
    }

    /// Extend the packet with data
    pub fn extend_from_slice(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    /// Get the VirtIO header if present
    pub fn get_virtio_header(&self) -> VirtioResult<VirtioNetHeader> {
        if self.data.len() < VIRTIO_NET_HDR_SIZE {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        VirtioNetHeader::from_bytes(&self.data[..VIRTIO_NET_HDR_SIZE])
    }

    /// Set the VirtIO header
    pub fn set_virtio_header(&mut self, header: &VirtioNetHeader) {
        if self.data.len() < VIRTIO_NET_HDR_SIZE {
            self.data.resize(VIRTIO_NET_HDR_SIZE, 0);
        }

        self.data[..VIRTIO_NET_HDR_SIZE].copy_from_slice(header.as_bytes());
    }

    /// Get the Ethernet payload (without VirtIO header)
    pub fn ethernet_payload(&self) -> &[u8] {
        if self.data.len() > VIRTIO_NET_HDR_SIZE {
            &self.data[VIRTIO_NET_HDR_SIZE..]
        } else {
            &[]
        }
    }

    /// Get mutable Ethernet payload
    pub fn ethernet_payload_mut(&mut self) -> &mut [u8] {
        if self.data.len() > VIRTIO_NET_HDR_SIZE {
            &mut self.data[VIRTIO_NET_HDR_SIZE..]
        } else {
            &mut []
        }
    }

    /// Validate the packet
    pub fn validate(&self) -> VirtioResult<()> {
        if self.data.len() < VIRTIO_NET_HDR_SIZE {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        let ethernet_len = self.data.len() - VIRTIO_NET_HDR_SIZE;
        if ethernet_len > VIRTIO_NET_MAX_PACKET_SIZE {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        if ethernet_len > 0 && ethernet_len < VIRTIO_NET_MIN_PACKET_SIZE {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        Ok(())
    }
}

impl Default for PacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Network packet abstraction
pub struct NetPacket {
    /// Packet buffer
    buffer: PacketBuffer,
}

impl NetPacket {
    /// Create a new network packet
    pub fn new() -> Self {
        Self {
            buffer: PacketBuffer::new(),
        }
    }

    /// Create a packet from raw data
    pub fn from_raw(data: Vec<u8>) -> Self {
        Self {
            buffer: PacketBuffer::from_data(data),
        }
    }

    /// Get the packet buffer
    pub fn buffer(&self) -> &PacketBuffer {
        &self.buffer
    }

    /// Get mutable packet buffer
    pub fn buffer_mut(&mut self) -> &mut PacketBuffer {
        &mut self.buffer
    }

    /// Prepare packet for transmission
    pub fn prepare_for_tx(&mut self) -> VirtioResult<()> {
        // Ensure we have a VirtIO header
        if self.buffer.len() < VIRTIO_NET_HDR_SIZE {
            let header = VirtioNetHeader::new();
            self.buffer.set_virtio_header(&header);
        }

        self.buffer.validate()
    }

    /// Prepare packet for reception
    pub fn prepare_for_rx(&mut self, ethernet_data: &[u8]) -> VirtioResult<()> {
        self.buffer.clear();
        
        // Add VirtIO header
        let header = VirtioNetHeader::new();
        self.buffer.set_virtio_header(&header);
        
        // Add Ethernet data
        self.buffer.extend_from_slice(ethernet_data);
        
        self.buffer.validate()
    }
}

impl Default for NetPacket {
    fn default() -> Self {
        Self::new()
    }
}

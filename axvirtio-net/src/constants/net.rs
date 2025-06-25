//! VirtIO Network Device Specific Constants

// ============================================================================
// VirtIO Network Device Features
// ============================================================================

/// Network device supports checksum offload
pub const VIRTIO_NET_F_CSUM: u64 = 1 << 0;

/// Network device supports guest checksum offload
pub const VIRTIO_NET_F_GUEST_CSUM: u64 = 1 << 1;

/// Network device supports control channel
pub const VIRTIO_NET_F_CTRL_VQ: u64 = 1 << 17;

/// Network device supports guest TSO4
pub const VIRTIO_NET_F_GUEST_TSO4: u64 = 1 << 7;

/// Network device supports guest TSO6
pub const VIRTIO_NET_F_GUEST_TSO6: u64 = 1 << 8;

/// Network device supports guest UFO
pub const VIRTIO_NET_F_GUEST_UFO: u64 = 1 << 10;

/// Network device supports host TSO4
pub const VIRTIO_NET_F_HOST_TSO4: u64 = 1 << 11;

/// Network device supports host TSO6
pub const VIRTIO_NET_F_HOST_TSO6: u64 = 1 << 12;

/// Network device supports host UFO
pub const VIRTIO_NET_F_HOST_UFO: u64 = 1 << 14;

/// Network device supports MAC address
pub const VIRTIO_NET_F_MAC: u64 = 1 << 5;

/// Network device supports status field
pub const VIRTIO_NET_F_STATUS: u64 = 1 << 16;

/// Network device supports multiqueue
pub const VIRTIO_NET_F_MQ: u64 = 1 << 22;

/// Combined feature set for basic network functionality
pub const VIRTIO_NET_FEATURES: u64 = VIRTIO_NET_F_MAC | VIRTIO_NET_F_STATUS;

// ============================================================================
// VirtIO Network Device Configuration Space Offsets
// (Relative to VIRTIO_MMIO_CONFIG)
// ============================================================================

/// MAC address offset (6 bytes)
pub const VIRTIO_NET_CFG_MAC: u64 = 0x00;

/// Status field offset (2 bytes)
pub const VIRTIO_NET_CFG_STATUS: u64 = 0x06;

/// Maximum number of virtqueue pairs offset (2 bytes)
pub const VIRTIO_NET_CFG_MAX_VQ_PAIRS: u64 = 0x08;

/// Maximum MTU offset (2 bytes)
pub const VIRTIO_NET_CFG_MTU: u64 = 0x0a;

// ============================================================================
// VirtIO Network Device Status Values
// ============================================================================

/// Link is up
pub const VIRTIO_NET_S_LINK_UP: u16 = 1;

/// Announce link status
pub const VIRTIO_NET_S_ANNOUNCE: u16 = 2;

// ============================================================================
// VirtIO Network Packet Header
// ============================================================================

/// Size of VirtIO network header
pub const VIRTIO_NET_HDR_SIZE: usize = 10;

/// Network header flags: needs checksum
pub const VIRTIO_NET_HDR_F_NEEDS_CSUM: u8 = 1;

/// Network header flags: data valid
pub const VIRTIO_NET_HDR_F_DATA_VALID: u8 = 2;

/// Network header GSO type: none
pub const VIRTIO_NET_HDR_GSO_NONE: u8 = 0;

/// Network header GSO type: TCPv4
pub const VIRTIO_NET_HDR_GSO_TCPV4: u8 = 1;

/// Network header GSO type: UDP
pub const VIRTIO_NET_HDR_GSO_UDP: u8 = 3;

/// Network header GSO type: TCPv6
pub const VIRTIO_NET_HDR_GSO_TCPV6: u8 = 4;

// ============================================================================
// VirtIO Network Queue Configuration
// ============================================================================

/// Receive queue index
pub const VIRTIO_NET_RX_QUEUE: u16 = 0;

/// Transmit queue index
pub const VIRTIO_NET_TX_QUEUE: u16 = 1;

/// Control queue index (if VIRTIO_NET_F_CTRL_VQ is enabled)
pub const VIRTIO_NET_CTRL_QUEUE: u16 = 2;

/// Default number of queue pairs
pub const VIRTIO_NET_DEFAULT_QUEUE_PAIRS: u16 = 1;

/// Maximum number of queue pairs
pub const VIRTIO_NET_MAX_QUEUE_PAIRS: u16 = 8;

// ============================================================================
// Network Packet Constants
// ============================================================================

/// Maximum transmission unit (MTU)
pub const VIRTIO_NET_DEFAULT_MTU: u16 = 1500;

/// Maximum packet size (MTU + Ethernet header + VLAN)
pub const VIRTIO_NET_MAX_PACKET_SIZE: usize = 1518;

/// Minimum Ethernet frame size
pub const VIRTIO_NET_MIN_PACKET_SIZE: usize = 60;

/// Ethernet header size
pub const ETHERNET_HEADER_SIZE: usize = 14;

/// MAC address size
pub const MAC_ADDRESS_SIZE: usize = 6;

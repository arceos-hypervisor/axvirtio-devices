//! VirtIO Console Device Specific Constants

// ============================================================================
// VirtIO Console Device Features
// ============================================================================

/// Console device supports multiple ports
pub const VIRTIO_CONSOLE_F_SIZE: u64 = 1 << 0;

/// Console device supports multiport
pub const VIRTIO_CONSOLE_F_MULTIPORT: u64 = 1 << 1;

/// Console device supports emergency write
pub const VIRTIO_CONSOLE_F_EMERG_WRITE: u64 = 1 << 2;

/// Combined feature set for basic console functionality
pub const VIRTIO_CONSOLE_FEATURES: u64 = VIRTIO_CONSOLE_F_SIZE;

// ============================================================================
// VirtIO Console Device Configuration Space Offsets
// (Relative to VIRTIO_MMIO_CONFIG)
// ============================================================================

/// Console columns offset (2 bytes)
pub const VIRTIO_CONSOLE_CFG_COLS: u64 = 0x00;

/// Console rows offset (2 bytes)
pub const VIRTIO_CONSOLE_CFG_ROWS: u64 = 0x02;

/// Maximum number of ports offset (4 bytes)
pub const VIRTIO_CONSOLE_CFG_MAX_NR_PORTS: u64 = 0x04;

/// Emergency write character offset (4 bytes)
pub const VIRTIO_CONSOLE_CFG_EMERG_WR: u64 = 0x08;

// ============================================================================
// VirtIO Console Queue Configuration
// ============================================================================

/// Receive queue index (port 0)
pub const VIRTIO_CONSOLE_RX_QUEUE: u16 = 0;

/// Transmit queue index (port 0)
pub const VIRTIO_CONSOLE_TX_QUEUE: u16 = 1;

/// Control receive queue index (if multiport is enabled)
pub const VIRTIO_CONSOLE_CTRL_RX_QUEUE: u16 = 2;

/// Control transmit queue index (if multiport is enabled)
pub const VIRTIO_CONSOLE_CTRL_TX_QUEUE: u16 = 3;

/// Default number of queues (RX + TX for port 0)
pub const VIRTIO_CONSOLE_DEFAULT_QUEUES: u16 = 2;

/// Number of queues with multiport (RX + TX + CTRL_RX + CTRL_TX)
pub const VIRTIO_CONSOLE_MULTIPORT_QUEUES: u16 = 4;

// ============================================================================
// VirtIO Console Control Messages
// ============================================================================

/// Control message: device ready
pub const VIRTIO_CONSOLE_DEVICE_READY: u16 = 0;

/// Control message: device add port
pub const VIRTIO_CONSOLE_DEVICE_ADD: u16 = 1;

/// Control message: device remove port
pub const VIRTIO_CONSOLE_DEVICE_REMOVE: u16 = 2;

/// Control message: port ready
pub const VIRTIO_CONSOLE_PORT_READY: u16 = 3;

/// Control message: console port
pub const VIRTIO_CONSOLE_CONSOLE_PORT: u16 = 4;

/// Control message: resize console
pub const VIRTIO_CONSOLE_RESIZE: u16 = 5;

/// Control message: port open
pub const VIRTIO_CONSOLE_PORT_OPEN: u16 = 6;

/// Control message: port name
pub const VIRTIO_CONSOLE_PORT_NAME: u16 = 7;

// ============================================================================
// VirtIO Console Buffer Sizes
// ============================================================================

/// Default console buffer size
pub const VIRTIO_CONSOLE_DEFAULT_BUFFER_SIZE: usize = 1024;

/// Maximum console buffer size
pub const VIRTIO_CONSOLE_MAX_BUFFER_SIZE: usize = 4096;

/// Minimum console buffer size
pub const VIRTIO_CONSOLE_MIN_BUFFER_SIZE: usize = 64;

// ============================================================================
// VirtIO Console Default Configuration
// ============================================================================

/// Default console columns
pub const VIRTIO_CONSOLE_DEFAULT_COLS: u16 = 80;

/// Default console rows
pub const VIRTIO_CONSOLE_DEFAULT_ROWS: u16 = 24;

/// Default maximum number of ports
pub const VIRTIO_CONSOLE_DEFAULT_MAX_PORTS: u32 = 1;

// ============================================================================
// VirtIO Console Control Message Structure
// ============================================================================

/// Size of console control message header
pub const VIRTIO_CONSOLE_CTRL_MSG_SIZE: usize = 8;

// ============================================================================
// VirtIO Console Port Configuration
// ============================================================================

/// Console port 0 (main console)
pub const VIRTIO_CONSOLE_PORT_0: u32 = 0;

/// Maximum number of console ports
pub const VIRTIO_CONSOLE_MAX_PORTS: u32 = 31;

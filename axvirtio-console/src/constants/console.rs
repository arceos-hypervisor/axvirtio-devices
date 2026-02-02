//! VirtIO Console Device Constants
//!
//! This module contains constants specific to VirtIO console devices,
//! including feature flags, configuration defaults, and queue indices.

use crate::constants::{VIRTIO_F_RING_EVENT_IDX, VIRTIO_F_VERSION_1};

// ============================================================================
// VirtIO Console Device Configuration Space Offsets
// (Relative to VIRTIO_MMIO_CONFIG)
// ============================================================================

/// Console columns (2 bytes at offset 0x00)
pub const VIRTIO_CONSOLE_CFG_COLS: u64 = 0x00;

/// Console rows (2 bytes at offset 0x02)
pub const VIRTIO_CONSOLE_CFG_ROWS: u64 = 0x02;

/// Maximum number of ports (4 bytes at offset 0x04)
pub const VIRTIO_CONSOLE_CFG_MAX_NR_PORTS: u64 = 0x04;

/// Emergency write (4 bytes at offset 0x08)
pub const VIRTIO_CONSOLE_CFG_EMERG_WR: u64 = 0x08;

// ============================================================================
// VirtIO Console Device Feature Flags
// ============================================================================

/// Console feature: Device has knowledge of the size
pub const VIRTIO_CONSOLE_F_SIZE: u64 = 1 << 0;

/// Console feature: Device supports multiple ports
pub const VIRTIO_CONSOLE_F_MULTIPORT: u64 = 1 << 1;

/// Console feature: Device supports emergency write
pub const VIRTIO_CONSOLE_F_EMERG_WRITE: u64 = 1 << 2;

/// Default device features for console
/// Includes VirtIO 1.0 support and size reporting
/// NOTE: EVENT_IDX is disabled because the Linux HVC driver's transmit completion
/// handler (out_intr) doesn't properly notify the HVC layer to send more data.
/// With flags mode, the Guest always kicks on every buffer, avoiding this issue.
pub const VIRTIO_CONSOLE_DEFAULT_FEATURES: u64 = VIRTIO_F_VERSION_1 | VIRTIO_CONSOLE_F_SIZE;

// ============================================================================
// VirtIO Console Queue Indices
// ============================================================================

/// Receive queue index (data from host to guest)
pub const VIRTIO_CONSOLE_RECEIVEQ: u16 = 0;

/// Transmit queue index (data from guest to host)
pub const VIRTIO_CONSOLE_TRANSMITQ: u16 = 1;

/// Control receive queue (for multiport mode)
pub const VIRTIO_CONSOLE_CTRL_RECEIVEQ: u16 = 2;

/// Control transmit queue (for multiport mode)
pub const VIRTIO_CONSOLE_CTRL_TRANSMITQ: u16 = 3;

/// Number of queues in single-port mode
pub const VIRTIO_CONSOLE_QUEUE_COUNT_SINGLE: usize = 2;

/// Number of queues in multiport mode (2 control + 2 per port)
pub const VIRTIO_CONSOLE_QUEUE_COUNT_MULTI_BASE: usize = 4;

// ============================================================================
// VirtIO Console Control Messages (for multiport mode)
// ============================================================================

/// Device is ready
pub const VIRTIO_CONSOLE_DEVICE_READY: u16 = 0;

/// Device add port
pub const VIRTIO_CONSOLE_DEVICE_ADD: u16 = 1;

/// Device remove port
pub const VIRTIO_CONSOLE_DEVICE_REMOVE: u16 = 2;

/// Port is ready
pub const VIRTIO_CONSOLE_PORT_READY: u16 = 3;

/// Console port
pub const VIRTIO_CONSOLE_CONSOLE_PORT: u16 = 4;

/// Console resize
pub const VIRTIO_CONSOLE_RESIZE: u16 = 5;

/// Port is open
pub const VIRTIO_CONSOLE_PORT_OPEN: u16 = 6;

/// Port name
pub const VIRTIO_CONSOLE_PORT_NAME: u16 = 7;

// ============================================================================
// VirtIO Console Default Configuration Values
// ============================================================================

/// Default console columns
pub const DEFAULT_CONSOLE_COLS: u16 = 80;

/// Default console rows
pub const DEFAULT_CONSOLE_ROWS: u16 = 25;

/// Default maximum number of ports (single port mode)
pub const DEFAULT_MAX_NR_PORTS: u32 = 1;

/// Default queue size for console
pub const DEFAULT_CONSOLE_QUEUE_SIZE: u16 = 64;

//! VirtIO Error Types
//!
//! This module defines common error types used across all VirtIO device implementations.

use axerrno::AxError;

/// VirtIO specific error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioError {
    /// Invalid queue configuration
    InvalidQueue,
    /// Queue not ready for operation
    QueueNotReady,
    /// Invalid descriptor
    InvalidDescriptor,
    /// Invalid access width for MMIO operation
    InvalidAccessWidth,
    /// Device not ready
    DeviceNotReady,
    /// Invalid device index
    InvalidDeviceIndex,
    /// Backend operation failed
    BackendError,
    /// Memory access error
    MemoryError,
    /// Invalid configuration
    InvalidConfig,
    /// Feature negotiation failed
    FeatureNegotiationFailed,
    /// Invalid request
    InvalidRequest,
    /// Operation not supported
    NotSupported,
    /// Invalid buffer size
    InvalidBufferSize,
    /// Invalid sector
    InvalidSector,
    /// Invalid register
    InvalidRegister,
}

impl VirtioError {
    /// Convert to AxError
    pub fn into_ax_error(self) -> AxError {
        match self {
            VirtioError::InvalidQueue => AxError::InvalidInput,
            VirtioError::QueueNotReady => AxError::BadState,
            VirtioError::InvalidDescriptor => AxError::InvalidInput,
            VirtioError::InvalidAccessWidth => AxError::InvalidInput,
            VirtioError::DeviceNotReady => AxError::BadState,
            VirtioError::InvalidDeviceIndex => AxError::InvalidInput,
            VirtioError::BackendError => AxError::Io,
            VirtioError::MemoryError => AxError::BadAddress,
            VirtioError::InvalidConfig => AxError::InvalidInput,
            VirtioError::FeatureNegotiationFailed => AxError::Unsupported,
            VirtioError::InvalidRequest => AxError::InvalidInput,
            VirtioError::NotSupported => AxError::Unsupported,
            VirtioError::InvalidBufferSize => AxError::InvalidInput,
            VirtioError::InvalidSector => AxError::InvalidInput,
            VirtioError::InvalidRegister => AxError::InvalidInput,
        }
    }
}

impl From<VirtioError> for AxError {
    fn from(err: VirtioError) -> Self {
        err.into_ax_error()
    }
}

/// Result type for VirtIO operations
pub type VirtioResult<T> = Result<T, VirtioError>;

use axvirtio_common::VirtioResult;
use crate::constants::*;

/// Trait for network device backends
pub trait NetworkBackend: Send + Sync {
    /// Send a packet to the network
    ///
    /// # Arguments
    /// * `packet` - The packet data to send
    ///
    /// # Returns
    /// Result indicating success or failure
    fn send_packet(&self, packet: &[u8]) -> VirtioResult<()>;

    /// Receive a packet from the network
    ///
    /// # Arguments
    /// * `buffer` - Buffer to store the received packet
    ///
    /// # Returns
    /// Number of bytes received, or 0 if no packet available
    fn receive_packet(&self, buffer: &mut [u8]) -> VirtioResult<usize>;

    /// Get the MAC address of the network interface
    ///
    /// # Returns
    /// 6-byte MAC address
    fn get_mac_address(&self) -> [u8; MAC_ADDRESS_SIZE];

    /// Set the MAC address of the network interface
    ///
    /// # Arguments
    /// * `mac` - 6-byte MAC address to set
    fn set_mac_address(&self, mac: [u8; MAC_ADDRESS_SIZE]) -> VirtioResult<()>;

    /// Get the current link status
    ///
    /// # Returns
    /// True if link is up, false otherwise
    fn is_link_up(&self) -> bool;

    /// Set the link status
    ///
    /// # Arguments
    /// * `up` - True to bring link up, false to bring it down
    fn set_link_up(&self, up: bool) -> VirtioResult<()>;

    /// Get the Maximum Transmission Unit (MTU)
    ///
    /// # Returns
    /// MTU size in bytes
    fn get_mtu(&self) -> u16;

    /// Set the Maximum Transmission Unit (MTU)
    ///
    /// # Arguments
    /// * `mtu` - MTU size in bytes
    fn set_mtu(&self, mtu: u16) -> VirtioResult<()>;

    /// Check if the backend supports promiscuous mode
    ///
    /// # Returns
    /// True if promiscuous mode is supported
    fn supports_promiscuous(&self) -> bool {
        false
    }

    /// Enable or disable promiscuous mode
    ///
    /// # Arguments
    /// * `enabled` - True to enable promiscuous mode, false to disable
    fn set_promiscuous(&self, enabled: bool) -> VirtioResult<()> {
        if enabled && !self.supports_promiscuous() {
            return Err(axvirtio_common::VirtioError::NotSupported);
        }
        Ok(())
    }

    /// Check if there are packets available for reception
    ///
    /// # Returns
    /// True if packets are available
    fn has_pending_packets(&self) -> bool {
        false
    }

    /// Get network statistics
    ///
    /// # Returns
    /// (packets_sent, packets_received, bytes_sent, bytes_received)
    fn get_statistics(&self) -> (u64, u64, u64, u64) {
        (0, 0, 0, 0)
    }

    /// Reset network statistics
    fn reset_statistics(&self) -> VirtioResult<()> {
        Ok(())
    }
}

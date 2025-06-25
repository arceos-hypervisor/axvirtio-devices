use axvirtio_common::{VirtioError, VirtioResult};

use super::traits::NetworkBackend;
use crate::constants::*;

/// TAP interface network backend
pub struct TapNetworkBackend {
    /// Device index
    device_index: usize,
    /// TAP interface name
    interface_name: alloc::string::String,
    /// MAC address
    mac_address: [u8; MAC_ADDRESS_SIZE],
    /// Link status
    link_up: bool,
    /// MTU size
    mtu: u16,
}

impl TapNetworkBackend {
    /// Create a new TAP network backend
    pub fn new(device_index: usize) -> VirtioResult<Self> {
        let interface_name = alloc::format!("tap{}", device_index);

        // Generate a deterministic MAC address based on device index
        let mac_address = [
            0x52,
            0x54,
            0x00, // QEMU OUI
            0x12,
            0x34,
            0x56 + device_index as u8,
        ];

        log::info!("Creating TAP interface: {}", interface_name);

        Ok(Self {
            device_index,
            interface_name,
            mac_address,
            link_up: false, // Will be set up when interface is configured
            mtu: VIRTIO_NET_DEFAULT_MTU,
        })
    }

    /// Get the TAP interface name
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    /// Initialize the TAP interface (placeholder for actual TAP setup)
    pub fn initialize(&mut self) -> VirtioResult<()> {
        // In a real implementation, this would:
        // 1. Create the TAP interface using system calls
        // 2. Configure the interface with the MAC address
        // 3. Set the MTU
        // 4. Bring the interface up

        log::info!("Initializing TAP interface: {}", self.interface_name);
        log::info!(
            "MAC address: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac_address[0],
            self.mac_address[1],
            self.mac_address[2],
            self.mac_address[3],
            self.mac_address[4],
            self.mac_address[5]
        );

        // For now, just mark as up
        self.link_up = true;

        Ok(())
    }
}

impl NetworkBackend for TapNetworkBackend {
    fn send_packet(&self, packet: &[u8]) -> VirtioResult<()> {
        if !self.link_up {
            return Err(VirtioError::DeviceNotReady);
        }

        if packet.len() > self.mtu as usize + ETHERNET_HEADER_SIZE {
            return Err(VirtioError::InvalidRequest);
        }

        // In a real implementation, this would write to the TAP file descriptor
        log::debug!(
            "TAP backend {}: would send packet of {} bytes to {}",
            self.device_index,
            packet.len(),
            self.interface_name
        );

        // Placeholder: In a real implementation, you would:
        // write(tap_fd, packet.as_ptr(), packet.len())

        Ok(())
    }

    fn receive_packet(&self, _buffer: &mut [u8]) -> VirtioResult<usize> {
        if !self.link_up {
            return Ok(0);
        }

        // In a real implementation, this would read from the TAP file descriptor
        // For now, return 0 to indicate no packets available
        log::debug!(
            "TAP backend {}: would read packet from {}",
            self.device_index,
            self.interface_name
        );

        // Placeholder: In a real implementation, you would:
        // let bytes_read = read(tap_fd, buffer.as_mut_ptr(), buffer.len());
        // return Ok(bytes_read);

        Ok(0)
    }

    fn get_mac_address(&self) -> [u8; MAC_ADDRESS_SIZE] {
        self.mac_address
    }

    fn set_mac_address(&self, mac: [u8; MAC_ADDRESS_SIZE]) -> VirtioResult<()> {
        // In a real implementation, this would update the TAP interface MAC address
        log::info!(
            "TAP backend {}: would set MAC address to {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.device_index,
            mac[0],
            mac[1],
            mac[2],
            mac[3],
            mac[4],
            mac[5]
        );

        // Placeholder: In a real implementation, you would use system calls
        // to update the interface MAC address

        Ok(())
    }

    fn is_link_up(&self) -> bool {
        self.link_up
    }

    fn set_link_up(&self, up: bool) -> VirtioResult<()> {
        log::info!(
            "TAP backend {}: would set link status to {} for {}",
            self.device_index,
            if up { "UP" } else { "DOWN" },
            self.interface_name
        );

        // In a real implementation, this would use system calls to bring
        // the interface up or down

        Ok(())
    }

    fn get_mtu(&self) -> u16 {
        self.mtu
    }

    fn set_mtu(&self, mtu: u16) -> VirtioResult<()> {
        if mtu < 68 || mtu > 9000 {
            return Err(VirtioError::InvalidRequest);
        }

        log::info!(
            "TAP backend {}: would set MTU to {} for {}",
            self.device_index,
            mtu,
            self.interface_name
        );

        // In a real implementation, this would use system calls to set the MTU

        Ok(())
    }

    fn supports_promiscuous(&self) -> bool {
        true // TAP interfaces typically support promiscuous mode
    }

    fn set_promiscuous(&self, enabled: bool) -> VirtioResult<()> {
        log::info!(
            "TAP backend {}: would {} promiscuous mode for {}",
            self.device_index,
            if enabled { "enable" } else { "disable" },
            self.interface_name
        );

        // In a real implementation, this would configure the TAP interface
        // promiscuous mode

        Ok(())
    }

    fn has_pending_packets(&self) -> bool {
        // In a real implementation, this would check if the TAP file descriptor
        // has data available for reading (e.g., using poll/select)
        false
    }
}

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use axvirtio_common::VirtioResult;
use spin::Mutex;

use super::traits::NetworkBackend;
use crate::constants::*;

/// Memory-based network backend for testing and simulation
pub struct MemoryNetworkBackend {
    /// Device index
    device_index: usize,
    /// MAC address
    mac_address: [u8; MAC_ADDRESS_SIZE],
    /// Link status
    link_up: bool,
    /// MTU size
    mtu: u16,
    /// Packet queue for simulation
    packet_queue: Mutex<VecDeque<Vec<u8>>>,
    /// Statistics
    stats: Mutex<NetworkStats>,
}

#[derive(Debug, Default)]
struct NetworkStats {
    packets_sent: u64,
    packets_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
}

impl MemoryNetworkBackend {
    /// Create a new memory network backend
    pub fn new(device_index: usize) -> Self {
        // Generate a deterministic MAC address based on device index
        let mac_address = [
            0x52, 0x54, 0x00, // QEMU OUI
            0x12, 0x34, 0x56 + device_index as u8,
        ];

        Self {
            device_index,
            mac_address,
            link_up: true,
            mtu: VIRTIO_NET_DEFAULT_MTU,
            packet_queue: Mutex::new(VecDeque::new()),
            stats: Mutex::new(NetworkStats::default()),
        }
    }

    /// Add a packet to the receive queue (for testing)
    pub fn inject_packet(&self, packet: Vec<u8>) {
        let mut queue = self.packet_queue.lock();
        queue.push_back(packet);
    }

    /// Get the number of packets in the queue
    pub fn packet_count(&self) -> usize {
        self.packet_queue.lock().len()
    }

    /// Clear all packets from the queue
    pub fn clear_packets(&self) {
        self.packet_queue.lock().clear();
    }
}

impl NetworkBackend for MemoryNetworkBackend {
    fn send_packet(&self, packet: &[u8]) -> VirtioResult<()> {
        if !self.link_up {
            return Err(axvirtio_common::VirtioError::DeviceNotReady);
        }

        if packet.len() > self.mtu as usize + ETHERNET_HEADER_SIZE {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        // Update statistics
        {
            let mut stats = self.stats.lock();
            stats.packets_sent += 1;
            stats.bytes_sent += packet.len() as u64;
        }

        // In a memory backend, we can simulate loopback by adding the packet back to the queue
        if packet.len() >= ETHERNET_HEADER_SIZE {
            let mut loopback_packet = Vec::from(packet);
            // Simple loopback: swap source and destination MAC addresses
            if loopback_packet.len() >= 12 {
                for i in 0..6 {
                    loopback_packet.swap(i, i + 6);
                }
            }
            self.inject_packet(loopback_packet);
        }

        log::debug!(
            "Memory backend {}: sent packet of {} bytes",
            self.device_index,
            packet.len()
        );

        Ok(())
    }

    fn receive_packet(&self, buffer: &mut [u8]) -> VirtioResult<usize> {
        if !self.link_up {
            return Ok(0);
        }

        let mut queue = self.packet_queue.lock();
        if let Some(packet) = queue.pop_front() {
            let len = packet.len().min(buffer.len());
            buffer[..len].copy_from_slice(&packet[..len]);

            // Update statistics
            drop(queue);
            let mut stats = self.stats.lock();
            stats.packets_received += 1;
            stats.bytes_received += len as u64;

            log::debug!(
                "Memory backend {}: received packet of {} bytes",
                self.device_index,
                len
            );

            Ok(len)
        } else {
            Ok(0)
        }
    }

    fn get_mac_address(&self) -> [u8; MAC_ADDRESS_SIZE] {
        self.mac_address
    }

    fn set_mac_address(&self, _mac: [u8; MAC_ADDRESS_SIZE]) -> VirtioResult<()> {
        // In a real implementation, this would update the MAC address
        // For the memory backend, we keep it read-only for simplicity
        Err(axvirtio_common::VirtioError::NotSupported)
    }

    fn is_link_up(&self) -> bool {
        self.link_up
    }

    fn set_link_up(&self, up: bool) -> VirtioResult<()> {
        // Note: This is not thread-safe in a real implementation
        // We would need proper synchronization
        let old_status = self.link_up;
        // self.link_up = up; // This would require interior mutability

        if old_status != up {
            log::info!(
                "Memory backend {}: link status changed to {}",
                self.device_index,
                if up { "UP" } else { "DOWN" }
            );
        }

        Ok(())
    }

    fn get_mtu(&self) -> u16 {
        self.mtu
    }

    fn set_mtu(&self, mtu: u16) -> VirtioResult<()> {
        if mtu < 68 || mtu > 9000 {
            return Err(axvirtio_common::VirtioError::InvalidRequest);
        }

        // Note: This would require interior mutability in a real implementation
        log::info!(
            "Memory backend {}: MTU change requested to {}",
            self.device_index,
            mtu
        );

        Ok(())
    }

    fn has_pending_packets(&self) -> bool {
        !self.packet_queue.lock().is_empty()
    }

    fn get_statistics(&self) -> (u64, u64, u64, u64) {
        let stats = self.stats.lock();
        (
            stats.packets_sent,
            stats.packets_received,
            stats.bytes_sent,
            stats.bytes_received,
        )
    }

    fn reset_statistics(&self) -> VirtioResult<()> {
        let mut stats = self.stats.lock();
        *stats = NetworkStats::default();
        Ok(())
    }
}

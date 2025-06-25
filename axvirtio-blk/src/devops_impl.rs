use axaddrspace::device::AccessWidth;
use axaddrspace::device::DeviceAddrRange;
use axdevice_base::BaseDeviceOps;
use axdevice_base::EmuDeviceType;

use axaddrspace::{GuestPhysAddr, GuestPhysAddrRange};
use axerrno::AxError;
use axerrno::AxResult;
use log::debug;
use memory_addr::MemoryAddr;

use crate::mmio::VirtioMmioDevice;

impl BaseDeviceOps<GuestPhysAddrRange> for VirtioMmioDevice {
    fn emu_type(&self) -> EmuDeviceType {
        EmuDeviceType::EmuDeviceTVirtioBlk
    }

    fn address_range(&self) -> GuestPhysAddrRange {
        GuestPhysAddrRange::new(self.base_ipa, self.base_ipa.add(self.length))
    }

    fn handle_read(&self, addr: GuestPhysAddr, width: AccessWidth) -> AxResult<usize> {
        self.mmio_read(addr, width)
    }

    fn handle_write(
        &self,
        addr: <GuestPhysAddrRange as DeviceAddrRange>::Addr,
        width: AccessWidth,
        val: usize,
    ) -> Result<(), AxError> {
        if let Err(e) = self.mmio_write(addr, width, val) {
            debug!("MMIO write error: {:?}", e);
        }

        Ok(())
    }
}

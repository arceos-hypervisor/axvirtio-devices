use axaddrspace::device::AccessWidth;
use axaddrspace::device::DeviceAddrRange;
use axdevice_base::BaseDeviceOps;
use axdevice_base::EmuDeviceType;

use axaddrspace::{GuestPhysAddr, GuestPhysAddrRange};
use axerrno::AxError;
use axerrno::AxResult;
use log::debug;
use log::trace;
use memory_addr::MemoryAddr;

use crate::VirtioConsoleDevice;

impl BaseDeviceOps<GuestPhysAddrRange> for VirtioConsoleDevice {
    fn emu_type(&self) -> EmuDeviceType {
        EmuDeviceType::EmuDeviceTVirtioConsole
    }

    fn address_range(&self) -> GuestPhysAddrRange {
        GuestPhysAddrRange::new(self.base_ipa, self.base_ipa.add(self.length))
    }

    fn handle_read(&self, addr: GuestPhysAddr, width: AccessWidth) -> AxResult<usize> {
        trace!("MMIO read at address: {:#x}, width: {:?}", addr, width);
        self.mmio_read(addr, width)
    }

    fn handle_write(
        &self,
        addr: <GuestPhysAddrRange as DeviceAddrRange>::Addr,
        width: AccessWidth,
        val: usize,
    ) -> Result<(), AxError> {
        trace!(
            "MMIO write at address: {:#x}, width: {:?}, value: {:#x}",
            addr,
            width,
            val
        );
        if let Err(e) = self.mmio_write(addr, width, val) {
            debug!("MMIO write error: {:?}", e);
        }
        Ok(())
    }
}

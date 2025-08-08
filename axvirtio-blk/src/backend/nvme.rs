use crate::backend::BlockBackend;
use alloc::vec::Vec;
use alloc::{ffi::CString, format, string::String};
use axdriver_pci::{
    BarInfo, Cam, Command, DeviceFunction, HeaderType, MemoryBarType, PciRangeAllocator, PciRoot,
};
use axhal::mem::virt_to_phys;
use axvirtio_common::{VirtioError, VirtioResult};
use byte_unit::Byte;
use core::{any::Any, ffi::CStr, ptr::NonNull};
use dma_api::{set_impl, Direction};
use log::{debug, error, info, trace, warn};
use memory_addr::{PhysAddr, VirtAddr};
use nvme_driver::{Config, Namespace, Nvme};
use spin::Mutex;

pub const PAGE_SIZE: usize = 0x1000;

// # Base physical address of the PCIe ECAM space.
// pci-ecam-base = 0x40_1000_0000  # uint
// # End PCI bus number (`bus-range` property in device tree).
// pci-bus-end = 0xff              # uint
// # PCI device memory ranges (`ranges` property in device tree).
const PCI_RANGES: [[u64; 2]; 3] = [
    [0x3ef_f0000, 0x1_0000],          // PIO space
    [0x1000_0000, 0x2eff_0000],       // 32-bit MMIO space
    [0x80_0000_0000, 0x80_0000_0000], // 64-bit MMIO space
]; // [(uint, uint)]

const PCI_BUS_END: u8 = 0xff;
const PCI_BAR_NUM: u8 = 6;

// NVMe controller PCI class codes
const PCI_CLASS_STORAGE: u8 = 0x01;
const PCI_SUBCLASS_NVM: u8 = 0x08;
const PCI_PROG_IF_NVME: u8 = 0x02;

struct DmaImpl;

impl dma_api::Impl for DmaImpl {
    fn map(addr: NonNull<u8>, _size: usize, _direction: Direction) -> u64 {
        let phys_addr = addr.as_ptr() as usize;
        let vaddr = VirtAddr::from(phys_addr);
        axhal::mem::virt_to_phys(vaddr).as_usize() as u64
    }

    fn unmap(addr: NonNull<u8>, size: usize) {
        // No-op for identity mapping, but log for debugging
        trace!(
            "DMA unmap: addr={:#x}, size={:#x}",
            addr.as_ptr() as u64,
            size
        );
    }

    fn flush(addr: NonNull<u8>, size: usize) {
        // No-op for now - in a real implementation, this would flush CPU caches
        trace!(
            "DMA flush: addr={:#x}, size={:#x}",
            addr.as_ptr() as u64,
            size
        );
    }

    fn invalidate(addr: NonNull<u8>, size: usize) {
        // No-op for now - in a real implementation, this would invalidate CPU caches
        trace!(
            "DMA invalidate: addr={:#x}, size={:#x}",
            addr.as_ptr() as u64,
            size
        );
    }
}

// DMA API implementation is set up using the set_impl! macro
set_impl!(DmaImpl);

/// Simple memory mapping function for PCI BAR addresses
/// In a real implementation, this would set up proper virtual memory mapping
fn simple_iomap(phys_addr: u64, _size: usize) -> NonNull<u8> {
    // For now, we assume identity mapping (physical == virtual)
    // In a real OS, this would involve setting up page tables
    NonNull::new(phys_addr as *mut u8).expect("Invalid physical address")
}

fn config_pci_device(
    root: &mut PciRoot,
    bdf: DeviceFunction,
    allocator: &mut Option<PciRangeAllocator>,
) -> Result<(), String> {
    let mut bar = 0;
    while bar < PCI_BAR_NUM {
        let info = root.bar_info(bdf, bar).unwrap();
        if let BarInfo::Memory {
            address_type,
            address,
            size,
            ..
        } = info
        {
            // if the BAR address is not assigned, call the allocator and assign it.
            if size > 0 && address == 0 {
                let new_addr = allocator
                    .as_mut()
                    .expect("No memory ranges available for PCI BARs!")
                    .alloc(size as _)
                    .ok_or("Failed to allocate memory for PCI BAR")?;
                if address_type == MemoryBarType::Width32 {
                    root.set_bar_32(bdf, bar, new_addr as _);
                } else if address_type == MemoryBarType::Width64 {
                    root.set_bar_64(bdf, bar, new_addr);
                }
            }
        }

        // read the BAR info again after assignment.
        let info = root.bar_info(bdf, bar).unwrap();
        match info {
            BarInfo::IO { address, size } => {
                if address > 0 && size > 0 {
                    debug!("  BAR {}: IO  [{:#x}, {:#x})", bar, address, address + size);
                }
            }
            BarInfo::Memory {
                address_type,
                prefetchable,
                address,
                size,
            } => {
                if address > 0 && size > 0 {
                    debug!(
                        "  BAR {}: MEM [{:#x}, {:#x}){}{}",
                        bar,
                        address,
                        address + size as u64,
                        if address_type == MemoryBarType::Width64 {
                            " 64bit"
                        } else {
                            ""
                        },
                        if prefetchable { " pref" } else { "" },
                    );
                }
            }
        }

        bar += 1;
        if info.takes_two_entries() {
            bar += 1;
        }
    }

    // Enable the device.
    let (_status, cmd) = root.get_status_command(bdf);
    root.set_command(
        bdf,
        cmd | Command::IO_SPACE | Command::MEMORY_SPACE | Command::BUS_MASTER,
    );
    Ok(())
}

pub fn scan_for_nvme() -> Option<NonNull<u8>> {
    let base_addr = 0x40_1000_0000 as *mut u8;
    let mut root = unsafe { PciRoot::new(base_addr, Cam::Ecam) };

    let mut allocator = PCI_RANGES
        .get(1)
        .map(|range| PciRangeAllocator::new(range[0], range[1]));

    info!("Scanning for NVMe devices...");
    for bus in 0..=PCI_BUS_END as u8 {
        for (bdf, dev_info) in root.enumerate_bus(bus) {
            debug!("PCI {}: {} {:?}", bdf, dev_info, dev_info.type_id());
            if dev_info.header_type != HeaderType::Standard {
                continue;
            }

            // Check if this is an NVMe controller
            let is_nvme = dev_info.class == PCI_CLASS_STORAGE
                && dev_info.subclass == PCI_SUBCLASS_NVM
                && dev_info.prog_if == PCI_PROG_IF_NVME;

            match config_pci_device(&mut root, bdf, &mut allocator) {
                Ok(_) => {
                    info!("PCI device {} enabled", bdf);

                    // If this is an NVMe controller, try to instantiate it
                    if is_nvme {
                        info!("Found NVMe controller at {}", bdf);

                        // Get BAR0 information for NVMe controller
                        if let Ok(bar_info) = root.bar_info(bdf, 0) {
                            if let BarInfo::Memory { address, size, .. } = bar_info {
                                if address > 0 && size > 0 {
                                    info!("NVMe BAR0: address={:#x}, size={:#x}", address, size);

                                    // Map the BAR address
                                    return Some(simple_iomap(address, size as usize));
                                } else {
                                    warn!("NVMe controller BAR0 not properly configured");
                                }
                            } else {
                                warn!("NVMe controller BAR0 is not a memory BAR");
                            }
                        } else {
                            error!("Failed to get BAR info for NVMe controller");
                        }
                    }
                }
                Err(e) => warn!(
                    "failed to enable PCI device at {}({}): {:?}",
                    bdf, dev_info, e
                ),
            }
        }
    }

    None
}

pub struct NvmeBackend {
    nvme: Mutex<Nvme>,
    name_space: Namespace,
}

// SAFETY: NvmeBackend is Send and Sync if the underlying hardware access is safe across threads.
// Ensure that you do not share mutable references across threads without synchronization.
unsafe impl Send for NvmeBackend {}
unsafe impl Sync for NvmeBackend {}

impl NvmeBackend {
    pub fn new() -> Self {
        let bar_ptr = scan_for_nvme().expect("No NVMe device found");
        let config = Config {
            page_size: PAGE_SIZE,
            io_queue_pair_count: 1,
        };
        info!("bar_ptr: {:#x}", bar_ptr.as_ptr() as u64);
        let mut nvme = Nvme::new(bar_ptr, config).expect("Failed to instantiate NVMe device");
        info!("nvme Ok");

        // 获取命名空间信息
        let namespace_list = match nvme.namespace_list() {
            Ok(list) => {
                info!("Found {} namespaces", list.len());
                for ns in &list {
                    let space = Byte::from_u64(ns.lba_size as u64 * ns.lba_count as u64);
                    info!("namespace: {:?}, space: {:#}", ns, space);
                }
                list
            }
            Err(e) => {
                error!("Failed to get namespace list: {:?}", e);
                Vec::new()
            }
        };
        Self {
            nvme: Mutex::new(nvme),
            name_space: namespace_list[0],
        }
    }
}

impl BlockBackend for NvmeBackend {
    fn read(&self, sector: u64, buffer: &mut [u8]) -> VirtioResult<usize> {
        trace!("read sector: {}", sector);
        match self
            .nvme
            .lock()
            .block_read_sync(&self.name_space, sector, buffer)
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to read from NVMe device: {:?}", e);
                return Err(VirtioError::BackendError);
            }
        }
        Ok(buffer.len())
    }
    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        match self
            .nvme
            .lock()
            .block_write_sync(&self.name_space, sector, buffer)
        {
            Ok(_) => {}
            Err(e) => {
                error!("Failed to write to NVMe device: {:?}", e);
                return Err(VirtioError::BackendError);
            }
        }
        Ok(buffer.len())
    }
    fn flush(&self) -> VirtioResult<()> {
        Ok(())
    }
}

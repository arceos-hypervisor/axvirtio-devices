use core::{alloc::Layout, ptr::NonNull};

use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use axvirtio_common::VirtioResult;
use log::{error, trace};
use spin::Mutex;
use virtio_drivers::{
    device::blk::VirtIOBlk,
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
    BufferDirection, Hal, PhysAddr, PAGE_SIZE,
};

use crate::backend::BlockBackend;

pub const VIRTIO_BASE_ADDR: usize = 0x0A00_0000;
pub const VIRTIO_SIZE: usize = 0x200; // 4K
pub const VIRTIO_COUNT: usize = 32;

pub struct HalImpl;

unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        // Safe because the layout has a non-zero size.
        let vaddr = unsafe { alloc_zeroed(layout) };
        let vaddr = if let Some(vaddr) = NonNull::new(vaddr) {
            vaddr
        } else {
            handle_alloc_error(layout)
        };
        let paddr = virt_to_phys(vaddr.as_ptr() as _);
        trace!("alloc DMA: paddr={:#x}, pages={}", paddr, pages);
        (paddr, vaddr)
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, vaddr: NonNull<u8>, pages: usize) -> i32 {
        trace!("dealloc DMA: paddr={:#x}, pages={}", paddr, pages);
        let layout = Layout::from_size_align(pages * PAGE_SIZE, PAGE_SIZE).unwrap();
        // Safe because the memory was allocated by `dma_alloc` above using the same allocator, and
        // the layout is the same as was used then.
        unsafe {
            dealloc(vaddr.as_ptr(), layout);
        }
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as _).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        // Nothing to do, as the host already has access to all memory.
        virt_to_phys(vaddr)
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Nothing to do, as the host already has access to all memory and we didn't copy the buffer
        // anywhere else.
    }
}

fn virt_to_phys(vaddr: usize) -> PhysAddr {
    vaddr
}

fn virtio_discover(device_type: DeviceType) -> Option<MmioTransport> {
    for i in 0..VIRTIO_COUNT {
        let base = VIRTIO_BASE_ADDR + i * VIRTIO_SIZE;
        let header = NonNull::new(base as *mut VirtIOHeader).unwrap();
        match unsafe { MmioTransport::new(header) } {
            Err(_) => {}
            Ok(transport) => {
                if transport.device_type() == device_type {
                    trace!(
                        "Detected virtio MMIO device with vendor id {:#X}, device type {:?}, version {:?}",
                        transport.vendor_id(),
                        transport.device_type(),
                        transport.version(),
                    );
                    return Some(transport);
                }
            }
        }
    }
    return None;
}

pub struct VirtioBackend {
    pub(crate) virtio: Mutex<VirtIOBlk<HalImpl, MmioTransport>>,
}

impl VirtioBackend {
    pub fn new() -> Self {
        let transport = virtio_discover(DeviceType::Block).unwrap();
        let virtio = VirtIOBlk::new(transport).unwrap();
        Self {
            virtio: Mutex::new(virtio),
        }
    }
}

impl BlockBackend for VirtioBackend {
    fn read(&self, sector: u64, buffer: &mut [u8]) -> VirtioResult<usize> {
        self.virtio
            .lock()
            .read_blocks(sector as usize, buffer)
            .unwrap();
        Ok(buffer.len())
    }
    fn write(&self, sector: u64, buffer: &[u8]) -> VirtioResult<usize> {
        self.virtio
            .lock()
            .write_blocks(sector as usize, buffer)
            .unwrap();
        Ok(buffer.len())
    }
    fn flush(&self) -> VirtioResult<()> {
        self.virtio.lock().flush().unwrap();
        Ok(())
    }
}

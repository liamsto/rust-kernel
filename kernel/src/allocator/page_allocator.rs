use core::arch::x86_64::_rdrand64_step;
use lazy_static::lazy_static;
use spin::mutex::Mutex;
use x86_64::{
    VirtAddr,
    structures::paging::{
        FrameAllocator, FrameDeallocator, Mapper, OffsetPageTable, Page, PageTableFlags, Size4KiB,
        mapper::{MapToError, UnmapError},
    },
};

use crate::memory::BitmapFrameAllocator;

lazy_static! {
    pub static ref PAGE_ALLOCATOR: Mutex<Option<PageAllocator<OffsetPageTable<'static>, BitmapFrameAllocator<'static>>>> =
        Mutex::new(None);
}

const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_START: usize = 0xFFFF_FF00_0000_0000;
pub const KERNEL_HEAP_SIZE: usize = 0x4000_0000; // 1GB
pub const KERNEL_HEAP_END: usize = KERNEL_HEAP_START + KERNEL_HEAP_SIZE;

pub struct PageAllocator<M, F> {
    pub frame_allocator: F,
    pub mapper: M,
    current_virt: usize,
    end_virt: usize,
}

impl<M, F> PageAllocator<M, F>
where
    M: Mapper<Size4KiB>,
    F: FrameAllocator<Size4KiB> + FrameDeallocator<Size4KiB>,
{
    pub fn new(mapper: M, frame_allocator: F, start_virt: usize, end_virt: usize) -> Self {
        PageAllocator {
            mapper,
            frame_allocator,
            current_virt: start_virt,
            end_virt,
        }
    }

    pub fn alloc(
        &mut self,
        num_pages: usize,
        flags: PageTableFlags,
    ) -> Result<usize, MapToError<Size4KiB>> {
        let bytes_needed = num_pages * PAGE_SIZE;
        if self.current_virt + bytes_needed > self.end_virt {
            return Err(MapToError::FrameAllocationFailed); // Out of memory
        }

        let start_addr = self.current_virt;

        for i in 0..num_pages {
            let page_virt = (start_addr + i * PAGE_SIZE) as u64;
            let page = Page::containing_address(VirtAddr::new(page_virt));
            let frame = self
                .frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;
            unsafe {
                self.mapper
                    .map_to(page, frame, flags, &mut self.frame_allocator)?
                    .flush();
            }

            self.current_virt += bytes_needed;
        }
        Ok(start_addr)
    }

    pub fn init_start_aslr(&mut self) {
        let mut rng = 0u64;
        unsafe {
            _rdrand64_step(&mut rng);
        }
        self.current_virt = KERNEL_HEAP_START + (rng as usize % KERNEL_HEAP_SIZE);
    }

    pub fn dealloc(&mut self, addr: usize, num_pages: usize) -> Result<(), UnmapError> {
        for i in 0..num_pages {
            let page_virt = (addr + i * PAGE_SIZE) as u64;
            let page = Page::containing_address(VirtAddr::new(page_virt));
            let (mapped_frame, flush) = self.mapper.unmap(page)?;
            flush.flush();
            //Safety: if this function is being called, you must be sure you are not deallocating a frame that is still in use
            unsafe {
                self.frame_allocator.deallocate_frame(mapped_frame);
            }
        }
        Ok(())
    }
}

pub fn init_page_allocator(
    mapper: OffsetPageTable<'static>,
    frame_alloc: BitmapFrameAllocator<'static>,
) {
    let page_alloc = PageAllocator::new(mapper, frame_alloc, KERNEL_HEAP_START, KERNEL_HEAP_END);
    crate::allocator::page_allocator::PAGE_ALLOCATOR
        .lock()
        .replace(page_alloc);
}

#[repr(C)]
pub struct PageAllocHeader {
    pub num_pages: usize,
}

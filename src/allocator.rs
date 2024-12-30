use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use fixed_size_block::FixedSizeBlockAllocator;
use page_allocator::PageAllocator;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub mod fixed_size_block;
pub mod page_allocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());
pub const HEAP_START: usize = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024;

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let start_page = Page::containing_address(heap_start);
        let end_page = Page::containing_address(heap_end);
        Page::range_inclusive(start_page, end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

pub fn init_heap_experimental(
    page_allocator: &mut PageAllocator<impl Mapper<Size4KiB>, impl FrameAllocator<Size4KiB>>,
) -> Result<(), MapToError<Size4KiB>> {
    //page_allocator.init_start_aslr();
    let num_pages = (HEAP_SIZE + 4095) / 4096; // Round up
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    let heap_start = page_allocator.alloc(num_pages, flags)?;
    unsafe {
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }

    Ok(())
}

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("Should not be called")
    }
}

// Wraps spin::Mutex to allow for trait implementations
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

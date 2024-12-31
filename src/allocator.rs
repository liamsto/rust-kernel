use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use fixed_size_block::FixedSizeBlockAllocator;
use page_allocator::PageAllocator;
use x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, FrameDeallocator, Mapper, Size4KiB,
};

pub mod alloc_info;
pub mod fixed_size_block;
pub mod page_allocator;

#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

pub fn init_heap_experimental(
    page_allocator: &mut PageAllocator<
        impl Mapper<Size4KiB>,
        impl FrameAllocator<Size4KiB> + FrameDeallocator<Size4KiB>,
    >,
) -> Result<(), MapToError<Size4KiB>> {
    unsafe {
        ALLOCATOR.lock().init(page_allocator);
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

/// A wrapper around spin::Mutex to allow for trait implementations.
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

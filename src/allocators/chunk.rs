use core::ptr::NonNull;
use spin::Mutex;
use x86_64::structures::paging::{FrameAllocator, Mapper, PageTableFlags};

pub struct ChunkManager {
    mapper: Mutex<&'static mut dyn Mapper<x86_64::structures::paging::Size4KiB>>,
    frame_allocator: Mutex<&'static mut dyn FrameAllocator<x86_64::structures::paging::Size4KiB>>,
}

impl ChunkManager {
    pub fn new(
        mapper: &'static mut dyn Mapper<x86_64::structures::paging::Size4KiB>,
        frame_allocator: &'static mut dyn FrameAllocator<x86_64::structures::paging::Size4KiB>,
    ) -> Self {
        Self { mapper: mapper.into(), frame_allocator: frame_allocator.into() }
    }

    /// Request a chunk of memory to create runs.
    pub fn allocate_chunk(&mut self, size: usize) -> Option<NonNull<u8>> {
        // 1. Round size up to multiple of page size.
        // 2. Use mapper and frame_allocator to map pages.
        // 3. Return pointer to start of chunk.
        unimplemented!()
    }

    /// Potentially free or recycle chunks (depends on policy).
    pub fn deallocate_chunk(&mut self, ptr: NonNull<u8>, size: usize) {
        // Unmap pages if desired. In a kernel, we may choose not to unmap.
        unimplemented!()
    }
}


unsafe impl Send for ChunkManager {}

unsafe impl Sync for ChunkManager {}
use core::ptr::NonNull;
use spin::{Mutex, Once};
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};

pub struct ChunkManager {
    mapper: Mutex<&'static mut dyn Mapper<x86_64::structures::paging::Size4KiB>>,
    frame_allocator: Mutex<&'static mut dyn FrameAllocator<x86_64::structures::paging::Size4KiB>>,
    next_addr: Mutex<u64>,
}

impl ChunkManager {
    pub fn new(
        mapper: &'static mut dyn Mapper<x86_64::structures::paging::Size4KiB>,
        frame_allocator: &'static mut dyn FrameAllocator<x86_64::structures::paging::Size4KiB>,
    ) -> Self {
        Self {
            mapper: mapper.into(),
            frame_allocator: frame_allocator.into(),
            next_addr: Mutex::new(0x_4444_4444_0000),
        }
    }

    /*
        Request a chunk of memory to create runs.
        This will map `size` bytes of memory (rounded up to page size)
        and return a pointer to the start.
    */
    pub fn allocate_chunk(&mut self, size: usize) -> Option<NonNull<u8>> {
        // 1. Round size up to multiple of page size.
        let page_size = 4096;
        let num_pages = (size + page_size - 1) / page_size;

        // 2. Find a free virtual address region
        // For now, use some fixed offset or maintain a bump for chunk allocation:
        static mut NEXT_CHUNK_ADDR: u64 = 0x_4444_4444_0000; // Just an example start address
        let start_addr = unsafe {
            let addr = NEXT_CHUNK_ADDR;
            NEXT_CHUNK_ADDR += (num_pages as u64) * (page_size as u64);
            addr
        };

        // Convert to Page range
        let start_page = Page::containing_address(x86_64::VirtAddr::new(start_addr));
        let end_page = Page::containing_address(x86_64::VirtAddr::new(
            start_addr + (num_pages as u64 * page_size as u64) - 1,
        ));
        let page_range = Page::range_inclusive(start_page, end_page);

        let mut mapper = self.mapper.lock();
        let mut frame_allocator = self.frame_allocator.lock();

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // 3. Map all pages
        for page in page_range {
            let frame = frame_allocator.allocate_frame().expect("Error allocating frame");
            unsafe {
                mapper
                    .map_to(page, frame, flags, &mut *frame_allocator)
                    .ok()?
                    .flush();
            }
        }

        // Return pointer to start of chunk
        Some(NonNull::new(start_addr as *mut u8).unwrap())
    }

    /// Potentially free or recycle chunks (depends on policy).
    pub fn deallocate_chunk(&mut self, ptr: NonNull<u8>, size: usize) {
        // Unmap pages if desired. In a kernel, we may choose not to unmap.
        unimplemented!()
    }
}

static GLOBAL_CHUNK_MANAGER: Once<ChunkManager> = Once::new();

pub unsafe fn init_global_allocator(
    mapper: &'static mut dyn Mapper<Size4KiB>,
    frame_allocator: &'static mut dyn FrameAllocator<Size4KiB>,
) {
    GLOBAL_CHUNK_MANAGER.call_once(|| ChunkManager::new(mapper, frame_allocator));
}

unsafe impl Send for ChunkManager {}

unsafe impl Sync for ChunkManager {}

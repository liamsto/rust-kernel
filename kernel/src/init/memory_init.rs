use crate::{
    allocator::{
        self,
        page_allocator::{PAGE_ALLOCATOR, init_page_allocator},
    },
    memory::{self, BitmapFrameAllocator},
};
use bootloader_api::BootInfo;
use bootloader_api::info::Optional;
use x86_64::VirtAddr;

pub fn init_memory(boot_info: &BootInfo) {
    // 1) Get the physical memory offset
    let offset = match boot_info.physical_memory_offset {
        Optional::Some(o) => o,
        Optional::None => panic!("Physical memory offset not provided by bootloader"),
    };

    // 2) Create a local mapper + frame-allocator
    let mapper = unsafe { memory::init(VirtAddr::new(offset)) };
    let allocator = unsafe { BitmapFrameAllocator::init(&boot_info.memory_regions, offset) };

    // 3) Install them as the global mapper & allocator
    init_page_allocator(mapper, allocator);

    // 4) Init your heap, etc.
    {
        let mut guard = PAGE_ALLOCATOR.lock();
        let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
        allocator::init_heap_experimental(page_alloc).expect("heap initialization failed");
    }
}

use crate::{
    allocator::{
        self,
        page_allocator::{PAGE_ALLOCATOR, init_page_allocator},
    },
    interrupts::PHYSICAL_MEMORY_OFFSET,
    memory::{self, BitmapFrameAllocator},
};
use bootloader_api::BootInfo;
use bootloader_api::info::Optional;
use x86_64::VirtAddr;

pub fn init_memory(boot_info: &BootInfo) {
    // 1) Get the physical memory offset
    let offset = match boot_info.physical_memory_offset {
        Optional::Some(o) => init_offset(VirtAddr::new(o)),
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

/// Initializes a write-once constant with the bootloader physical offset.
pub fn init_offset(offset: VirtAddr) -> u64 {
    PHYSICAL_MEMORY_OFFSET.call_once(|| offset);
    offset.as_u64()
}

/// Returns the physical memory offset of the kernel.
pub fn get_offset() -> VirtAddr {
    *PHYSICAL_MEMORY_OFFSET.wait()
}

pub fn get_offset_u64() -> u64 {
    get_offset().as_u64()
}

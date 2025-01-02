#![no_std]
#![no_main]
#![feature(custom_test_frameworks, box_as_ptr)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;
use alloc::{alloc::dealloc, boxed::Box};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use rust_os::allocator::{
    self,
    page_allocator::{init_page_allocator, PAGE_ALLOCATOR},
};

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use rust_os::memory::{self, BitmapFrameAllocator};
    use x86_64::VirtAddr;

    rust_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mapper = unsafe { memory::init(phys_mem_offset) };
    let test_allocator = unsafe {
        BitmapFrameAllocator::init(&boot_info.memory_map, boot_info.physical_memory_offset)
    };
    init_page_allocator(mapper, test_allocator);

    {
        let mut guard = PAGE_ALLOCATOR.lock();
        let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
        allocator::init_heap_experimental(page_alloc).expect("heap initialization failed");
    }

    test_main();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_os::test_panic_handler(info)
}

#[test_case]
fn simple_deallocation() {
    let heap_value_1 = Box::new(41);
    unsafe {
        dealloc(
            Box::<_>::into_raw(heap_value_1) as *mut u8,
            core::alloc::Layout::new::<i32>(),
        );
    }
    assert!(true);
}

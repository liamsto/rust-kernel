#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use bootloader_api::info::Optional;
use bootloader_api::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_os::allocator::page_allocator::PAGE_ALLOCATOR;
use rust_os::allocator::{self, page_allocator::init_page_allocator};

entry_point!(main);

fn main(boot_info: &'static mut BootInfo) -> ! {
    use rust_os::memory::{self, BitmapFrameAllocator};
    use x86_64::VirtAddr;

    rust_os::init();
    // let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // let mapper = unsafe { memory::init(phys_mem_offset) };
    // let test_allocator = unsafe {
    //     BitmapFrameAllocator::init(&boot_info.memory_map, boot_info.physical_memory_offset)
    // };
    // init_page_allocator(mapper, test_allocator);
    if let Optional::Some(physical_offset) = boot_info.physical_memory_offset {
        let mapper = unsafe { memory::init(VirtAddr::new(physical_offset)) };
        let test_allocator =
            unsafe { BitmapFrameAllocator::init(&boot_info.memory_regions, physical_offset) };
        init_page_allocator(mapper, test_allocator);
    } else {
        panic!("Physical memory offset not provided by bootloader");
    }

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
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

#[test_case]
fn large_vec() {
    let n = 500;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn many_boxes() {
    for i in 0..1000 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1); // new
    for i in 0..1000 {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1); // new
}

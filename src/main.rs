#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use alloc::vec::Vec;
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use rust_os::allocator::page_allocator::init_page_allocator;
use rust_os::allocator::page_allocator::PAGE_ALLOCATOR;
use rust_os::println;
use rust_os::task::executor::Executor;
use rust_os::task::{keyboard, Task};
extern crate alloc;

entry_point!(kernel_main);

#[no_mangle]
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use rust_os::allocator;
    use rust_os::memory::{self, BitmapFrameAllocator};
    use x86_64::VirtAddr;

    rust_os::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mapper = unsafe { memory::init(phys_mem_offset) };
    let allocator = unsafe {
        BitmapFrameAllocator::init(&boot_info.memory_map, boot_info.physical_memory_offset)
    };
    init_page_allocator(mapper, allocator);
    {
        let mut guard = PAGE_ALLOCATOR.lock();
        let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
        allocator::init_heap_experimental(page_alloc).expect("heap initialization failed");
    }

    let mut vec: Vec<u64> = Vec::with_capacity(512);
    for i in 0..512 {
        vec.push(i as u64);
    }
    let vec_size = core::mem::size_of_val(&vec);
    let elements_size = vec.len() * core::mem::size_of::<Vec<u64>>();
    let size = vec_size + elements_size;
    println!("vec occupies {} bytes", size);

    #[cfg(test)]
    test_main();

    let mut executor = Executor::new();
    executor.spawn(Task::new(example_task()));
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();
}

async fn async_number() -> u32 {
    42
}

async fn example_task() {
    let number = async_number().await;
    println!("async number: {}", number);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    rust_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_os::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

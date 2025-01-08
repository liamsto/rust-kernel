#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::info::Optional;
use bootloader_api::{entry_point, BootInfo};
use rust_os::interrupts::KernelAcpiHandler;
use core::panic::PanicInfo;
use rust_os::allocator::page_allocator::init_page_allocator;
use rust_os::allocator::page_allocator::PAGE_ALLOCATOR;
use rust_os::println;
use rust_os::task::executor::Executor;
use rust_os::task::{keyboard, Task};
extern crate alloc;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    use rust_os::allocator;
    use rust_os::memory::{self, BitmapFrameAllocator};
    use x86_64::VirtAddr;

    rust_os::init();

    let acpi_handler = KernelAcpiHandler {};

    let rsdp_addr = boot_info.rsdp_addr;
    if let Optional::Some(physical_offset) = boot_info.physical_memory_offset {
        // 1) create the mapper & frame allocator
        let mapper = unsafe { memory::init(VirtAddr::new(physical_offset)) };
        let test_allocator = unsafe {
            BitmapFrameAllocator::init(&boot_info.memory_regions, physical_offset)
        };

        init_page_allocator(mapper, test_allocator);



    } else {
        panic!("Physical memory offset not provided by bootloader");
    }

    {
        let mut guard = PAGE_ALLOCATOR.lock();
        let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
        allocator::init_heap_experimental(page_alloc)
            .expect("heap initialization failed");
    }

    println!("Testing heap allocation");
    //create a big array to test heap allocation
    let array = alloc::boxed::Box::new([0; 1000]);
    println!("Array location: {:p}", array);

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

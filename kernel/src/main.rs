#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use rust_os::init::multicore::init_multicore;
use core::panic::PanicInfo;
use rust_os::init::{self, graphics, memory_init};
use rust_os::println;
use rust_os::task::executor::Executor;
use rust_os::task::{Task, keyboard};
extern crate alloc;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    rust_os::init_gdt_idt();

    graphics::init_framebuffer(boot_info);

    memory_init::init_memory(boot_info);

    let (_tables, platform_info) = init::acpi::init_acpi(boot_info);

    if let Some (ref i) = platform_info.processor_info {
        init_multicore(i);
    }

    init::apic::init_apic(&platform_info);

    x86_64::instructions::interrupts::enable();

    println!("All initialization steps completed successfully!");

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

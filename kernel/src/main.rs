#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use acpi::HpetInfo;
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_os::smp::ap_protected::load_ap_trampoline;
use rust_os::init::hpet::{HPET_BASE, init_hpet};
use rust_os::init::{self, graphics, memory_init};
use rust_os::println;
use rust_os::task::executor::Executor;
use rust_os::task::{Task, keyboard};
use rust_os::timer::delay_ms;
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

    let (tables, platform_info) = init::acpi::init_acpi(boot_info);

    init::apic::init_apic(&platform_info);

    if let Ok(hpet_info) = HpetInfo::new(&tables) {
        init_hpet(&hpet_info);
        unsafe {
            println!("hello!");
            delay_ms(HPET_BASE, 10);
            println!("I should show up 10 ms later!");
        }
    }

    // if let Some(ref i) = platform_info.processor_info {
    //     unsafe { init_smp(APIC_BASE.expect("BSP APIC uninitalized!").as_ptr(), i) };
    // }

    x86_64::instructions::interrupts::enable();

    println!("All initialization steps completed successfully!");

    println!("Testing heap allocation");
    //create a big array to test heap allocation
    let array = alloc::boxed::Box::new([0; 1000]);
    println!("Array location: {:p}", array);

    #[cfg(test)]
    test_main();

    unsafe { load_ap_trampoline() };

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

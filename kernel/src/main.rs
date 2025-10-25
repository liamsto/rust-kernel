#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use acpi::HpetInfo;
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_kernel::apic_ptr::APIC_BASE;
use rust_kernel::init::hpet::init_hpet;
use rust_kernel::init::multicore::{init_smp, init_stack_top, remap_trampoline_uncacheable};
use rust_kernel::init::{self, graphics, memory_init};
use rust_kernel::smp::trampoline;
use rust_kernel::task::executor::Executor;
use rust_kernel::task::{Task, keyboard};
use rust_kernel::{println, serial_println};
extern crate alloc;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    rust_kernel::init_gdt_idt();

    graphics::init_framebuffer(boot_info);

    memory_init::init_memory(boot_info);

    serial_println!(
        "Physical memory offset: {:#?}",
        boot_info.physical_memory_offset
    );

    let (tables, platform_info) = init::acpi::init_acpi(boot_info);

    init::apic::init_apic(&platform_info);

    if let Ok(hpet_info) = HpetInfo::new(&tables) {
        init_hpet(&hpet_info);
    }

    x86_64::instructions::interrupts::enable();

    unsafe {
        //unmapped - sort out mapping?
        remap_trampoline_uncacheable();
        trampoline::load_ap_trampoline();
        init_stack_top();
        if let Some(i) = platform_info.processor_info {
            init_smp(APIC_BASE.expect("BSP APIC uninitalized!").as_ptr(), &i);
        }
    }

    println!("All initialization steps completed successfully!");

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
    rust_kernel::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info)
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

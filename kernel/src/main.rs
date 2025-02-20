#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use acpi::platform::interrupt::{Polarity, TriggerMode};
use acpi::{AcpiTables, InterruptModel, platform};
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::info::Optional;
use bootloader_api::{BootInfo, entry_point};
use x86_64::instructions::interrupts::int3;
use core::panic::PanicInfo;
use rust_os::allocator::page_allocator::PAGE_ALLOCATOR;
use rust_os::allocator::page_allocator::init_page_allocator;
use rust_os::apic_ptr::{u32_to_apic_ptr, APIC_BASE};
use rust_os::interrupts::{
    KernelAcpiHandler, TIMER_VEC, disable_pic, enable_local_apic, init_apic_timer,
    map_apic_registers, map_io_apic, set_ioapic_redirect,
};
use rust_os::task::executor::Executor;
use rust_os::task::{Task, keyboard};
use rust_os::{println, serial_println};
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
    if let Optional::Some(ref mut fb) = boot_info.framebuffer {
        let info = fb.info();
        rust_os::framebuffer::init_framebuffer_writer(fb.buffer_mut(), info);
    } else {
        panic!("No framebuffer available in BootInfo");
    }

    let physical_offset = boot_info.physical_memory_offset;
    let offset = match physical_offset {
        Optional::None => {
            panic!("Physical memory offset not provided by bootloader");
        }
        Optional::Some(offset) => offset,
    };


    let mapper = unsafe { memory::init(VirtAddr::new(offset)) };

    let allocator = unsafe { BitmapFrameAllocator::init(&boot_info.memory_regions, offset) };

    init_page_allocator(mapper, allocator);

    {
        let mut guard = PAGE_ALLOCATOR.lock();
        let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
        allocator::init_heap_experimental(page_alloc).expect("heap initialization failed");
    }

    let rsdp_addr = boot_info.rsdp_addr;

    let rsdp_addr = match rsdp_addr {
        Optional::Some(addr) => addr,
        Optional::None => {
            panic!("RSDP address not provided by bootloader");
        }
    };

    println!("RSDP located at {:#x}", rsdp_addr);
    let addr = rsdp_addr.try_into().unwrap();
    let acpi_handler = KernelAcpiHandler {};
    println!("ACPI handler created.");
    let tables = unsafe {
        match AcpiTables::from_rsdp(acpi_handler, addr) {
            Ok(tables) => {
                serial_println!("ACPI tables parsed successfully!");
                tables
            }
            Err(err) => {
                serial_println!("Error parsing ACPI tables: {:?}", err);
                panic!("Failed to parse ACPI tables: {:?}", err);
            }
        }
    };
    let platform_info = platform::PlatformInfo::new(&tables).unwrap();
    let interrupt_model = platform_info.interrupt_model;
    serial_println!("Interrupt model: {:?}", interrupt_model);

    match interrupt_model {
        InterruptModel::Apic(apic_info) => {
            // Map the APIC registers once:
            let mapped_ptr = map_apic_registers(apic_info.local_apic_address as u64);
            unsafe { APIC_BASE = Some(u32_to_apic_ptr(mapped_ptr)) };
            let local_apic_base = unsafe { &APIC_BASE.unwrap() };
    
            println!("[INFO] APIC registers mapped to {:#?}", local_apic_base.as_ptr());
            if apic_info.also_has_legacy_pics {
                disable_pic();
                serial_println!("PIC Disabled.");
            }
    
            // Use the already mapped pointer directly:
            let apic_mmio = local_apic_base.as_ptr();
            println!("[INFO] APIC MMIO at {:?}", apic_mmio);
    
            unsafe {
                enable_local_apic(apic_mmio);
                init_apic_timer(apic_mmio, TIMER_VEC);
            }

            //TODO:
            // APIC timer - set the LVT Timer register, divide configuration, and initial count
            // To handle NMI or external interrupts via the local APIC’s LINT pins, configure them in LVT LINT0/1 registers.
            // Multi core setup - repeat APIC init for each core


            serial_println!("Found {} I/O APICS", apic_info.io_apics.len());
            for io_apic in apic_info.io_apics.iter() {
                serial_println!(
                    "  IO APIC id={}, address={:#x}, GSI base={}",
                    io_apic.id,
                    io_apic.address,
                    io_apic.global_system_interrupt_base
                );
                map_io_apic(io_apic.address.try_into().unwrap());
                println!("[INFO] IO APIC LOCATION: {:#x}", io_apic.address);
                println!("[INFO] GLOBAL SYSTEM INTERRUPT BASE: {:#x}", io_apic.global_system_interrupt_base);
                unsafe {
                    set_ioapic_redirect(
                        io_apic.address.try_into().unwrap(),
                        1,
                        0,
                        0x2F,
                        TriggerMode::Edge,
                        Polarity::ActiveHigh,
                    )
                }
            }

            serial_println!(
                "Interrupt overrides: {}",
                apic_info.interrupt_source_overrides.len()
            );
            for iso in apic_info.interrupt_source_overrides.iter() {
                serial_println!(
                    "  Overriding ISA IRQ={} → GSI={}, polarity={:?}, trigger_mode={:?}",
                    iso.isa_source,
                    iso.global_system_interrupt,
                    iso.polarity,
                    iso.trigger_mode
                );
                // Possibly call `set_ioapic_redirect` again to handle the override. For example:
                // let vector = some_vector_for(iso.global_irq);
                // unsafe { set_ioapic_redirect(ioapic_base, iso.global_irq, local_apic_id, vector, ...) };
            }

            // 6) Check local_apic_nmi_lines, nmi_sources, etc., if needed
            serial_println!(
                "Local APIC NMI lines: {}",
                apic_info.local_apic_nmi_lines.len()
            );
            for nmi_line in apic_info.local_apic_nmi_lines.iter() {
                serial_println!("  local APIC NMI line: {:?}", nmi_line);
                // handle your local APIC NMI configuration
            }

            serial_println!("NMI sources: {}", apic_info.nmi_sources.len());
            for nmi_src in apic_info.nmi_sources.iter() {
                println!("  NMI source: {:?}", nmi_src);
                // configure NMI source if needed
            }
        }

        _ => {
            panic!("Non-APIC model!")
        }
    }
    x86_64::instructions::interrupts::enable();

    serial_println!("All functions called successfully");
    
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

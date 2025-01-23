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
use core::panic::PanicInfo;
use rust_os::allocator::page_allocator::PAGE_ALLOCATOR;
use rust_os::allocator::page_allocator::init_page_allocator;
use rust_os::apic_ptr::{APIC_BASE, as_apic_ptr};
use rust_os::interrupts::{
    disable_pic, enable_local_apic, init_apic_timer, map_apic_registers, map_io_apic, set_ioapic_redirect, KernelAcpiHandler, TIMER_VEC
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
    //let framebuffer = boot_info.framebuffer.as_ref().unwrap();

    serial_println!("Hello World{}", "!");

    let physical_offset = boot_info.physical_memory_offset;
    let offset = match physical_offset {
        Optional::None => {
            panic!("Physical memory offset not provided by bootloader");
        }
        Optional::Some(offset) => offset,
    };

    serial_println!(
        "Physical memory offset provided by bootloader: {:?}",
        offset
    );

    let mapper = unsafe { memory::init(VirtAddr::new(offset)) };

    let allocator = unsafe { BitmapFrameAllocator::init(&boot_info.memory_regions, offset) };

    init_page_allocator(mapper, allocator);
    serial_println!("Page allocator initialized");

    {
        let mut guard = PAGE_ALLOCATOR.lock();
        let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
        allocator::init_heap_experimental(page_alloc).expect("heap initialization failed");
        serial_println!("Heap initialized");
    }

    let rsdp_addr = boot_info.rsdp_addr;

    let rsdp_addr = match rsdp_addr {
        Optional::Some(addr) => addr,
        Optional::None => {
            panic!("RSDP address not provided by bootloader");
        }
    };

    let acpi_handler = KernelAcpiHandler {};
    let tables = unsafe { AcpiTables::from_rsdp(acpi_handler, rsdp_addr.try_into().unwrap()) };

    let tables = tables.expect("Failed to parse ACPI tables");
    let platform_info = platform::PlatformInfo::new(&tables).unwrap();
    let interrupt_model = platform_info.interrupt_model;

    match interrupt_model {
        InterruptModel::Apic(apic_info) => {
            let mut apic_base_guard = APIC_BASE.lock();
            apic_base_guard.replace(as_apic_ptr(apic_info.local_apic_address));
            let local_apic_base = apic_base_guard.as_ref().unwrap();
            serial_println!("Local APIC base: {:#x}", local_apic_base);
            if apic_info.also_has_legacy_pics {
                // If we also have a legacy PIC, we will need to disable that first before proceeding with APIC
                disable_pic();
                serial_println!("PIC Disabled.")
            }

            let apic_mmio = map_apic_registers(local_apic_base.as_u64());
            unsafe {
                init_apic_timer(apic_mmio, TIMER_VEC);
            }

            //TODO:
            // APIC timer - set the LVT Timer register, divide configuration, and initial count
            // To handle NMI or external interrupts via the local APIC’s LINT pins, configure them in LVT LINT0/1 registers.
            // Multi core setup - repeat APIC init for each core
            unsafe {
                enable_local_apic(apic_mmio);
            }

            drop(apic_base_guard); // release the lock, APIC_BASE is now initialized

            println!("Found {} I/O APICS", apic_info.io_apics.len());
            for io_apic in apic_info.io_apics.iter() {
                println!(
                    "  IO APIC id={}, address={:#x}, GSI base={}",
                    io_apic.id, io_apic.address, io_apic.global_system_interrupt_base
                );
                map_io_apic(io_apic.address.try_into().unwrap());
                unsafe {
                    set_ioapic_redirect(
                        io_apic.address.try_into().unwrap(),
                        33,
                        0,
                        0x2E,
                        TriggerMode::Edge,
                        Polarity::ActiveHigh,
                    )
                }
            }

            println!(
                "Interrupt overrides: {}",
                apic_info.interrupt_source_overrides.len()
            );
            for iso in apic_info.interrupt_source_overrides.iter() {
                println!(
                    "  Overriding ISA IRQ={} → GSI={}, polarity={:?}, trigger_mode={:?}",
                    iso.isa_source, iso.global_system_interrupt, iso.polarity, iso.trigger_mode
                );
                // Possibly call `set_ioapic_redirect` again to handle the override. For example:
                // let vector = some_vector_for(iso.global_irq);
                // unsafe { set_ioapic_redirect(ioapic_base, iso.global_irq, local_apic_id, vector, ...) };
            }

            // 6) Check local_apic_nmi_lines, nmi_sources, etc., if needed
            println!(
                "Local APIC NMI lines: {}",
                apic_info.local_apic_nmi_lines.len()
            );
            for nmi_line in apic_info.local_apic_nmi_lines.iter() {
                println!("  local APIC NMI line: {:?}", nmi_line);
                // handle your local APIC NMI configuration
            }

            println!("NMI sources: {}", apic_info.nmi_sources.len());
            for nmi_src in apic_info.nmi_sources.iter() {
                println!("  NMI source: {:?}", nmi_src);
                // configure NMI source if needed
            }
        }

        _ => {
            panic!("Non-APIC model!")
        }
    }
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

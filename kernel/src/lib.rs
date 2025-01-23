#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

#[cfg(test)]
use bootloader_api::{BootInfo, entry_point};

#[cfg(test)]
entry_point!(test_kernel_main);

use core::panic::PanicInfo;

pub mod allocator;
pub mod framebuffer;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod task;
pub mod vga_buffer;
pub mod apic_ptr;

extern crate alloc;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}.........", core::any::type_name::<T>());
        self();
        serial_println!("[OK]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// Exits QEMU with the given exit code by passing that code to the QEMU port I/O interface
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;
    unsafe {
        let mut port = Port::new(0xf4);
        serial_println!("Tests returned: {:?}!\nExiting QEMU...", exit_code);
        port.write(exit_code as u32);
    }
}

/// Entry point for `cargo xtest`
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    init();
    test_main();
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}

/// This function does several things. Firstly, it sets up the GDT (Global Descriptor Table).
/// After that, it initializes the IDT (Interrupt Descriptor Table). Then, it initializes the PICs (Programmable Interrupt Controllers).
/// Finally, it intializes the PIC and enables interrupts.
pub fn init() {
    serial_println!("initializing...");
    gdt::init();
    serial_println!("gdt initialized");
    interrupts::init_idt();
    serial_println!("idt initialized");
    //unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
    serial_println!("interrupts enabled");
}

// A wrapper for the `hlt` instruction that loops until an interrupt is received
// This is used to halt the CPU until the next interrupt is fired. If this wasn't done, the CPU would be running at 100% utilization, all the time.
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

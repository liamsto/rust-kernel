use core::ptr::NonNull;
use core::{panic, slice, usize};

use crate::allocator::page_allocator::{KERNEL_HEAP_START, PAGE_ALLOCATOR};
use crate::apic_ptr::APIC_BASE;
use crate::memory::PAGE_SIZE;
use crate::{gdt, print, println, serial_println};
use acpi::platform::interrupt::{Polarity, TriggerMode};
use acpi::rsdp::Rsdp;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub const TIMER_VEC: u8 = 0x2E;
pub const KEYBOARD_VEC: u8 = 0x2F;
pub const SPURIOUS_VEC: u8 = 0xFF;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[TIMER_VEC].set_handler_fn(apic_timer_interrupt_handler);
        idt[KEYBOARD_VEC].set_handler_fn(apic_keyboard_interrupt_handler);
        idt[SPURIOUS_VEC].set_handler_fn(spurious_interrupt_handler);

        idt.page_fault.set_handler_fn(apic_page_fault_handler);

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

pub fn init() {
    init_idt();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

// Timer interrupts arrive as interrupt 32 (from 0 + offset 32)
// Keyboard interrupts arrive as interrupt 33 (from 1 + offset 32). We don't need to explicitly set this since the default value is prev + 1.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // PS/2 Keyboard for now
}

impl InterruptIndex {
    fn _as_u8(self) -> u8 {
        self as u8
    }
}

// APIC Interrupt Handlers

extern "x86-interrupt" fn spurious_interrupt_handler(_frame: InterruptStackFrame) {
    println!("[NOTE] Spurious interrupt handler triggered.");
    let apic_mmio = unsafe { &APIC_BASE.expect("[ERROR] APIC_BASE unset!") };
    write_apic_reg(apic_mmio.as_ptr(), APIC_REG_EOI, 0);
}

extern "x86-interrupt" fn apic_timer_interrupt_handler(_frame: InterruptStackFrame) {
    print!(".");
    let apic_mmio = unsafe { &APIC_BASE.expect("[ERROR] APIC_BASE unset!") };
    write_apic_reg(apic_mmio.as_ptr(), APIC_REG_EOI, 0);
}

extern "x86-interrupt" fn apic_keyboard_interrupt_handler(_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    crate::task::keyboard::add_scancode(scancode);

    let apic_mmio = unsafe { &APIC_BASE.expect("[ERROR] APIC_BASE unset!") };
    write_apic_reg(apic_mmio.as_ptr(), APIC_REG_EOI, 0);
}

extern "x86-interrupt" fn apic_page_fault_handler(
    frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error code: {:#?}", error_code);
    println!("{:#?}", frame);

    let apic_mmio = unsafe { &APIC_BASE.expect("[ERROR] APIC_BASE unset!") };
    write_apic_reg(apic_mmio.as_ptr(), APIC_REG_EOI, 0);
}

use acpi::AcpiHandler;
use acpi::PhysicalMapping;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB};

const PHYSICAL_MEMORY_OFFSET: usize = 0x20000000000;

#[derive(Clone, Copy)]
/// An implementation of the `AcpiHandler` trait that can be used to map ACPI tables.
// Define the bootloader’s physical memory offset.

pub struct KernelAcpiHandler;

impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        // Determine the page boundaries.
        let phys_base_page = physical_address & !(PAGE_SIZE as usize - 1);
        let offset_in_page = physical_address - phys_base_page;
        let mapped_size = offset_in_page + size;
        // With the bootloader’s direct mapping, simply add the offset.
        let virt_base = PHYSICAL_MEMORY_OFFSET + phys_base_page;
        let t_virtual = (virt_base + offset_in_page) as *mut T;

        // Construct the PhysicalMapping. (The handler is just a marker type.)
        let mapping = unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new(t_virtual).expect("Mapping must not be null"),
                size,
                mapped_size,
                *self,
            )
        };

        mapping
    }

    // Because the bootloader mapping is permanent, unmapping is a no-op.
    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        serial_println!("unmap_physical_region: No operation performed (bootloader mapping)");
    }
}

pub fn map_physical(phys_addr: usize, num_pages: usize) -> usize {
    let mut pa_guard = PAGE_ALLOCATOR.lock();
    let page_alloc = pa_guard.as_mut().expect("PAGE_ALLOCATOR uninitialized");
    let virt_base = KERNEL_HEAP_START + phys_addr;

    // 2) for each page in [0..num_pages], map it to the existing physical address
    for i in 0..num_pages {
        let pa = phys_addr + i * (PAGE_SIZE as usize);
        let va = virt_base + i * (PAGE_SIZE as usize);
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(va as u64));

        // Instead of allocating a new frame, create a PhysFrame at `pa`
        let phys_frame = PhysFrame::containing_address(x86_64::PhysAddr::new(pa as u64));

        serial_println!(
            "Calling map_to for page {:#X} to physical address {:#X} and virtual address {:#X}",
            page.start_address(),
            pa,
            va
        );

        if let Ok(translate) = page_alloc.mapper.translate_page(page) {
            serial_println!(
                "Page {:?} is already mapped to {:?}. Ensuring frames are equal...",
                page,
                translate
            );
            if translate.start_address().as_u64() == phys_frame.start_address().as_u64() {
                continue;
            } else {
                serial_println!(
                    "translate.start_address() = {:#X}, phys_frame.start_address() = {:#X}",
                    translate.start_address().as_u64(),
                    phys_frame.start_address().as_u64()
                );
                panic!(
                    "Page {:?} is already mapped to a different frame {:?}",
                    page, translate
                );
            }
        }
        // if the page is unmapped, map it
        else {
            unsafe {
                let page_flush = match page_alloc.mapper.map_to(
                    page,
                    phys_frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    &mut page_alloc.frame_allocator,
                ) {
                    Ok(flush) => flush,
                    Err(e) => {
                        serial_println!("map_to failed: {:?}", e);
                        panic!("map_to failed: {:?}", e);
                    }
                };

                page_flush.flush();
            }
        }
    }

    serial_println!(
        "map_physical complete. Successfully mapped physical address {:#X} to virtual address {:#X}",
        phys_addr,
        virt_base
    );

    virt_base
}

const RSDP_SIGNATURE: [u8; 8] = *b"RSD PTR ";

/// A copy of the RSDP validation function for sanity checks
pub unsafe fn validate_rsdp<T>(rsdp_ptr: *mut T) -> bool {
    // Reinterpret the provided pointer as an Rsdp pointer.
    let rsdp = unsafe { &*(rsdp_ptr as *mut Rsdp) };

    // Check that the signature is correct.
    if rsdp.signature() != RSDP_SIGNATURE {
        serial_println!(
            "RSDP validation error: Incorrect signature. Found: {:?}, expected: {:?}",
            rsdp.signature(),
            RSDP_SIGNATURE
        );
        return false;
    }

    // Check that the OEM id is valid UTF-8.
    if str::from_utf8(rsdp.oem_id().as_bytes()).is_err() {
        serial_println!("RSDP validation error: OEM ID is not valid UTF-8.");
        return false;
    }

    // Determine the length: For ACPI versions 2.0+ use `rsdp.length`, otherwise use the hard-coded V1 length.
    let length = if rsdp.revision() > 0 {
        rsdp.length() as usize
    } else {
        20
    };

    // Create a slice of bytes covering the entire RSDP structure.
    let bytes = unsafe { slice::from_raw_parts(rsdp as *const Rsdp as *const u8, length) };

    // Calculate the checksum.
    let sum = bytes.iter().fold(0u8, |acc, &byte| acc.wrapping_add(byte));
    if sum != 0 {
        serial_println!(
            "RSDP validation error: Invalid checksum. Computed checksum: {}",
            sum
        );
        return false;
    }

    // If all checks pass, return true.
    true
}

/// Maps the APIC registers to physical memory.
/// # Parameters
///
/// - `apic_base`: The base address of the APIC.
///
/// # Returns
///
/// - `page_aligned_base`: The page-aligned base address.
///
/// # Example
///
/// ```rust
/// let apic_base: usize = 0xfee00000;
/// let page_aligned_base: usize = apic_base & !((PAGE_SIZE as usize) - 1);
/// assert_eq!(page_aligned_base, 0xfee00000);
/// ```
pub fn map_apic_registers(apic_base: u64) -> *mut u32 {
    let page_aligned_base: u64 = apic_base & !((PAGE_SIZE) - 1);
    let internal_page_offset = apic_base - page_aligned_base;
    // Use the bootloader's offset rather than KERNEL_HEAP_START.
    let virt_base = PHYSICAL_MEMORY_OFFSET + (page_aligned_base as usize);
    let apic_ptr = (virt_base + internal_page_offset as usize) as *mut u32;
    apic_ptr
}
/// Read the value of a given APIC register
///
/// # Parameters
/// - `reg_offset`: The offset of the register, which is expected to be a multiple of 4.
///
/// # Returns
/// - `index`: The calculated index as a `usize`.
///
/// # Example
/// ```rust
/// let reg_offset = 8;
/// let index = (reg_offset / 4) as usize;
/// assert_eq!(index, 2);
/// ```

fn read_apic_reg(apic_mmio: *mut u32, reg_offset: u32) -> u32 {
    let index = (reg_offset / 4) as usize;
    unsafe { core::ptr::read_volatile(apic_mmio.add(index)) }
}

/// Write a value to a given APIC register
///
/// # Parameters
/// - `apic_mmio`: The pointer to the APIC MMIO region.
/// - `reg_offset`: The offset of the register, which is expected to be a multiple of 4.
/// - `value`: The value to write to the register.
///
/// # Example
/// ```rust
/// let apic_mmio: *mut u32 = 0xfee00000 as *mut u32;
/// let reg_offset = 0x20;
/// let value = 0x12345678;
/// write_apic_reg(apic_mmio, reg_offset, value);
/// ```
fn write_apic_reg(apic_mmio: *mut u32, reg_offset: u32, value: u32) {
    let index = (reg_offset / 4) as usize;
    unsafe {
        core::ptr::write_volatile(apic_mmio.add(index), value);
    }
}

const APIC_REG_ID: u32 = 0x20; // Local APIC ID Register
const APIC_REG_TPR: u32 = 0x80; // Task Priority
const APIC_REG_EOI: u32 = 0xB0; // End of Interrupt
const APIC_REG_SVR: u32 = 0xF0; // SIV
const APIC_SVR_ENABLE: u32 = 1 << 8; // Bit storing 'APIC Software Enable' in SVR
const APIC_REG_LVT_TIMER: u32 = 0x320; // Local Vector Table Timer
const APIC_REG_TIMER_INITIAL_COUNT: u32 = 0x380;
//const APIC_REG_TIMER_CURRENT_COUNT: u32 = 0x390;
const APIC_REG_TIMER_DIV: u32 = 0x3E0;

pub unsafe fn init_apic_timer(apic_mmio: *mut u32, vector: u8) {
    //In this case, the "value" we write to the APIC register is the divide value. 0x3 is 16 (???).
    write_apic_reg(apic_mmio, APIC_REG_TIMER_DIV, 0x3);

    let lvt_timer_value = vector as u32 | 0x20000; // bit 17 is the mask bit
    write_apic_reg(apic_mmio, APIC_REG_LVT_TIMER, lvt_timer_value);

    let inital_count = 20_000_000; // placeholder
    write_apic_reg(apic_mmio, APIC_REG_TIMER_INITIAL_COUNT, inital_count);
}

pub unsafe fn enable_local_apic(apic_mmio: *mut u32) {
    // Set SVR
    let vector: u32 = 0xFF;
    let value = vector | APIC_SVR_ENABLE;
    write_apic_reg(apic_mmio, APIC_REG_SVR, value);

    // Clear the TPR by setting priority to 0 so all interrupts come in
    write_apic_reg(apic_mmio, APIC_REG_TPR, 0);

    let lapic_id = read_apic_reg(apic_mmio, APIC_REG_ID) >> 24;
    println!("Enabled local APIC with ID={}", lapic_id);
}

/// Returns a pointer to the I/O APIC register window.
pub fn map_io_apic(io_apic_base: usize) -> *mut u8 {
    let ptr = map_physical(io_apic_base, 1);
    ptr as *mut u8
}

const IOREGSEL: u32 = 0x00;
const IOWIN: u32 = 0x10;

/// Write a 32-bit register in the I/O APIC.
unsafe fn ioapic_write(ioapic_mmio: *mut u8, reg_index: u32, value: u32) {
    unsafe {
        // Write the index to IOREGSEL (offset 0x00)
        core::ptr::write_volatile(ioapic_mmio.add(IOREGSEL as usize).cast::<u32>(), reg_index);
        // Then write the value to IOWIN (offset 0x10)
        core::ptr::write_volatile(ioapic_mmio.add(IOWIN as usize).cast::<u32>(), value);
    }
}

/// Read a 32-bit register in the I/O APIC.
unsafe fn _ioapic_read(ioapic_mmio: *mut u8, reg_index: u32) -> u32 {
    unsafe {
        // Write the index
        core::ptr::write_volatile(ioapic_mmio.add(IOREGSEL as usize).cast::<u32>(), reg_index);
        // Then read from IOWIN
        core::ptr::read_volatile(ioapic_mmio.add(IOWIN as usize).cast::<u32>())
    }
}

pub unsafe fn set_ioapic_redirect(
    io_apic_base: usize,
    gsi: u32,
    dest_apic_id: u32,
    vector: u8,
    trigger: TriggerMode,
    polarity: Polarity,
) {
    // Map  the I/O APIC to read/write the regs
    let ioapic_mmio = map_io_apic(io_apic_base);

    // Each GSI has 2 regs: low dword and high dword
    // base index for GSI is 0x10 + 2*gsi

    let redtbl_index_low = 0x10 + 2 * gsi;
    let redtbl_index_high = redtbl_index_low + 1;

    //build the low dword:
    // bits [0..7]: 'vector'
    // bits [8..10]: 'delivery mode' (0 for 'fixed')
    // bit [13]: 0 for edge, 1 for level
    // bit [15]: 0 for active-high, 1 for active-low
    // bit [16]: 'mask' (0=enabled, 1=masked). 0 => not masked
    // etc. (some bits are advanced features, skip for now)
    // all subject to change

    let mut low_dword = vector as u32;

    //double check this at some point
    // supposedly:
    //   bit 13 = 0 => edge, 1 => level
    //   bit 13 is called 'trigger mode'

    let trigger_bit = match trigger {
        TriggerMode::Edge => 0 << 13,
        TriggerMode::Level => 1 << 13,
        TriggerMode::SameAsBus => 0 << 13, //idek
    };

    low_dword |= trigger_bit;

    //bit 15 => 0 for active high, 1 for active low
    let polarity_bit = match polarity {
        Polarity::ActiveHigh => 0 << 15,
        Polarity::ActiveLow => 1 << 15,
        Polarity::SameAsBus => 0 << 15, // again who knows that this does
    };

    low_dword |= polarity_bit;

    // (possibly) set the mask bit
    let mask_bit = 0 << 16;
    low_dword |= mask_bit;

    // The high dword: bits [24..31] is the APIC ID. (some say bits [56..63], but in x86_64 with xapic it's 24..31). Assuming xAPIC for now
    let high_dword = (dest_apic_id as u32) << 24;

    unsafe {
        ioapic_write(ioapic_mmio, redtbl_index_low, low_dword);
        ioapic_write(ioapic_mmio, redtbl_index_high, high_dword);
    }

    //maybe unmap here?
}

pub fn disable_pic() {
    use x86_64::instructions::port::Port;

    //const PIC1_CMD: u16 = 0x20;
    const PIC1_DATA: u16 = 0x21;
    //const PIC2_CMD: u16 = 0xA0;
    const PIC2_DATA: u16 = 0xA1;

    unsafe {
        let mut pic1_data = Port::new(PIC1_DATA);
        let mut pic2_data = Port::new(PIC2_DATA);
        pic1_data.write(0xFFu8);
        pic2_data.write(0xFFu8);
    }
}

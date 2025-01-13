use core::ptr::NonNull;
use core::{panic, usize};

use crate::allocator::page_allocator::{PAGE_ALLOCATOR, PageAllocator};
use crate::memory::{BitmapFrameAllocator, PAGE_SIZE};
use crate::{gdt, hlt_loop, print, println};
use acpi::platform::interrupt::{Polarity, TriggerMode};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);

        idt.page_fault.set_handler_fn(page_fault_handler);

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
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
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
    fn as_u8(self) -> u8 {
        self as u8
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    crate::task::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error code: {:#?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

use acpi::AcpiHandler;
use acpi::PhysicalMapping;
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
};

/// An implementation of the `AcpiHandler` trait that can be used to map ACPI tables.
#[derive(Clone)]
pub struct KernelAcpiHandler;

impl AcpiHandler for KernelAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let phys_base_page = physical_address & !(PAGE_SIZE as usize - 1);
        let offset_as_page = physical_address - phys_base_page;
        let mapped_size = offset_as_page + size;
        let num_pages = (mapped_size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

        let virt_base: usize = map_physical(phys_base_page, num_pages);

        let t_virtual = (virt_base + offset_as_page) as *mut T;

        unsafe {
            PhysicalMapping::new(
                physical_address,
                NonNull::new(t_virtual).expect("Mapping must not be null"),
                size,
                mapped_size,
                self.clone(),
            )
        }
    }
    fn unmap_physical_region<T>(region: &PhysicalMapping<Self, T>) {
        let virt_ptr = region.virtual_start().as_ptr() as usize;
        let physical_start = region.physical_start();

        let phys_base_page = physical_start & !(PAGE_SIZE as usize - 1);
        let offset_as_page = physical_start - phys_base_page;
        let mapped_size = offset_as_page + region.mapped_length();
        let num_pages = (mapped_size + PAGE_SIZE as usize - 1) / PAGE_SIZE as usize;

        // The actual base of the virtual mapping is (virt_ptr - offset_as_page)
        let virt_base = virt_ptr - offset_as_page;

        // Now unmap that range
        unmap_physical(virt_base, num_pages);
    }
}

pub fn map_physical(phys_addr: usize, num_pages: usize) -> usize {
    let mut pa_guard = PAGE_ALLOCATOR.lock();
    let page_alloc = pa_guard.as_mut().expect("PAGE_ALLOCATOR uninitialized");

    // 1) allocate a chunk of kernel virtual addresses from your “kernel pages”
    let virt_base = allocate_kernel_pages(page_alloc, num_pages);

    // 2) for each page in [0..num_pages], map it to the existing physical address
    for i in 0..num_pages {
        let pa = phys_addr + i * (PAGE_SIZE as usize);
        let va = virt_base + i * (PAGE_SIZE as usize);
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(va as u64));

        // Instead of allocating a new frame, create a PhysFrame at `pa`
        let phys_frame = PhysFrame::containing_address(x86_64::PhysAddr::new(pa as u64));

        unsafe {
            page_alloc
                .mapper
                .map_to(
                    page,
                    phys_frame, // existing frame at 'pa'
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    &mut page_alloc.frame_allocator,
                )
                .expect("map_to failed")
                .flush();
        }
    }

    virt_base
}

fn unmap_physical(base_page: usize, num_pages: usize) {
    let mut pa_guard = PAGE_ALLOCATOR.lock();
    let page_alloc = pa_guard.as_mut().expect("PAGE_ALLOCATOR not initialized");

    for i in 0..num_pages {
        let va = base_page + i * (PAGE_SIZE as usize);
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(va as u64));
        let (_frame, flush) = page_alloc.mapper.unmap(page).expect("unmap failed");
        flush.flush();

        //free?
        // unsafe { page_alloc.frame_allocator.deallocate_frame(frame); }
    }

    // Always free the kernel-virtual pages
    free_kernel_pages(page_alloc, base_page, num_pages);
}

fn free_kernel_pages(
    page_alloc: &mut PageAllocator<OffsetPageTable<'static>, BitmapFrameAllocator<'static>>,
    base: usize,
    num_pages: usize,
) {
    page_alloc
        .dealloc(base, num_pages)
        .expect("Failed to deallocate kernel pages");
}

fn allocate_kernel_pages(
    page_alloc: &mut PageAllocator<OffsetPageTable<'static>, BitmapFrameAllocator<'static>>,
    num_pages: usize,
) -> usize {
    let page = page_alloc.alloc(
        num_pages,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
    );
    page.expect("Failed to allocate kernel pages") as usize
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
pub fn map_apic_registers(apic_base: usize) -> *mut u32 {
    let page_aligned_base: usize = apic_base & !((PAGE_SIZE as usize) - 1);
    let internal_page_offset = apic_base - page_aligned_base;
    let virt_base = map_physical(page_aligned_base, 1);
    let apic_ptr = (virt_base + internal_page_offset) as *mut u32;
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

pub unsafe fn enable_local_apic(apic_mmio: *mut u32) {
    // Set SVR
    let vector: u32 = 0xFF;
    let value = vector | APIC_SVR_ENABLE;
    write_apic_reg(apic_mmio, APIC_REG_SVR, value);

    // Clear the TPR by setting priority to 0 so all interrupts come in
    write_apic_reg(apic_mmio, APIC_REG_TPR, 0);

    let lapic_id = read_apic_reg(apic_mmio, APIC_REG_ID) >> 24;
    println!("Enabled local APIC with ID={}", lapic_id);

    // TODO: APIC is software enabled, but we must also add an IDT entry for 0xFF
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
unsafe fn ioapic_read(ioapic_mmio: *mut u8, reg_index: u32) -> u32 {
    unsafe {
        // Write the index
        core::ptr::write_volatile(ioapic_mmio.add(IOREGSEL as usize).cast::<u32>(), reg_index);
        // Then read from IOWIN
        core::ptr::read_volatile(ioapic_mmio.add(IOWIN as usize).cast::<u32>())
    }
}

// if shit goes wrong it's definitely this function. Check intel manual if it does
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

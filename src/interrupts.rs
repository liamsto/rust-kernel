use core::ptr::NonNull;
use core::{panic, usize};

use crate::allocator::page_allocator::PAGE_ALLOCATOR;
use crate::memory::{FRAME_ALLOCATOR, MAPPER, PAGE_SIZE};
use crate::{gdt, hlt_loop, print, println};
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

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
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
use x86_64::structures::paging::{
    FrameDeallocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB
};
use x86_64::{PhysAddr, VirtAddr};

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

        PhysicalMapping::new(
            physical_address,
            NonNull::new(t_virtual).expect("Mapping must not be null"),
            size,
            mapped_size,
            self.clone(),
        )
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
    // find "kernel memory map" region for I/O or ACPI,
    let virt_base: usize = allocate_kernel_pages(num_pages);

    // 2) For each page in [0 .. num_pages], create a page-table entry
    //    pointing `virt_base + i*PAGE_SIZE` → `phys_addr + i*PAGE_SIZE`.
    let mut binding = MAPPER.lock();
    let mapper_lock = binding.as_mut().expect("Failed to lock MAPPER");
    //let mut mapper = mapper_lock;
    for i in 0..num_pages {
        let offset = i * PAGE_SIZE as usize;
        let pa = phys_addr + offset;
        let va = virt_base + offset;
        let phys_addr = PhysAddr::new(pa.try_into().unwrap());
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(va.try_into().unwrap()));
        let mut frame_alloc_guard = FRAME_ALLOCATOR.lock();
        let frame_alloc_ref = &mut *frame_alloc_guard;

        unsafe {
            mapper_lock
                .map_to(
                    page,
                    PhysFrame::containing_address(phys_addr),
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_alloc_ref,
                )
                .expect("map_to failed")
                .flush();
        }
    }
    virt_base
}
fn unmap_physical(base_page: usize, num_pages: usize) {
    let mut binding = MAPPER.lock();
    let mapper_lock = binding.as_mut().expect("Failed to lock MAPPER");
    let mut binding = FRAME_ALLOCATOR.lock();
    let frame_alloc = &mut *binding;
    for i in 0..num_pages {
        let va = base_page + i * PAGE_SIZE as usize;
        let page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(va as u64));
        let (frame, flush) = mapper_lock.unmap(page).expect("unmap failed");
        flush.flush();
        unsafe {
            frame_alloc.deallocate_frame(frame);
        }
    }

    free_kernel_pages(base_page, num_pages);
}

fn free_kernel_pages(base: usize, num_pages: usize) {
    let mut guard = PAGE_ALLOCATOR.lock();
    let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
    page_alloc
        .dealloc(base, num_pages)
        .expect("Failed to deallocate kernel pages");
}

fn allocate_kernel_pages(num_pages: usize) -> usize {
    let mut guard = PAGE_ALLOCATOR.lock();
    let page_alloc = guard.as_mut().expect("PAGE_ALLOCATOR not initialized");
    let page = page_alloc.alloc(
        num_pages,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
    );
    page.expect("alloc failed") as usize
}


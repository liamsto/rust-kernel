use core::ptr::NonNull;

use acpi::{AcpiHandler, PhysicalMapping};
use x86_64::{
    VirtAddr,
    structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB},
};

use crate::{
    allocator::page_allocator::{KERNEL_HEAP_START, PAGE_ALLOCATOR}, init::memory_init::get_offset_u64, memory::PAGE_SIZE, serial_println
};

#[derive(Clone, Copy)]
/// An implementation of the `AcpiHandler` trait that can be used to map ACPI tables.
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
        let virt_base = get_offset_u64() as usize + phys_base_page;
        let t_virtual = (virt_base + offset_in_page) as *mut T;

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
        //serial_println!("unmap_physical_region: No operation performed (bootloader mapping)");
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

        let phys_frame = PhysFrame::containing_address(x86_64::PhysAddr::new(pa as u64));
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

    virt_base
}

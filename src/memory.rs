use x86_64::{
    structures::paging::{OffsetPageTable, PageTable},
    VirtAddr,
};

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

/*
Initializes an instance of OffsetPageTable.
Must be marked unsafe because caller guarantees that the physical memory
is being mapped to the virtual memory specified by 'physical_memory_offset'.
This function is only to be called once, to avoid aliasing mutable references,
which is undefined behavior in Rust.

Aliasing: When two or more pointers point to the same memory location:
"You can have either one mutable reference or any number of immutable
references to a particular piece of data at any given time."

For example:

    unsafe {
        let table_ptr = active_level_4_table(offset); // Obtain a mutable reference
        let table_ref = OffsetPageTable::new(table_ptr, offset); // Use it for initialization

        // If another reference to the same memory is created:
        let illegal_alias = &*table_ptr; // Immutable alias (or another mutable alias)

        println!("{:?}", illegal_alias); // Attempt to use the aliased reference
        println!("{:?}", table_ref);     // Attempt to use the original reference
    }

Will result in undefined behavior, as two references (one mutable and one immutable)
are accessing the same memory concurrently. To prevent this, ensure that this function
is called only once and that the returned `OffsetPageTable` instance owns the memory.

*/
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

use x86_64::{
    structures::paging::{FrameAllocator, Mapper, Page, PhysFrame, Size4KiB},
    PhysAddr,
};

pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_result = unsafe {
        //do not do this
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_result.expect("map_to failed").flush();
}

pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /*
    Create a FrameAllocator from a given memory map. Must be marked unsafe
    because the caller has to guarantee the passed memory map is valid.
    All frames marked 'USABLE' in the map must be really unused.
     */
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    // Return an iterator over usable frames from a given map
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));

        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

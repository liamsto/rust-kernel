use core::u64;

use x86_64::{
    structures::paging::{FrameDeallocator, OffsetPageTable, PageTable},
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

// EXPERIMENTAL

use alloc::vec;
use alloc::vec::Vec;

const PAGE_SIZE: u64 = 4096;

pub struct BitmapFrameAllocator {
    base_addr: u64,
    frame_count: usize,
    bitmap: Vec<bool>,
}

impl BitmapFrameAllocator {
    pub unsafe fn init(memory_map: &MemoryMap) -> Self {
        let usable_regions = memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable);

        //get the lowest starting address and highest ending addr of all usable regions, which we will use to define our overall bitmap range
        let mut min_addr = u64::MAX;
        let mut max_addr = 0;

        for region in usable_regions.clone() {
            if region.range.start_addr() < min_addr {
                min_addr = region.range.start_addr();
            }
            if region.range.end_addr() > max_addr {
                max_addr = region.range.end_addr();
            }
        }

        //ensure alignment of min_addr along page boundaries so we only handle full frames
        let min_frame_addr = (min_addr / PAGE_SIZE) * PAGE_SIZE;
        let max_frame_addr = ((max_addr + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;

        let frame_count = ((max_frame_addr - min_frame_addr) / PAGE_SIZE) as usize;
        let mut bitmap = vec![true; frame_count];

        //mark all usable frames as free
        for region in usable_regions {
            let start = region.range.start_addr();
            let end = region.range.end_addr();

            let start_frame = (start / PAGE_SIZE) * PAGE_SIZE;
            let end_frame = ((end + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;

            for addr in (start_frame..end_frame).step_by(PAGE_SIZE as usize) {
                let frame = (addr - min_frame_addr) / PAGE_SIZE;
                bitmap[frame as usize] = false; // false = free
            }
        }

        BitmapFrameAllocator {
            base_addr: min_frame_addr,
            frame_count: frame_count as usize,
            bitmap,
        }
    }

    fn frame_as_index(&self, frame: PhysFrame) -> Option<usize> {
        let frame_addr = frame.start_address().as_u64();
        if frame_addr < self.base_addr {
            return None;
        }
        let offset = frame_addr - self.base_addr;
        let index = offset / PAGE_SIZE;
        if index >= self.frame_count as u64 {
            return None;
        } else {
            return Some(index as usize);
        }
    }

    fn index_as_frame(&self, index: usize) -> PhysFrame {
        let addr = self.base_addr + (index as u64) * PAGE_SIZE;
        PhysFrame::containing_address(PhysAddr::new(addr))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BitmapFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        //find the first free frame (false in bitmap)
        //1. Create an iterator over the bitmap, which is a vector of booleans
        //2. Enumerate the iterator, which creates an iterator of tuples (index, element)
        //3. Find the first tuple where the element is false - "|(_i, used)| !**used" checks if the element "used" is not true - the double deref is necessary because iterator provides &bool
        //4. If a tuple is found, mark the frame as used and return it
        //5. If no tuple is found, return None
        if let Some((idx, _)) = self.bitmap.iter().enumerate().find(|(_i, used)| !**used) {
            self.bitmap[idx] = true; //mark as used
            Some(self.index_as_frame(idx)) // return the physical frame
        } else {
            //no free frames
            None
        }
    }
}

impl FrameDeallocator<Size4KiB> for BitmapFrameAllocator {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        if let Some(idx) = self.frame_as_index(frame) {
            self.bitmap[idx] = false; //mark as free
        } else {
            //frame not found
            //TODO: for now we panic, but in the future we will want to handle this more gracefully
            todo!("Attempted to deallocate frame that was not allocated by the allocator");
        }
    }
}

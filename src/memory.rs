use core::u64;

use x86_64::{
    structures::paging::{FrameDeallocator, OffsetPageTable, PageTable},
    VirtAddr,
};

use bootloader_api::info::{MemoryRegionKind, MemoryRegions};

use bitvec::prelude::*;
use spin::Mutex;

use crate::println;

pub const PAGE_SIZE: u64 = 4096;

pub struct BitmapFrameAllocator<'a> {
    base_addr: u64,
    frame_count: usize,
    bitmap: Mutex<&'a mut BitSlice<u8, Lsb0>>,
}

impl<'a> BitmapFrameAllocator<'a> {
    pub unsafe fn init(memory_map: &MemoryRegions, offset: u64) -> Self {
        // 1) Print out the memory map for debugging
        // for region in memory_map.iter() {
        //     println!(
        //         "Region: start={:#x}, end={:#x}, type={:?}",
        //         region.range.start_addr(),
        //         region.range.end_addr(),
        //         region.region_type
        //     );
        // }

        // 2) Find the maximum physical address in all "Usable" regions

        let mut max_addr = 0;
        // for region in memory_map.iter() {
        //     if region.region_type == MemoryRegionType::Usable {
        //         if region.range.end_addr() > max_addr {
        //             max_addr = region.range.end_addr();
        //         }
        //     }
        // }

        for region in memory_map.iter() {
            if region.kind == MemoryRegionKind::Usable {
                if region.end > max_addr {
                    max_addr = region.end;
                }
            }
        }

        // 3) Convert max_addr -> max_frame, figure out how many frames we have in total
        let max_frame = (max_addr + PAGE_SIZE - 1) / PAGE_SIZE;
        let frame_count = max_frame as usize;
        println!("Max frame: {}", max_frame);

        // 4) Compute how many bytes our bitmap needs (1 bit per frame)
        let bytes_needed = (frame_count + 7) / 8;
        println!("Bytes needed: {}", bytes_needed);

        // 5) Collect all "non-usable" regions into a Vec so we can skip them
        const MAX_ILLEGAL: usize = 32; // or however many
        static mut ILLEGAL_REGIONS: [AddressRange; MAX_ILLEGAL] =
            [AddressRange { start: 0, end: 0 }; MAX_ILLEGAL];

        // let mut count = 0;
        // for region in memory_map.iter() {
        //     if region.region_type != MemoryRegionType::Usable && count < MAX_ILLEGAL {
        //         unsafe {
        //             ILLEGAL_REGIONS[count] = AddressRange {
        //                 start: region.range.start_addr(),
        //                 end: region.range.end_addr(),
        //             };
        //         }
        //         count += 1;
        //     }
        // }

        let mut count = 0;
        for region in memory_map.iter() {
            if region.kind != MemoryRegionKind::Usable && count < MAX_ILLEGAL {
                unsafe {
                    ILLEGAL_REGIONS[count] = AddressRange {
                        start: region.start,
                        end: region.end,
                    };
                }
                count += 1;
            }
        }

        // 6) Find a single "Usable" region large enough to hold the bitmap without overlapping any "illegal" region
        let mut region_base = None;

        'outer: for region in memory_map.iter() {
            if region.kind == MemoryRegionKind::Usable {
                let start = region.start;
                let end = region.end;
                let size = end - start;

                // If region can't fit the bitmap, skip it
                if size < bytes_needed as u64 {
                    continue;
                }

                // Check if it intersects any illegal region
                let local_illegal_regions = ILLEGAL_REGIONS;
                for off in &local_illegal_regions {
                    if ranges_intersect(start, end, off.start, off.end) {
                        // Overlaps something non-usable, skip
                        continue 'outer;
                    }
                }

                // If we got here, this region is big enough and doesn't overlap the illegal ranges
                region_base = Some(start);
                break;
            }
        }

        if region_base.is_none() {
            panic!("Could not find a suitable region to place the bitmap!");
        }
        let bitmap_phys_addr = region_base.unwrap();

        // 7) Convert that physical address into a virtual address
        let bitmap_virt_addr = phys_to_virt(bitmap_phys_addr, offset);

        // 8) Create a slice that references that memory
        use core::slice;
        let bitmap_slice =
            unsafe { slice::from_raw_parts_mut(bitmap_virt_addr as *mut u8, bytes_needed) };

        // 9) Convert that slice into a BitSlice
        use bitvec::prelude::*;
        let bitmap_bits: &mut BitSlice<u8, Lsb0> = BitSlice::from_slice_mut(bitmap_slice);

        // 10) Initialize everything to "used" (true)
        for i in 0..bitmap_bits.len() {
            bitmap_bits.set(i, true);
        }

        // 11) Mark the bitmap's own frames as used
        let start_frame = bitmap_phys_addr / PAGE_SIZE;
        let end_frame = (bitmap_phys_addr + bytes_needed as u64 + PAGE_SIZE - 1) / PAGE_SIZE;
        for frame_num in start_frame..end_frame {
            if frame_num < max_frame {
                bitmap_bits.set(frame_num as usize, true);
            }
        }

        // 12) Now mark all truly free frames (in "Usable" ranges) as false
        for region in memory_map.iter() {
            if region.kind == MemoryRegionKind::Usable {
                let start_frame = region.end / PAGE_SIZE;
                let end_frame = (region.start + PAGE_SIZE - 1) / PAGE_SIZE;

                for frame in start_frame..end_frame {
                    if frame >= max_frame as u64 {
                        break;
                    }
                    let frame_addr = frame * PAGE_SIZE;
                    let frame_end = frame_addr + PAGE_SIZE;

                    // Skip if it intersects the bitmap storage
                    let bitmap_end = bitmap_phys_addr + bytes_needed as u64;
                    if ranges_intersect(frame_addr, frame_end, bitmap_phys_addr, bitmap_end) {
                        continue;
                    }

                    // Skip if it intersects any illegal region
                    let mut intersects_illegal = false;

                    let mut local_illegal_regions = ILLEGAL_REGIONS;
                    for off in &mut local_illegal_regions {
                        if ranges_intersect(frame_addr, frame_end, off.start, off.end) {
                            intersects_illegal = true;
                            break;
                        }
                    }

                    if intersects_illegal {
                        continue;
                    }

                    // If none of the above conditions triggered, it's truly free
                    bitmap_bits.set(frame as usize, false);
                }
            }
        }

        let mut free_count = 0;
        for i in 0..bitmap_bits.len() {
            if !bitmap_bits[i] {
                free_count += 1;
            }
        }
        println!("Total free frames: {}", free_count);

        BitmapFrameAllocator {
            base_addr: 0,
            frame_count,
            bitmap: Mutex::new(bitmap_bits),
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
            None
        } else {
            Some(index as usize)
        }
    }

    fn index_as_frame(&self, index: usize) -> PhysFrame {
        let addr = self.base_addr + (index as u64) * PAGE_SIZE;
        PhysFrame::containing_address(PhysAddr::new(addr))
    }
}

unsafe impl<'a> FrameAllocator<Size4KiB> for BitmapFrameAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        // Find the first free frame (a 'false' bit in the bitvec).
        let mut bitmap_guard = self.bitmap.lock();

        // Split the iteration and bit setting into two steps to avoid borrowing issues.
        let free_index = {
            let mut bit_iter = bitmap_guard.iter().enumerate();
            bit_iter.find(|(_, bit)| !**bit).map(|(idx, _)| idx)
        };

        if let Some(idx) = free_index {
            bitmap_guard.set(idx, true);
            Some(self.index_as_frame(idx))
        } else {
            None
        }
    }
}

impl<'a> FrameDeallocator<Size4KiB> for BitmapFrameAllocator<'a> {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        if let Some(idx) = self.frame_as_index(frame) {
            self.bitmap.lock().set(idx, false);
        } else {
            // We will panic for now, but eventually handle this more gracefully.
            todo!("Attempted to deallocate frame that was not allocated by the allocator");
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct AddressRange {
    start: u64,
    end: u64,
}

fn ranges_intersect(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    // True if the intervals (a_start..a_end) and (b_start..b_end) overlap
    // Overlap means they have some common address:
    //   a_start < b_end && a_end > b_start
    //   (assuming a_start < a_end and b_start < b_end)
    a_start < b_end && a_end > b_start
}

fn phys_to_virt(phys: u64, offset: u64) -> u64 {
    phys + offset
}

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

//Legacy Frame Allocator
pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    /*
    Create a FrameAllocator from a given memory map. Must be marked unsafe
    because the caller has to guarantee the passed memory map is valid.
    All frames marked 'USABLE' in the map must be really unused.
     */
    pub unsafe fn init(memory_map: &'static MemoryRegions) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    // Return an iterator over usable frames from a given map
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
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

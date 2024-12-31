use crate::allocator::alloc_info::AllocationInfo;
use crate::allocator::alloc_info::LARGE_ALLOCS;
use crate::memory::PAGE_SIZE;
use crate::println;

use super::page_allocator::PageAllocator;
use super::page_allocator::PAGE_ALLOCATOR;
use super::Locked;
use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use core::mem;
use core::ptr;
use x86_64::structures::paging::FrameAllocator;
use x86_64::structures::paging::FrameDeallocator;
use x86_64::structures::paging::Mapper;
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::Size4KiB;

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
const MAX_LIST_LENGTH: usize = 4096;

struct ListNode {
    next: Option<&'static mut ListNode>,
}

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    list_lengths: [usize; BLOCK_SIZES.len()],
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            list_lengths: [0; BLOCK_SIZES.len()],
        }
    }

    pub unsafe fn init(
        &mut self,
        page_allocator: &mut PageAllocator<
            impl Mapper<Size4KiB>,
            impl FrameAllocator<Size4KiB> + FrameDeallocator<Size4KiB>,
        >,
    ) {
        // Let's say we want to pre-allocate a page or two for small blocks
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        if let Ok(start_addr) = page_allocator.alloc(/* num_pages = */ 1, flags) {
            println!("Initializing FixedSizeBlockAllocator");
            let page_size = 4096;
            // We'll fill as many 8-byte blocks as we can in this single page
            let block_size = 8;
            let num_blocks = page_size / block_size;

            let mut current_addr = start_addr;
            for _ in 0..num_blocks {
                let node_ptr = current_addr as *mut ListNode;
                (*node_ptr).next = self.list_heads[0].take(); // index 0 => 8-byte blocks
                self.list_heads[0] = Some(&mut *node_ptr);
                current_addr += block_size;
            }
            println!("FixedSizeBlockAllocator initialized");
        }
    }

    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());
        let num_pages = (size + ((PAGE_SIZE as usize) - 1)) / (PAGE_SIZE as usize);

        let mut guard = PAGE_ALLOCATOR.lock();
        println!(
            "Falling back to page allocator for size {} - main fallback alloc function",
            size
        );
        if let Some(ref mut page_alloc) = *guard {
            if let Ok(addr) = page_alloc.alloc(
                num_pages,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            ) {
                let mut map = LARGE_ALLOCS.lock();
                map.insert(addr, AllocationInfo { num_pages });

                return addr as *mut u8;
            }
        }
        ptr::null_mut()
    }
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    ///
    ///     Allocates memory with a given layout using a segregated free list or fallback allocator.

    ///     ## Steps:
    ///     1. Lock the allocator for a mutable reference.
    ///     2. Determine block size via `list_index`.
    ///        - If `None`, use `fallback_alloc`.
    ///     3. If a valid index exists:
    ///        - Pop the first node from `list_heads[index]` using `Option::take`.
    ///        - If a node is available, update the list head and return the node as a raw pointer.
    ///        - If empty, allocate a new block with `BLOCK_SIZES[index]` for size/alignment, create a `Layout`, and use `fallback_alloc`.
    ///     4. Allocations greater than the largest block size in BLOCK_SIZES will be handed to the PageAllocator.

    ///     ## Safety:
    ///     - Marked `unsafe` due to raw pointer manipulation, necessitates on correct allocator use.
    ///
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                match allocator.list_heads[index].take() {
                    Some(node) => {
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    None => {
                        // If no block of the required size is available, allocate a new block
                        let block_size = BLOCK_SIZES[index];
                        // Ensure that the block size is multiple of the layout's alignment
                        let block_align = block_size;
                        let layout = Layout::from_size_align(block_size, block_align).unwrap();
                        println!(
                            "Falling back to page allocator for size {} - alloc",
                            block_size
                        );
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            None => {
                println!(
                    "Falling back to page allocator for size {} - alloc2",
                    layout.size()
                );
                allocator.fallback_alloc(layout)
            }
        }
    }

    ///
    ///     Deallocates a memory block, returning it to the segregated free list or fallback allocator.

    ///     ## Steps:
    ///     1. Lock the allocator.
    ///     2. Determine block size via `list_index`.
    ///        - If `None`, deallocate with `fallback_allocator` using a `NonNull` pointer.
    ///     3. If a valid index exists:
    ///        - Create a new `ListNode` pointing to `list_heads[index]`.
    ///        - Assert block size supports a `ListNode` with proper alignment.
    ///        - Write the `ListNode` to the memory block and update the list head.
    ///     4. Aligns and sizes blocks.

    ///     - Blocks from `fallback_alloc` are returned to it, while segregated blocks grow their respective lists as needed.

    ///     ## Safety:
    ///     - `unsafe` for raw pointer manipulation and memory management. Validates alignment and size before writes.
    ///

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();

        // figure out if it's small or large
        if let Some(index) = list_index(&layout) {
            // This is a small block
            if allocator.list_lengths[index] < MAX_LIST_LENGTH {
                // push it onto the free list
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);

                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
                allocator.list_lengths[index] += 1;
            } else {
                // a small block but the free list is at capacity
                // If we're at capacity, just leak this block (for now)
                println!(
                    "Warning: free list for block size {} is at capacity, leaking block ptr=0x{:x}",
                    BLOCK_SIZES[index], ptr as usize
                );
            }
        } else {
            // Large allocation => look up `ptr` in the map and deallocate
            let mut map = LARGE_ALLOCS.lock();
            let start_addr = ptr as usize;
            let info = map
                .remove(&start_addr)
                .expect("ERROR: Attempted to free an allocation that was not found in the map!");
            let num_pages = info.num_pages;

            let mut guard = PAGE_ALLOCATOR.lock();
            if let Some(ref mut page_alloc) = *guard {
                page_alloc
                    .dealloc(start_addr, num_pages)
                    .expect("dealloc failed");
            }
        }
    }
}

fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

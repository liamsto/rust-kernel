use super::page_allocator::PAGE_ALLOCATOR;
use super::Locked;
use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use core::ptr;
use core::{mem, ptr::NonNull};
use x86_64::structures::paging::PageTableFlags;

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
const INITIAL_BLOCKS_PER_SIZE: usize = 16;
const MAX_LIST_LENGTH: usize = 1024;

struct ListNode {
    next: Option<&'static mut ListNode>,
}

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    list_lengths: [usize; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            list_lengths: [0; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);

        /*
            Custom optimization: pre-allocate some blocks
            Instead of waiting until the first allocation request to populate the
            free lists (which causes an immediate fallback to the slow allocator),
            pre-allocate a certain number of blocks of each size during initialization.
            This reduces latency for the initial allocations.
        */
        for (index, &block_size) in BLOCK_SIZES.iter().enumerate() {
            let layout = Layout::from_size_align(block_size, block_size).unwrap();
            for _ in 0..INITIAL_BLOCKS_PER_SIZE {
                let ptr = match self.fallback_allocator.allocate_first_fit(layout) {
                    Ok(allocation) => allocation.as_ptr(),
                    Err(_) => break, // If we run out of memory early, just stop.
                };
                let node = ListNode {
                    next: self.list_heads[index].take(),
                };
                let node_ptr = ptr as *mut ListNode;
                node_ptr.write(node);
                self.list_heads[index] = Some(&mut *node_ptr);
            }
        }
    }

    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());
        let num_pages = (size + 4095) / 4096;

        let mut guard = PAGE_ALLOCATOR.lock();

        if let Some(ref mut page_alloc) = *guard {
            if let Ok(addr) = page_alloc.alloc(
                num_pages,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            ) {
                return addr as *mut u8;
            }
        }
        ptr::null_mut()
    }
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    /*
        Allocates memory with a given layout using a segregated free list or fallback allocator.

        Steps:
        1. Lock the allocator for a mutable reference.
        2. Determine block size via `list_index`.
           - If `None`, use `fallback_alloc`.
        3. If a valid index exists:
           - Pop the first node from `list_heads[index]` using `Option::take`.
           - If a node is available, update the list head and return the node as a raw pointer.
           - If empty, allocate a new block with `BLOCK_SIZES[index]` for size/alignment, create a `Layout`, and use `fallback_alloc`.
        4. Allocations greater than the largest block size in BLOCK_SIZES will be handed to the PageAllocator.

        Safety:
        - Marked `unsafe` due to raw pointer manipulation, necessitates on correct allocator use.
    */
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
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }

    /*
        Deallocates a memory block, returning it to the segregated free list or fallback allocator.

        Steps:
        1. Lock the allocator.
        2. Determine block size via `list_index`.
           - If `None`, deallocate with `fallback_allocator` using a `NonNull` pointer.
        3. If a valid index exists:
           - Create a new `ListNode` pointing to `list_heads[index]`.
           - Assert block size supports a `ListNode` with proper alignment.
           - Write the `ListNode` to the memory block and update the list head.
        4. Aligns and sizes blocks.

        - Blocks from `fallback_alloc` are returned to it, while segregated blocks grow their respective lists as needed.

        Safety:
        - `unsafe` for raw pointer manipulation and memory management. Validates alignment and size before writes.
    */

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        if let Some(index) = list_index(&layout) {
            if allocator.list_lengths[index] < MAX_LIST_LENGTH {
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
                // If at max capacity, free block via fallback instead
                let nn = NonNull::new(ptr).unwrap();
                allocator.fallback_allocator.deallocate(nn, layout);
            }
        } else {
            let nn = NonNull::new(ptr).unwrap();
            allocator.fallback_allocator.deallocate(nn, layout);
        }
    }
}

fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

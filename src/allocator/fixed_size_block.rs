use super::Locked;
use alloc::alloc::GlobalAlloc;
use alloc::alloc::Layout;
use core::ptr;
use core::{mem, ptr::NonNull};

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];
const INITIAL_BLOCKS_PER_SIZE: usize = 16;
struct ListNode {
    next: Option<&'static mut ListNode>,
}

pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);

        // custom optimization: pre-allocate some blocks
        /*
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
                let node = ListNode { next: self.list_heads[index].take() };
                let node_ptr = ptr as *mut ListNode;
                node_ptr.write(node);
                self.list_heads[index] = Some(&mut *node_ptr);
            }
        }
    }

    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }

}



unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    /*
        Allocates a memory block with the given layout using a segregated free list or a fallback allocator.

        Steps:
        1. Acquire a mutable reference to the allocator instance using the `lock` method.
        2. Determine the appropriate block size for the layout using `list_index`.
        - If no suitable block size exists (index is `None`), fall back to the `fallback_alloc` method.
        3. If a valid block size index is found:
        - Attempt to retrieve the first node in the corresponding list (`list_heads[index]`) using `Option::take`.
        - If a node is available (`Some(node)`), update the list head to point to the next node and return the popped node as a raw pointer (`*mut u8`).
        - If the list is empty (`None`), allocate a new block:
            - Use the block size from `BLOCK_SIZES[index]` as both size and alignment.
            - Create a new `Layout` with the adjusted size and alignment.
            - Perform the allocation using `fallback_alloc` and return the result.
        4. Handles both small and large allocations by falling back to a general-purpose allocator when needed.

        Safety:
        - This function is marked `unsafe` as it interacts with raw pointers and relies on proper usage of the allocator to avoid undefined behavior.
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
        Deallocates a memory block, returning it to the segregated free list or the fallback allocator.

        Steps:
        1. Acquire a mutable reference to the allocator instance using the `lock` method.
        2. Determine the appropriate block size for the layout using `list_index`.
        - If no fitting block size exists (`None`), the memory was allocated using the fallback allocator:
            - Convert the pointer (`*mut u8`) to a `NonNull` type and use the `fallback_allocator` to deallocate it.
        3. If a valid block size index is found:
        - Create a new `ListNode` pointing to the current list head (`list_heads[index]`).
        - Verify that the block size is sufficient to hold a `ListNode` with correct alignment using `assert!`.
        - Convert the raw pointer (`*mut u8`) to a `*mut ListNode` and write the new `ListNode` into the memory block.
        - Update the head of the list to point to the newly created node.
        4. Ensures that blocks returned to the free list are properly aligned and sized for future reuse.

        Notes:
        - Blocks allocated by `fallback_alloc` are deallocated back to the fallback allocator.
        - Blocks allocated via the segregated free list are returned to the corresponding list, growing its capacity over time.
        - This lazy approach initializes block lists empty and fills them only when allocations of specific sizes are requested.

        Safety:
        - This function is marked `unsafe` due to its use of raw pointers and reliance on correct memory management practices.
        - Ensures alignment and size compatibility before performing writes to memory.
    */

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // Ensure that the new node is properly aligned and is of the correct size
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
            None => {
                let ptr = NonNull::new(ptr).unwrap();
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}

fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

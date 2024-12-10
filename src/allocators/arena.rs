use spin::Mutex;

use crate::allocators::bin::Bin;

use super::chunk::ChunkManager;

const BIN_COUNT: usize = 16;
pub struct Arena {
    bins: [Mutex<Bin>; BIN_COUNT],
    chunk_manager: ChunkManager,
}

impl Arena {
    pub fn new(chunk_manager: ChunkManager) -> Self {
        // Initialize all bins with size classes.
        Self {
            bins: core::array::from_fn(|i| Mutex::new(Bin::new(i))), // Initialize each bin with its size class parameters.
            chunk_manager,
            // stats: ArenaStats::new(),
        }
    }

    pub fn alloc(&self, size: usize, align: usize) -> *mut u8 {
        // 1. Determine bin/size class from `size`.
        // 2. Lock the arena or bin as needed.
        // 3. Delegate to the appropriate bin.
        // 4. If bin requires new run, request from chunk_manager.
        // 5. Return pointer.
        todo!()
    }

    pub fn dealloc(&self, ptr: *mut u8) {
        // 1. Determine which run and bin this pointer belongs to.
        // 2. Mark the slot in the run as free.
        todo!()
    }
}


unsafe impl Send for Arena {}
unsafe impl  Sync for Arena {}
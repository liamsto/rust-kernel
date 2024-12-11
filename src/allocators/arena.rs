use spin::Mutex;

use crate::allocators::bin::Bin;
use crate::allocators::chunk::ChunkManager;

// Determine how to map sizes to bins.
const BIN_COUNT: usize = 16;

pub struct Arena {
    bins: [Mutex<Bin>; BIN_COUNT],
    chunk_manager: Mutex<ChunkManager>,
}

impl Arena {
    pub fn new(chunk_manager: ChunkManager) -> Self {
        // Testing initialization: each bin i corresponds to a size class.
        // Will eventually have a proper mapping from requested size to bin index.
        Self {
            bins: core::array::from_fn(|i| Mutex::new(Bin::new((i + 1) * 16))),
            chunk_manager: Mutex::new(chunk_manager),
        }
    }

    pub fn alloc(&self, size: usize, align: usize) -> *mut u8 {
        // 1. Determine bin index from `size` (for now - just pick the smallest bin that can fit `size`).
        let bin_index = self.size_to_bin_index(size);
        let mut bin = self.bins[bin_index].lock();

        // 2. Try to allocate from bin. If bin needs a run, it'll call a helper method that uses chunk_manager.
        bin.alloc(&self.chunk_manager, size, align)
            .unwrap_or(core::ptr::null_mut())
    }

    pub fn dealloc(&self, ptr: *mut u8) {
        // 1. Determine which bin/run this pointer belongs to.
        // Need to implement metadata first.
        // For now, just a placeholder.
        todo!()
    }

    fn size_to_bin_index(&self, size: usize) -> usize {
        // Placeholder logic: find first bin whose object_size >= size
        // A real system might have a precomputed lookup table.
        for (i, bin_lock) in self.bins.iter().enumerate() {
            let bin = bin_lock.lock();
            if bin.object_size() >= size {
                return i;
            }
        }
        BIN_COUNT - 1
    }
}

unsafe impl Send for Arena {}
unsafe impl Sync for Arena {}

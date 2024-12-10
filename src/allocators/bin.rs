use core::ptr::NonNull;

use alloc::vec::Vec;

use super::run::Run;

pub struct Bin {
    object_size: usize,
    runs: Vec<NonNull<Run>>,
}

impl Bin {
    pub fn new(object_size: usize) -> Bin {
        Self {
            object_size,
            runs: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> Option<*mut u8> {
        // 1. Find a run with a free slot.
        // 2. If none found, request a new run from arena.
        // 3. Return the allocated object pointer.
        unimplemented!()
    }

    pub fn dealloc(&mut self, ptr: *mut u8) {
        // 1. Identify the run that `ptr` belongs to.
        // 2. Free the object in the run.
        // 3. Potentially move run between empty/partial lists.
        unimplemented!()
    }

    pub fn add_run(&mut self, run: NonNull<Run>) {
        // Add run to the runs vector and possibly mark it as available.
        unimplemented!()
    }

    pub fn contains(&self, ptr: *mut u8) -> bool {
        // Check if ptr belongs to any run of this bin. This method may be unnecessary, depending on how I end up implementing metadata.
        unimplemented!()
    }
}

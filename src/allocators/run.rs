pub struct Run {
    // Pointer to the start of the run.
    start: *mut u8,
    // Size of each object in the run.
    object_size: usize,
    // Number of objects this run can hold.
    num_objects: usize,
    // Free bitmap. Each bit corresponds to an object in the run.
    free_bitmap: [u64; 4], // small bitmap for now
    // Number of free objects in the run.
    free_count: usize,
}

impl Run {
    pub fn new(start: *mut u8, object_size: usize, num_objects: usize) -> Self {
        Self {
            start,
            object_size,
            num_objects,
            free_bitmap: [0xFFFF_FFFF_FFFF_FFFF; 4], // initially all free
            free_count: num_objects,
        }
    }

    /// Allocate one object from this run.
    pub fn alloc(&mut self) -> Option<*mut u8> {
        // 1. Find a free bit in free_bitmap.
        // 2. Mark it allocated.
        // 3. Compute object address: start + (index * object_size).
        // 4. Return pointer.
        unimplemented!()
    }

    /// Free an object back to this run.
    pub fn dealloc(&mut self, ptr: *mut u8) {
        // 1. Compute index from ptr: (ptr - start) / object_size.
        // 2. Set the corresponding bit in free_bitmap.
        // 3. Increment free_count.
        unimplemented!()
    }

    /// Check if this run owns the given pointer.
    pub fn contains(&self, ptr: *mut u8) -> bool {
        let run_start = self.start as usize;
        let run_end = run_start + (self.num_objects * self.object_size);
        let addr = ptr as usize;
        addr >= run_start && addr < run_end
    }
}

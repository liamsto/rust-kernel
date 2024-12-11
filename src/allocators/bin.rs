use alloc::vec::Vec;
use core::ptr::NonNull;
use spin::Mutex;

use crate::allocators::chunk::ChunkManager;
use crate::allocators::run::Run;

pub struct Bin {
    object_size: usize,
    runs: Vec<NonNull<Run>>,
}

impl Bin {
    pub fn new(object_size: usize) -> Self {
        Self {
            object_size,
            runs: Vec::new(),
        }
    }

    pub fn object_size(&self) -> usize {
        self.object_size
    }

    pub fn alloc(
        &mut self,
        chunk_manager: &Mutex<ChunkManager>,
        _size: usize,
        _align: usize,
    ) -> Option<*mut u8> {
        // Try each run to find a free slot
        for run_ptr in &mut self.runs {
            let run = unsafe { run_ptr.as_mut() };
            if let Some(ptr) = run.alloc() {
                return Some(ptr);
            }
        }

        // No free slot found, request a new run
        self.add_run(chunk_manager)?;

        // Try again after adding a run
        self.alloc(chunk_manager, _size, _align)
    }

    fn add_run(&mut self, chunk_manager: &Mutex<ChunkManager>) -> Option<()> {
        let page_size = 4096;
        // Determine how large a run should be. For simplicity, let's say one run = one page.
        let run_size = page_size;

        let mut cm = chunk_manager.lock();
        let ptr = cm.allocate_chunk(run_size)?; // allocate_chunk returns an Option<NonNull<u8>>

        // Create a new Run over that memory.
        // Determine how many objects fit in a run. Example: run_size / object_size.
        let num_objects = run_size / self.object_size;

        let run_raw = ptr.as_ptr() as *mut Run;
        unsafe {
            run_raw.write(Run::new(ptr.as_ptr(), self.object_size, num_objects));
        }

        self.runs.push(unsafe { NonNull::new_unchecked(run_raw) });
        Some(())
    }
}

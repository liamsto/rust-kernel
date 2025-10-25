//! This module contains the data structures and functions for tracking allocation information.
use spin::RwLock;

/// Contains information about a large allocation.
///
/// This allows us to track the number of pages allocated, making it easier to deallocate.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AllocationInfo {
    pub num_pages: usize,
}

// lazy_static! {
//     /// A map of large allocations to their respective `AllocationInfo`.
//     pub static ref LARGE_ALLOCS: RwLock<[Option<(usize, AllocationInfo)>; 512]> = [None; 512].into();
// }

pub static LARGE_ALLOCS: RwLock<[Option<(usize, AllocationInfo)>; 512]> = RwLock::new([None; 512]);

/// Inserts a large allocation into the `LARGE_ALLOCS` map.
pub fn large_alloc_insert(addr: usize, info: AllocationInfo) {
    let mut large_allocs = LARGE_ALLOCS.write();
    for slot in large_allocs.iter_mut() {
        if slot.is_none() {
            *slot = Some((addr, info));
            return;
        }
    }
    panic!("LARGE_ALLOCS is full!");
}

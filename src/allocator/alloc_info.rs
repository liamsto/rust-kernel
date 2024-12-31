//! This module contains the data structures and functions for tracking allocation information.
use alloc::collections::BTreeMap;
use lazy_static::lazy_static;
use spin::Mutex;

/// Contains information about a large allocation.
///
/// This allows us to track the number of pages allocated, making it easier to deallocate.
#[repr(C)]
pub struct AllocationInfo {
    pub num_pages: usize,
}

lazy_static! {
    /// A map of large allocations to their respective `AllocationInfo`.
    pub static ref LARGE_ALLOCS: Mutex<BTreeMap<usize, AllocationInfo>> = Mutex::new(BTreeMap::new());
}

use arena::Arena;

pub mod arena;
pub mod bin;
pub mod run;
pub mod chunk;
pub mod metadata;

static GLOBAL_ARENA: spin::Once<Arena> = spin::Once::new();

pub fn init_global_allocator(
    mapper: &'static mut dyn x86_64::structures::paging::Mapper<x86_64::structures::paging::Size4KiB>,
    frame_allocator: &'static mut dyn x86_64::structures::paging::FrameAllocator<x86_64::structures::paging::Size4KiB>,
) {
    let chunk_manager = chunk::ChunkManager::new(mapper, frame_allocator);
    GLOBAL_ARENA.call_once(|| Arena::new(chunk_manager));
}

pub fn alloc(size: usize, align: usize) -> *mut u8 {
    GLOBAL_ARENA.get().unwrap().alloc(size, align)
}

pub fn dealloc(ptr: *mut u8) {
    GLOBAL_ARENA.get().unwrap().dealloc(ptr)
}
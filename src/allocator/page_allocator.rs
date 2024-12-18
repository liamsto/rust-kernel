use core::arch::x86_64::_rdrand64_step;

use x86_64::structures::paging::{FrameAllocator, Size4KiB};

// Will eventually be replaced with ASLR
pub const KERNEL_HEAP_START: usize = 0xFFFF_FF00_0000_0000;
pub const KERNEL_HEAP_SIZE: usize = 0x2000_0000; // 512MB heap for now
pub const KERNEL_HEAP_END: usize = KERNEL_HEAP_START + KERNEL_HEAP_SIZE;


pub struct PageAllocator {
    frames: &'static dyn FrameAllocator<Size4KiB>,
    current_virt: usize,
    end_virt: usize,
}

impl PageAllocator {
    pub const fn new (frames: &'static dyn FrameAllocator<Size4KiB>, start_virt: usize, end_virt: usize) -> Self {
        PageAllocator {
            frames,
            current_virt: start_virt,
            end_virt,
        }
    }

    fn init_start_aslr(&mut self) {
        let mut rng = 0u64;
        unsafe {
            _rdrand64_step(&mut rng);
        }
        self.current_virt = KERNEL_HEAP_START + (rng as usize % KERNEL_HEAP_SIZE);


    }
}
use crate::interrupts::PHYSICAL_MEMORY_OFFSET;

// This block tells the compiler that there are symbols, called _start_trampoline and _end_trampoline, that exist, and are u8.
unsafe extern "C" {
    unsafe static _start_trampoline: u8;
    unsafe static _end_trampoline: u8;
}

/// Loads the AP trampoline code into physical memory at address 0x8000.
pub unsafe fn load_ap_trampoline() {
    let trampoline_size = unsafe { &_end_trampoline } as *const u8 as usize
        - unsafe { &_start_trampoline } as *const u8 as usize;

    const TRAMPOLINE_PHYS: usize = 0x8000;
    let dest = (PHYSICAL_MEMORY_OFFSET + TRAMPOLINE_PHYS) as *mut u8;

    let src = unsafe { &_start_trampoline } as *const u8;

    unsafe { core::ptr::copy_nonoverlapping(src, dest, trampoline_size) };
}

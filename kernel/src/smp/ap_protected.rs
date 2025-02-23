use crate::interrupts::PHYSICAL_MEMORY_OFFSET;

/// Reference to the .bin file created from the assembling of ap_trampoline.asm. Running `cargo build` should produce a .bin file  in the smp directory, which this variable  will reference using `include_bytes!()`
pub static AP_TRAMPOLINE_BIN: &[u8] = include_bytes!("ap_trampoline.bin");

/// Loads the AP trampoline code into physical memory at address 0x8000.
pub unsafe fn load_ap_trampoline() {
    let trampoline_size = AP_TRAMPOLINE_BIN.len();
    const TRAMPOLINE_PHYS: usize = 0x8000;
    let dest = (PHYSICAL_MEMORY_OFFSET + TRAMPOLINE_PHYS) as *mut u8;
    unsafe { core::ptr::copy_nonoverlapping(AP_TRAMPOLINE_BIN.as_ptr(), dest, trampoline_size) };
}

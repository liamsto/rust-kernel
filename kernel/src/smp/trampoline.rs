pub const TRAMPOLINE_BASE: usize = 0x8000; // physical base of the trampoline

// Offsets within the trampoline's data (from its start at 0x8000)
pub const CR3VAL_OFFSET: usize = 0;         // 4 bytes
pub const KCODE_OFFSET: usize  = 8;         // 8 bytes (u64)
pub const KSTACK_OFFSET: usize = 16;        // 8 bytes (u64)
pub const KGSVAL_OFFSET: usize = 24;        // 8 bytes (u64)
pub const COMMWORD_OFFSET: usize = 32;      // 4 bytes

use core::arch::asm;
use core::sync::atomic::Ordering;

use crate::init::hpet::HPET_BASE;
use crate::interrupts::PHYSICAL_MEMORY_OFFSET;
use crate::init::multicore::{ap_startup, AP_STACKS, AP_STACK_INDEX, NUM_AP_STACKS};
use crate::serial_println;
use crate::timer::get_current_time_us;

pub static AP_TRAMPOLINE_BIN: &[u8] = include_bytes!("ap_trampoline.bin");

/// Loads the AP trampoline code into physical memory at TRAMPOLINE_BASE.
pub unsafe fn load_ap_trampoline() {
    let trampoline_size = AP_TRAMPOLINE_BIN.len();
    let dest = (PHYSICAL_MEMORY_OFFSET + TRAMPOLINE_BASE) as *mut u8;
    unsafe { core::ptr::copy_nonoverlapping(AP_TRAMPOLINE_BIN.as_ptr(), dest, trampoline_size) };
}

/// Patches the trampoline's data fields with values from the BSP.
pub unsafe fn patch_trampoline() {
    let tramp_ptr = (PHYSICAL_MEMORY_OFFSET + TRAMPOLINE_BASE) as *mut u8;
    // Patch CR3 (4 bytes)
    let cr3: u64 = unsafe { read_cr3() };
    unsafe {

        serial_println!("CR3: {:#x}", read_cr3());
        *(tramp_ptr.add(CR3VAL_OFFSET) as *mut u64) = cr3;
    
        //Patch kernel entry pointer
        let ap_entry: u64 = ap_startup as usize as u64;
        serial_println!("Patching trampoline: ap_startup = {:#x}", ap_entry);
        *(tramp_ptr.add(KCODE_OFFSET) as *mut u64) = ap_entry;
        
        //Allocate an AP stack and patch the pointer
        let ap_stack: u64 = allocate_ap_stack(); 
        serial_println!("Patching trampoline: AP stack top = {:#x}", ap_stack);
        *(tramp_ptr.add(KSTACK_OFFSET) as *mut u64) = ap_stack;
        
        //Patch GS value if needed
        *(tramp_ptr.add(KGSVAL_OFFSET) as *mut u64) = 0;
        
        //clear commword
        *(tramp_ptr.add(COMMWORD_OFFSET) as *mut u32) = 0;    
    }
}

/// Wait for the AP to signal readiness by polling the commword.
pub unsafe fn wait_for_ap(timeout_us: u64) -> bool {
    let tramp_ptr = (PHYSICAL_MEMORY_OFFSET + TRAMPOLINE_BASE) as *const u8;
    let comm_ptr = unsafe { tramp_ptr.add(COMMWORD_OFFSET) } as *const u32;
    let start = unsafe { get_current_time_us(HPET_BASE) }; // You need a timer function.
    while unsafe { get_current_time_us(HPET_BASE) } - start < timeout_us {
        if unsafe { core::ptr::read_volatile(comm_ptr) } == 1 {
            return true;
        }
    }
    false
}


#[inline]
pub unsafe fn read_cr3() -> u64 {
    let value: u64;
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}


/// Allocates an AP stack and returns its top address (as a u64).
/// Each stack is a fixed-size block (32KB), and the top-of-stack is at the end of the array.
/// Panics if no more stacks are available.
pub unsafe fn allocate_ap_stack() -> u64 {
    let index = AP_STACK_INDEX.fetch_add(1, Ordering::Relaxed);
    if index >= NUM_AP_STACKS {
        panic!("Out of AP stacks!");
    }
    let stack = unsafe{&AP_STACKS[index]};
    let stack_ptr = stack.as_ptr() as usize;
    let stack_size = core::mem::size_of::<[u8; 32768]>();
    (stack_ptr + stack_size) as u64
}

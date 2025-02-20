use acpi::HpetInfo;

use crate::{interrupts::PHYSICAL_MEMORY_OFFSET, println};

pub static mut HPET_BASE: *mut u64 = core::ptr::null_mut();

//HPET registers, in bytes
const HPET_CAPS_OFFSET: usize = 0x0;
const HPET_CONFIG_OFFSET: usize = 0x10;
const HPET_COUNTER_OFFSET: usize = 0xF0;

pub fn init_hpet(hpet_info: &HpetInfo) {
    let virt_addr = hpet_info.base_address + PHYSICAL_MEMORY_OFFSET;
    unsafe {
        HPET_BASE = virt_addr as *mut u64;
        let caps = core::ptr::read_volatile(HPET_BASE.add(HPET_CAPS_OFFSET / 8));
        println!("HPET capabilities: {:#x}", caps);
        println!("HPET clock tick unit: {} fs", hpet_info.clock_tick_unit);
    
        // Enable the HPET by writing to the config register
        let config_ptr = HPET_BASE.add(HPET_CONFIG_OFFSET / 8);
        core::ptr::write_volatile(config_ptr, 1); // set the enable bit
        let config = core::ptr::read_volatile(config_ptr);
        println!("HPET config register: {:#x}", config);
    
        // Optionally, check the main counter once
        let main_counter = core::ptr::read_volatile(HPET_BASE.add(HPET_COUNTER_OFFSET / 8));
        println!("Initial HPET main counter: {}", main_counter);
    }
}


/// Reads the clock tick unit from the HPET capabilities register as a fallback.
pub unsafe fn get_clock_tick_unit_fallback(hpet_base: *const u64) -> u32 {
    // Read the capabilities register (offset 0)
    let caps = unsafe { core::ptr::read_volatile(hpet_base) };
    // Bits 32-63 contain the tick period in femtoseconds.
    (caps >> 32) as u32
}

use acpi::HpetInfo;

use crate::{interrupts::PHYSICAL_MEMORY_OFFSET, println};

pub static mut HPET_BASE: *mut u64 = core::ptr::null_mut();

//HPET registers, in bytes
const HPET_CAPS_OFFSET: usize = 0x0;    // Capabilities and ID
const HPET_CONFIG_OFFSET: usize = 0x10; // General Configuration
const HPET_COUNTER_OFFSET: usize = 0xF0;    // Main Counter


pub fn init_hpet(hpet_info: &HpetInfo) {
    let virt_addr = hpet_info.base_address + PHYSICAL_MEMORY_OFFSET;
    
    unsafe {
        // Store the base of the HPET globally so that our timer functions can use it later on
        HPET_BASE = virt_addr as *mut u64;

        let caps = core::ptr::read_volatile(HPET_BASE.add(HPET_CAPS_OFFSET/8));
        println!("[INFO] HPET Capabilties: {:#x}", caps);
        println!("HPET clock tick unit: {} femtoseconds", hpet_info.clock_tick_unit);

        // Turn on the HPET - write to General Configuration.
        // Bit 0 should be set.
        let config_ptr = HPET_BASE.add(HPET_CONFIG_OFFSET / 8);
        let mut _config = core::ptr::read_volatile(config_ptr);
        _config |= 1; // set enable
        println!("[INFO] HPET Enabled");

        // print the main counter
        let main_counter = core::ptr::read_volatile(HPET_BASE.add(HPET_COUNTER_OFFSET / 8));
        println!("Initial HPET main counter value: {}", main_counter);
    }

}
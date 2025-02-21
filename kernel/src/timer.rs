use crate::{init::hpet::get_clock_tick_unit_fallback, println};

/// Delay for the given number of milliseconds using HPET.
/// Assumes the HPET registers are already mapped at `hpet_base`.
///
/// `clock_tick_unit` is given in femtoseconds (fs) per tick.
pub unsafe fn delay_ms(hpet_base: *const u64, mut clock_tick_unit: u32, ms: u64) {
    if clock_tick_unit == 0 {
        clock_tick_unit = unsafe { get_clock_tick_unit_fallback(hpet_base) } as u32;
        if clock_tick_unit == 0 {
            panic!("HPET clock tick unit is still zero!");
        }
    }
    println!("Using clock tick unit {}", clock_tick_unit);
    let main_counter_ptr = unsafe { hpet_base.add(0xF0 / 8) };
    let start = unsafe { core::ptr::read_volatile(main_counter_ptr) };

    // 1 millisecond = 1e12 femtoseconds.
    let delay_fs = ms * 1_000_000_000_000;
    // Compute the number of ticks to wait.
    let ticks_to_wait = delay_fs / clock_tick_unit as u64;
    let target = start.wrapping_add(ticks_to_wait);

    // Spin until the main counter reaches the target.
    while unsafe { core::ptr::read_volatile(main_counter_ptr) } < target {
        core::hint::spin_loop();
    }
}
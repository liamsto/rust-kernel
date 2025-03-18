use acpi::platform::{ProcessorInfo, ProcessorState};

pub unsafe fn init_smp(
    lapic_base: *mut u32,
    processor_info: &ProcessorInfo<'_, alloc::alloc::Global>,
) {
    let trampoline_vector = 0x8; // since 0x8000/0x1000 = 8

    // Patch and load the trampoline into low memory.
    unsafe {
        patch_trampoline();
        load_ap_trampoline();
    }

    // For each AP (skipping the BSP), send INIT/SIPI.
    for ap in processor_info.application_processors.iter() {
        if ap.state == ProcessorState::WaitingForSipi {
            serial_println!("Sending INIT IPI.");

            unsafe {
                send_init_ipi(lapic_base, ap.local_apic_id);
                delay_ms(HPET_BASE, 10);
    
                serial_println!("Sending Startup IPI.");
                send_startup_ipi(lapic_base, ap.local_apic_id, trampoline_vector);
                serial_println!("balls");
                delay_us(HPET_BASE, 200);
            }

            // Compute pointer to the trampoline's communication word.
            let tramp_comm_ptr = (crate::interrupts::PHYSICAL_MEMORY_OFFSET
                + crate::smp::trampoline::TRAMPOLINE_BASE
                + crate::smp::trampoline::COMMWORD_OFFSET)
                as *const u32;

            // Poll for the AP to signal readiness.
            if unsafe { wait_for_ap(HPET_BASE, tramp_comm_ptr, 100_000) } {
                serial_println!("AP {} started.", ap.local_apic_id);
            } else {
                serial_println!("AP {} did not start in time.", ap.local_apic_id);
                // Optionally, send another SIPI here.
            }
        }
    }
}

/// Sends an INIT IPI to the target AP.
pub unsafe fn send_init_ipi(lapic_base: *mut u32, apic_id: u32) {
    unsafe {
        // Clear APIC errors (@ offset 0x280)
        core::ptr::write_volatile(lapic_base.add(0x280 / 4), 0);
        // Set the target APIC ID in the ICR high register (offset 0x310)
        let icr_high = lapic_base.add(0x310 / 4);
        let current = core::ptr::read_volatile(icr_high);
        core::ptr::write_volatile(icr_high, (current & 0x00FF_FFFF) | ((apic_id as u32) << 24));

        // Send INIT IPI by writing to ICR low (offset 0x300)
        let icr_low = lapic_base.add(0x300 / 4);
        core::ptr::write_volatile(icr_low, 0x0000_4500);

        // maybe wait until delivery status is cleared?
    }
}

/// Sends a Startup IPI (SIPI) to the target AP.
/// `vector` is the wherever the asm "trampoline" physical page is (if trampoline is at 0x8000, then vector = 0x8).
pub unsafe fn send_startup_ipi(lapic_base: *mut u32, apic_id: u32, vector: u8) {
    unsafe {
        // Clear APIC errors
        core::ptr::write_volatile(lapic_base.add(0x280 / 4), 0);
        serial_println!("APIC Errors cleared.");

        // Set target APIC ID
        let icr_high = lapic_base.add(0x310 / 4);
        let current = core::ptr::read_volatile(icr_high);
        core::ptr::write_volatile(icr_high, (current & 0x00FF_FFFF) | ((apic_id as u32) << 24));
        serial_println!("Wrote ICR to APIC.");

        // Send SIPI: vector (in lower 8 bits) ORed with 0x600
        let icr_low = lapic_base.add(0x300 / 4);
        core::ptr::write_volatile(icr_low, (vector as u32) | 0x0000_4600);
        serial_println!("Wrote SIPI!");
    }
}

pub unsafe fn wait_for_ap(
    hpet_base: *const u64,
    comm_ptr: *const u32,
    timeout_us: u64,
) -> bool {
    let start = unsafe { get_current_time_us(hpet_base) };
    loop {
        if unsafe {core::ptr::read_volatile(comm_ptr) == 1} {
            return true;
        }
        if unsafe { get_current_time_us(hpet_base) } - start >= timeout_us {
            return false;
        }
        core::hint::spin_loop();
    }
}



use core::{arch::x86_64::_mm_pause, sync::atomic::AtomicUsize};

use crate::{serial_println, smp::trampoline::{load_ap_trampoline, patch_trampoline}, timer::{delay_ms, delay_us, get_current_time_us}};

use super::hpet::HPET_BASE;

#[unsafe(no_mangle)]
pub extern "C" fn ap_startup(_apic_id: i32) -> ! {
    // This function is called on each Application Processor (AP).
    // Perform per-core initialization here.
    // For now, we just loop
    serial_println!("hello");

    //initalize GDT
    crate::gdt::init();
    loop {
        unsafe {
            _mm_pause();
        }
    }
}

/// Allocate a block of memory for AP stacks.
/// Here we assume a maximum of 4 APs, each with a 32KB stack.
#[repr(align(16))]
pub struct Stack([u8; 32768]);

#[unsafe(no_mangle)]
pub static mut AP_STACKS: [Stack; 4] = [
    Stack([0; 32768]),
    Stack([0; 32768]),
    Stack([0; 32768]),
    Stack([0; 32768]),
];

pub static AP_STACK_INDEX: AtomicUsize = AtomicUsize::new(0);
pub const NUM_AP_STACKS: usize = 4;

impl Stack {
    pub fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
}

/// The symbol 'stack_top' is used by the assembly code to
/// set up the AP stack. Here we set it to the end of the AP_STACKS block.
#[unsafe(no_mangle)]
pub static mut STACK_TOP: u32 = 0;
#[unsafe(no_mangle)]
pub static mut BSPDONE: u8 = 0;
#[unsafe(no_mangle)]
pub static mut APPRUNNING: u8 = 0;

pub unsafe fn init_stack_top() {
    unsafe {
        STACK_TOP = (&raw const AP_STACKS as *const _ as u32)
            .wrapping_add(core::mem::size_of_val(&&raw const AP_STACKS) as u32)
    };
}

unsafe extern "C" {
    unsafe fn ap_init();
}

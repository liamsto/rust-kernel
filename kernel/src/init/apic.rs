use acpi::PlatformInfo;
use acpi::platform::interrupt::{InterruptModel, Polarity, TriggerMode};

use crate::apic_ptr::{APIC_BASE, u32_to_apic_ptr};
use crate::interrupts::{
    TIMER_VEC, disable_pic, enable_local_apic, init_apic_timer, map_apic_registers, map_io_apic,
    set_ioapic_redirect,
};
use crate::println;

pub fn init_apic(platform_info: &PlatformInfo<'_, alloc::alloc::Global>) {
    match &platform_info.interrupt_model {
        InterruptModel::Apic(apic_info) => {
            // 1) Map local APIC
            let mapped_ptr = map_apic_registers(apic_info.local_apic_address as u64);
            unsafe { APIC_BASE = Some(u32_to_apic_ptr(mapped_ptr)) };
            let local_apic_base = unsafe { &APIC_BASE.unwrap() };

            println!(
                "[INFO] APIC registers mapped to {:#?}",
                local_apic_base.as_ptr()
            );
            if apic_info.also_has_legacy_pics {
                disable_pic();
                println!("PIC Disabled.");
            }

            // 2) Enable local APIC and set up timer
            let apic_mmio = local_apic_base.as_ptr();
            println!("[INFO] APIC MMIO at {:?}", apic_mmio);
            unsafe {
                enable_local_apic(apic_mmio);
                init_apic_timer(apic_mmio, TIMER_VEC);
            }

            // 3) Map I/O APIC(s) and set up keyboard redirect
            for io_apic in apic_info.io_apics.iter() {
                println!(
                    "  IO APIC id={}, address={:#x}, GSI base={}",
                    io_apic.id, io_apic.address, io_apic.global_system_interrupt_base
                );
                map_io_apic(io_apic.address.try_into().unwrap());
                unsafe {
                    // GSI=1 => keyboard IRQ on IOAPIC with base=0
                    set_ioapic_redirect(
                        io_apic.address.try_into().unwrap(),
                        1,
                        0,
                        0x2F, // KEYBOARD_VEC
                        TriggerMode::Edge,
                        Polarity::ActiveHigh,
                    );
                }
            }

            // 4) Handle overrides, NMIs, etc.
        }
        _ => panic!("Non-APIC model!"),
    }
}

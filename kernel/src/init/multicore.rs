use acpi::platform::{ProcessorInfo, ProcessorState};

use crate::println;




pub fn init_multicore(ref processor_info: ProcessorInfo<'_, alloc::alloc::Global>) {
    let bsp_apic_id = processor_info.boot_processor.local_apic_id;
    println!("BSP local APIC ID: {}", bsp_apic_id);

    for ap in processor_info.application_processors.iter() {
        if ap.state != ProcessorState::Disabled {
            println!(
                "AP found: local_apic_id={}, processor_uid={}, state={:?}",
                ap.local_apic_id, ap.processor_uid, ap.state
            );
        }
    }
}
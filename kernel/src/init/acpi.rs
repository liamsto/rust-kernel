use crate::kernel_acpi::KernelAcpiHandler;
use crate::println;
use acpi::{AcpiTables, platform::PlatformInfo};
use bootloader_api::BootInfo;
use bootloader_api::info::Optional;

pub fn init_acpi(
    boot_info: &BootInfo,
) -> (
    AcpiTables<KernelAcpiHandler>,
    acpi::PlatformInfo<'_, alloc::alloc::Global>,
) {
    let rsdp_addr = match boot_info.rsdp_addr {
        Optional::Some(a) => a,
        Optional::None => panic!("RSDP address not provided by bootloader"),
    };
    println!("RSDP located at {:#x}", rsdp_addr);

    let acpi_handler = KernelAcpiHandler {};
    println!("ACPI handler created.");

    let tables = unsafe {
        AcpiTables::from_rsdp(acpi_handler, rsdp_addr.try_into().unwrap())
            .expect("Failed to parse ACPI tables")
    };
    let platform_info = PlatformInfo::new(&tables).unwrap();

    (tables, platform_info)
}

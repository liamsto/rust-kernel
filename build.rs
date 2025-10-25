use std::path::PathBuf;

fn main() {
    let target_dir =
        PathBuf::from(std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string()));
    let build_mode = std::env::var("PROFILE").unwrap(); // "debug" or "release"
    let output_dir = target_dir.join(build_mode);

    // Make sure the target directory exists
    std::fs::create_dir_all(&output_dir).expect("Failed to create target directory");
    // Locate kernel binary
    let kernel_bin_name = format!("CARGO_BIN_FILE_{}_{}", "RUST_KERNEL", "rust-kernel");
    let kernel = PathBuf::from(std::env::var_os(kernel_bin_name).expect("Kernel binary not found"));

    // UEFI and BIOS disk iamges
    let uefi_path = output_dir.join("uefi.img");
    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&uefi_path)
        .expect("Failed to create UEFI disk image");

    let bios_path = output_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel)
        .create_disk_image(&bios_path)
        .expect("Failed to create BIOS disk image");

    // paths for linking and runtime
    println!("cargo:rustc-link-search={}", output_dir.display());
    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
}

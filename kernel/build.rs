use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/smp/ap_trampoline.asm");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::create_dir_all(&out_dir).expect("Failed to create OUT_DIR");

    let asm_src = PathBuf::from("src/smp/ap_trampoline.asm");
    let asm_obj = out_dir.join("ap_trampoline.o");
    let bin_out = out_dir.join("ap_trampoline.bin");

    let status = Command::new("nasm")
        .args(["-f", "elf64"])
        .arg(&asm_src)
        .arg("-o")
        .arg(&asm_obj)
        .status()
        .expect("failed to run nasm");
    assert!(status.success(), "nasm failed");

    let status = Command::new("llvm-objcopy")
        .args(["-O", "binary"])
        .arg(&asm_obj)
        .arg(&bin_out)
        .status()
        .expect("failed to run objcopy");
    assert!(status.success(), "objcopy failed");

    println!("cargo:rustc-env=AP_TRAMPOLINE_BIN={}", bin_out.display());
}

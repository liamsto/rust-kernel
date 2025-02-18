fn main() {
    // read env variables set in build.rs
    //let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // let uefi = true;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.arg("-drive")
        .arg(format!("format=raw,file={bios_path}"));
    // pass additional args to QEMU, e.g.:
    cmd.args(["-serial", "stdio", "-cpu", "Skylake-Client"]);
    println!("Running QEMU with command: {:?}", cmd);

    let mut child = cmd.spawn().expect("Failed to launch QEMU");
    child.wait().unwrap();
}

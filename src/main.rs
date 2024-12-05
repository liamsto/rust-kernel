#![no_std]
#![no_main]

static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga = 0xb8000 as *mut u8;
    
    for(i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga.offset(i as isize * 2) = byte;
            *vga.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
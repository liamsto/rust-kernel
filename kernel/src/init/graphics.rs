use bootloader_api::BootInfo;
use bootloader_api::info::Optional;

pub fn init_framebuffer(boot_info: &mut BootInfo) {
    if let Optional::Some(ref mut fb) = boot_info.framebuffer {
        let info = fb.info();
        // Convert the mutable slice to have a 'static lifetime.
        let buffer: &'static mut [u8] = unsafe { core::mem::transmute(fb.buffer_mut()) };
        crate::framebuffer::init_framebuffer_writer(buffer, info);
    } else {
        panic!("No framebuffer available in BootInfo");
    }
}

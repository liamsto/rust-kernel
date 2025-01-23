use core::fmt::LowerHex;

use lazy_static::lazy_static;
use spin::Mutex;

/// A wrapper around the APIC MMIO pointer. Since raw pointers don't implement `Send` or `Sync`, we
/// need to wrap it in a type and manually implement those traits. This is safe because the APIC base
/// address is only written once and then never modified. We initalize it at boot time, store it, and
/// then never change it again.
pub struct ApicPtr {
    ptr: *mut u32,
}

unsafe impl Send for ApicPtr {}
unsafe impl Sync for ApicPtr {}

impl ApicPtr {
    pub fn new(ptr: *mut u32) -> Self {
        Self { ptr }
    }

    pub fn as_ptr(&self) -> *mut u32 {
        self.ptr
    }

    pub fn as_u64(&self) -> u64 {
        self.ptr as u64
    }
}

lazy_static! {
    /// A global holding the APIC MMIO pointer once it's mapped.
    pub static ref APIC_BASE: Mutex<Option<ApicPtr>> = Mutex::new(None);
}

/// Convert a u64 to an `ApicPtr`. This is useful for initializing the APIC base address.
pub fn as_apic_ptr(ptr: u64) -> ApicPtr {
    ApicPtr::new(ptr as *mut u32)
}

impl LowerHex for ApicPtr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:x}", self.ptr as u64)
    }
}

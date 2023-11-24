use std::arch::asm;
use std::sync::atomic::compiler_fence;
use std::sync::atomic::Ordering;

#[inline(always)]
pub fn dmb() {
    // It is not certain if these compiler fences are really required to prevent
    // the compiler from reordering the dmb instruction with neighbouring code.
    compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!("dmb sy");
    }
    compiler_fence(Ordering::SeqCst);
}

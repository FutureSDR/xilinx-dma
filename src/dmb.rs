use std::sync::atomic::compiler_fence;
use std::sync::atomic::Ordering;

#[link(name = "dmb")]
extern "C" {
    pub fn __dmb();
}
#[inline(always)]
pub fn dmb() {
    compiler_fence(Ordering::SeqCst);
    unsafe {
        __dmb();
    }
    compiler_fence(Ordering::SeqCst);
}

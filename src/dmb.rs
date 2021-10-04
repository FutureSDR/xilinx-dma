#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[link(name = "dmb")]
extern "C" {
    pub fn dmb();
}

#[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
pub fn dmb() {
    // DMB is ARM-only, so we use a nop in other archs
}

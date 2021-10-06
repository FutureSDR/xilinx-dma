mod axi_dma;
mod axi_dma_async;
mod dma_buffer;
pub use axi_dma::AxiDma;
pub use axi_dma_async::AxiDmaAsync;
pub use dma_buffer::DmaBuffer;


#[cfg(any(target_arch = "armv7", target_arch = "aarch64"))]
mod dmb;
#[cfg(any(target_arch = "armv7", target_arch = "aarch64"))]
pub use dmb::dmb;

#[cfg(not(any(target_arch = "armv7", target_arch = "aarch64")))]
#[inline(always)]
pub fn dmb() {
    // DMB is ARM-only, so we use a nop in other archs
}

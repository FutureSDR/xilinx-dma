mod axi_dma;

#[cfg(feature = "async")]
mod axi_dma_async;

mod dma_buffer;
pub use axi_dma::AxiDma;

#[cfg(feature = "async")]
pub use axi_dma_async::AxiDmaAsync;

pub use dma_buffer::DmaBuffer;

#[cfg(feature = "scatter-gather")]
mod scatter_gather;
#[cfg(feature = "scatter-gather")]
pub use scatter_gather::{SgDescriptor, SG_DESCRIPTOR_LEN};

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
mod dmb;
#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
pub use dmb::dmb;

#[cfg(not(any(target_arch = "arm", target_arch = "aarch64")))]
#[inline(always)]
pub fn dmb() {
    // DMB is ARM-only, so we use a nop in other archs
}

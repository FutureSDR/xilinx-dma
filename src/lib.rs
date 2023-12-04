mod axi_dma;

mod dma_buffer;
pub use axi_dma::AxiDma;

#[cfg(feature = "async")]
pub use axi_dma::AxiDmaAsync;

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

/// Xilinx DMA Error
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Mmapping DMA buffer into virtual memory failed.")]
    Mmap,
    #[error("Scatter Gather is disabled in HW.")]
    SgDisabled,
    #[error("DMA internal error (DMASR 0x{0:08x})")]
    DmaInternal(u32),
    #[error("DMA slave error (DMASR 0x{0:08x})")]
    DmaSlave(u32),
    #[error("DMA decode error (DMASR 0x{0:08x})")]
    DmaDecode(u32),
    #[error("Scatter Gather internal error (DMASR 0x{0:08x})")]
    SgInternal(u32),
    #[error("Scatter Gather slave error (DMASR 0x{0:08x})")]
    SgSlave(u32),
    #[error("Scatter Gather decode error (DMASR 0x{0:08x})")]
    SgDecode(u32),
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse integer from sysfs files.")]
    Parse(#[from] std::num::ParseIntError),
}

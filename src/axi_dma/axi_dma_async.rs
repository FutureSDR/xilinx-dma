use async_io::Async;
use std::fmt;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd;

use super::AxiDmaBase;
#[cfg(feature = "scatter-gather")]
use crate::dmb;
use crate::DmaBuffer;
use crate::Error;
#[cfg(feature = "scatter-gather")]
use crate::SgDescriptor;

pub struct AxiDmaAsync {
    dev_fd: Async<File>,
    dma: AxiDmaBase,
}

impl fmt::Debug for AxiDmaAsync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "AxiDmaAsync ({})", &self.dma.dev)?;
        writeln!(f, "  file: {:?}", &self.dev_fd)?;
        writeln!(f, "  base: {:?}", &self.dma.base)?;
        write!(f, "  size: {:#x?}", &self.dma.size)
    }
}

impl AxiDmaAsync {
    pub fn new(uio: &str) -> Result<AxiDmaAsync, Error> {
        let dev_fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/{}", uio))?;
        let dma = AxiDmaBase::new(uio, dev_fd.as_raw_fd())?;
        Ok(AxiDmaAsync {
            dev_fd: Async::new(dev_fd)?,
            dma,
        })
    }

    pub async fn start_h2d(&mut self, buff: &DmaBuffer, bytes: usize) -> Result<(), Error> {
        self.dma.start_h2d_ini(buff, bytes);
        self.enable_uio_irqs().await?;
        self.dma.start_h2d_fini(buff, bytes);
        Ok(())
    }

    pub async fn start_d2h(&mut self, buff: &DmaBuffer, bytes: usize) -> Result<(), Error> {
        self.dma.start_d2h_ini(buff, bytes);
        self.enable_uio_irqs().await?;
        self.dma.start_d2h_fini(buff, bytes);
        Ok(())
    }

    async fn enable_uio_irqs(&mut self) -> Result<(), Error> {
        unsafe {
            self.dev_fd
                .write_with_mut(|s| s.write(&[1u8, 0, 0, 0]))
                .await?;
        }
        Ok(())
    }

    #[cfg(feature = "scatter-gather")]
    pub fn enqueue_sg_h2d(&mut self, descriptor: &mut SgDescriptor) -> Result<(), Error> {
        self.dma.enqueue_sg_h2d(descriptor)
    }

    #[cfg(feature = "scatter-gather")]
    pub fn enqueue_sg_d2h(&mut self, descriptor: &mut SgDescriptor) -> Result<(), Error> {
        self.dma.enqueue_sg_d2h(descriptor)
    }

    #[cfg(feature = "scatter-gather")]
    pub async fn wait_sg_complete_h2d(&mut self, descriptor: &SgDescriptor) -> Result<(), Error> {
        loop {
            if descriptor.completed() {
                dmb(); // the complete flag acts as an acquire lock
                break;
            }

            // Wait for an interrupt that might indicate that the descriptor has
            // been completed.
            self.enable_uio_irqs().await?;
            self.wait_h2d().await?;

            self.dma.wait_sg_complete_h2d_fini()?;
        }
        Ok(())
    }

    #[cfg(feature = "scatter-gather")]
    pub async fn wait_sg_complete_d2h(&mut self, descriptor: &SgDescriptor) -> Result<(), Error> {
        loop {
            if descriptor.completed() {
                dmb(); // the complete flag acts as an acquire lock
                break;
            }

            // Wait for an interrupt that might indicate that the descriptor has
            // been completed.
            self.enable_uio_irqs().await?;
            self.wait_d2h().await?;

            self.dma.wait_sg_complete_d2h_fini()?;
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.dma.reset();
    }

    pub fn status_h2d(&self) {
        self.dma.status_h2d();
    }

    pub fn status_d2h(&self) {
        self.dma.status_d2h();
    }

    pub async fn wait_d2h(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; 4];
        unsafe { self.dev_fd.read_with_mut(|s| s.read(&mut buf)).await? };
        Ok(())
    }

    pub async fn wait_h2d(&mut self) -> Result<(), Error> {
        let mut buf = [0u8; 4];
        unsafe { self.dev_fd.read_with_mut(|s| s.read(&mut buf)).await? };
        Ok(())
    }

    pub fn size_d2h(&self) -> usize {
        self.dma.size_d2h()
    }
}

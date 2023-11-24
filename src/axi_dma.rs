use anyhow::Result;
use std::fmt;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::os::unix::io::AsRawFd;
use std::ptr;

use crate::dmb;
use crate::DmaBuffer;
#[cfg(feature = "scatter-gather")]
use crate::SgDescriptor;

#[allow(clippy::erasing_op)]
const MM2S_DMACR: isize = 0x0 / 4;
#[allow(clippy::eq_op)]
const MM2S_DMASR: isize = 0x4 / 4;
#[cfg(feature = "scatter-gather")]
const MM2S_CURRDESC: isize = 0x8 / 4;
#[cfg(feature = "scatter-gather")]
const MM2S_CURRDESC_MSB: isize = 0xC / 4;
#[cfg(feature = "scatter-gather")]
const MM2S_TAILDESC: isize = 0x10 / 4;
#[cfg(feature = "scatter-gather")]
const MM2S_TAILDESC_MSB: isize = 0x14 / 4;
const MM2S_SA: isize = 0x18 / 4;
const MM2S_SA_MSB: isize = 0x1C / 4;
const MM2S_LENGTH: isize = 0x28 / 4;
const S2MM_DMACR: isize = 0x30 / 4;
const S2MM_DMASR: isize = 0x34 / 4;
#[cfg(feature = "scatter-gather")]
const S2MM_CURRDESC: isize = 0x38 / 4;
#[cfg(feature = "scatter-gather")]
const S2MM_CURRDESC_MSB: isize = 0x3C / 4;
#[cfg(feature = "scatter-gather")]
const S2MM_TAILDESC: isize = 0x40 / 4;
#[cfg(feature = "scatter-gather")]
const S2MM_TAILDESC_MSB: isize = 0x44 / 4;
const S2MM_DA: isize = 0x48 / 4;
const S2MM_DA_MSB: isize = 0x4C / 4;
const S2MM_LENGTH: isize = 0x58 / 4;

pub struct AxiDma {
    dev: String,
    dev_fd: File,
    base: *mut u32,
    size: usize,
}

impl fmt::Debug for AxiDma {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "AxiDma ({})", &self.dev)?;
        writeln!(f, "  file: {:?}", &self.dev_fd)?;
        writeln!(f, "  base: {:?}", &self.base)?;
        write!(f, "  size: {:#x?}", &self.size)
    }
}

impl AxiDma {
    pub fn new(uio: &str) -> Result<AxiDma> {
        let dev_fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/{}", uio))?;

        let mut size_f = File::open(format!("/sys/class/uio/{}/maps/map0/size", uio))?;
        let mut buf = String::new();
        size_f.read_to_string(&mut buf)?;
        let buf = buf.trim().trim_start_matches("0x");
        let size = usize::from_str_radix(buf, 16)?;

        let dev;
        unsafe {
            dev = libc::mmap(
                std::ptr::null_mut::<libc::c_void>(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                dev_fd.as_raw_fd(),
                0,
            );
            if dev == libc::MAP_FAILED {
                anyhow::bail!("mapping dma buffer into virtual memory failed");
            }
        }

        Ok(AxiDma {
            dev: uio.to_string(),
            dev_fd,
            base: dev as *mut u32,
            size,
        })
    }

    pub fn start_h2d(&mut self, buff: &DmaBuffer, bytes: usize) -> Result<()> {
        debug_assert!(buff.size() >= bytes);
        unsafe {
            // Ensure that the DDR buffer has been written to
            dmb();

            // clear irqs in dma
            ptr::write_volatile(self.base.offset(MM2S_DMASR), 0x7000);

            // enable irqs for uio driver
            self.dev_fd.write_all(&[1u8, 0, 0, 0])?;

            // Configure AXIDMA - MM2S (PS -> PL)
            ptr::write_volatile(self.base.offset(MM2S_DMACR), 0x7001);
            #[allow(clippy::identity_op)]
            ptr::write_volatile(
                self.base.offset(MM2S_SA),
                (buff.phys_addr() & 0xffff_ffff) as u32,
            );
            ptr::write_volatile(
                self.base.offset(MM2S_SA_MSB),
                (buff.phys_addr() & !0xffff_ffff).wrapping_shr(32) as u32,
            );
            ptr::write_volatile(self.base.offset(MM2S_LENGTH), bytes as u32);
        }
        Ok(())
    }

    pub fn start_d2h(&mut self, buff: &DmaBuffer, bytes: usize) -> Result<()> {
        debug_assert!(buff.size() >= bytes);
        unsafe {
            // clear irqs in dma
            ptr::write_volatile(self.base.offset(S2MM_DMASR), 0x7000);

            // enable irqs for uio driver
            self.dev_fd.write_all(&[1u8, 0, 0, 0])?;

            // Configure AXIDMA - S2MM (PL -> PS)
            ptr::write_volatile(self.base.offset(S2MM_DMACR), 0x7001);
            #[allow(clippy::identity_op)]
            ptr::write_volatile(
                self.base.offset(S2MM_DA),
                (buff.phys_addr() & 0xffff_ffff) as u32,
            );
            ptr::write_volatile(
                self.base.offset(S2MM_DA_MSB),
                (buff.phys_addr() & !0xffff_ffff).wrapping_shr(32) as u32,
            );
            ptr::write_volatile(self.base.offset(S2MM_LENGTH), bytes as u32);
        }
        Ok(())
    }

    #[cfg(feature = "scatter-gather")]
    pub fn enqueue_sg_h2d(&mut self, descriptor: &mut SgDescriptor) -> Result<()> {
        unsafe {
            // Mark descriptor as not complete so that calls to
            // wait_sg_complete_h2d must wait for the DMA to mark it as
            // complete.
            descriptor.clear_status();

            // Ensure that the descriptor and buffer have been written to
            dmb();

            let status = ptr::read_volatile(self.base.offset(MM2S_DMASR));
            let sg_incl = status & (1 << 3) != 0;
            if !sg_incl {
                anyhow::bail!("Scatter Gather is disabled");
            }
            self.check_errors(status)?;
            let stopped = status & 1 != 0;
            if stopped {
                // Start DMA

                // Write descriptor as first descriptor. This can only be
                // done with the DMA stopped.
                #[allow(clippy::identity_op)]
                ptr::write_volatile(
                    self.base.offset(MM2S_CURRDESC),
                    (descriptor.phys_addr() & 0xffff_ffff) as u32,
                );
                ptr::write_volatile(
                    self.base.offset(MM2S_CURRDESC_MSB),
                    (descriptor.phys_addr() & !0xffff_ffff).wrapping_shr(32) as u32,
                );

                // Start the DMA:
                // - IRQThreshold = 1
                // - ERR_IrqEn = 1
                // - Dly_IrqEN = 0
                // - IOC_IrqEn = 1
                ptr::write_volatile(self.base.offset(MM2S_DMACR), 0x0001_5001);
            }

            // Write descriptor as tail descriptor. The MSB is written
            // first, since writing the LSB triggers the DMA to start if it
            // was stopped.
            ptr::write_volatile(
                self.base.offset(MM2S_TAILDESC_MSB),
                (descriptor.phys_addr() & !0xffff_ffff).wrapping_shr(32) as u32,
            );
            // Here there is a subtle race condition because the MSB and LSB
            // registers of the TAILDESC cannot be updated atomically. This
            // is not really a problem, because if the DMA was already
            // running, the TAILDESC only needs to be set correctly when the
            // DMA arrives to the end of the buffer for the descriptor we
            // are enqueueing (so that it enters the idle state if no futher
            // descriptors have been equeued).
            #[allow(clippy::identity_op)]
            ptr::write_volatile(
                self.base.offset(MM2S_TAILDESC),
                (descriptor.phys_addr() & 0xffff_ffff) as u32,
            );

            Ok(())
        }
    }

    #[cfg(feature = "scatter-gather")]
    pub fn enqueue_sg_d2h(&mut self, descriptor: &mut SgDescriptor) -> Result<()> {
        unsafe {
            // Mark descriptor as not complete so that calls to
            // wait_sg_complete_d2h must wait for the DMA to mark it as
            // complete.
            descriptor.clear_status();

            // Ensure that the descriptor has been written to
            dmb();

            let status = ptr::read_volatile(self.base.offset(S2MM_DMASR));
            let sg_incl = status & (1 << 3) != 0;
            if !sg_incl {
                anyhow::bail!("Scatter Gather is disabled");
            }
            self.check_errors(status)?;
            let stopped = status & 1 != 0;
            if stopped {
                // Start DMA

                // Write descriptor as first descriptor. This can only be
                // done with the DMA stopped.
                #[allow(clippy::identity_op)]
                ptr::write_volatile(
                    self.base.offset(S2MM_CURRDESC),
                    (descriptor.phys_addr() & 0xffff_ffff) as u32,
                );
                ptr::write_volatile(
                    self.base.offset(S2MM_CURRDESC_MSB),
                    (descriptor.phys_addr() & !0xffff_ffff).wrapping_shr(32) as u32,
                );

                // Start the DMA:
                // - IRQThreshold = 1
                // - ERR_IrqEn = 1
                // - Dly_IrqEN = 0
                // - IOC_IrqEn = 1
                ptr::write_volatile(self.base.offset(S2MM_DMACR), 0x0001_5001);
            }

            // Write descriptor as tail descriptor. The MSB is written
            // first, since writing the LSB triggers the DMA to start if it
            // was stopped.
            ptr::write_volatile(
                self.base.offset(S2MM_TAILDESC_MSB),
                (descriptor.phys_addr() & !0xffff_ffff).wrapping_shr(32) as u32,
            );
            // Here there is a subtle race condition because the MSB and LSB
            // registers of the TAILDESC cannot be updated atomically. This
            // is not really a problem, because if the DMA was already
            // running, the TAILDESC only needs to be set correctly when the
            // DMA arrives to the end of the buffer for the descriptor we
            // are enqueueing (so that it enters the idle state if no futher
            // descriptors have been equeued).
            #[allow(clippy::identity_op)]
            ptr::write_volatile(
                self.base.offset(S2MM_TAILDESC),
                (descriptor.phys_addr() & 0xffff_ffff) as u32,
            );

            Ok(())
        }
    }

    #[cfg(feature = "scatter-gather")]
    pub fn wait_sg_complete_h2d(&mut self, descriptor: &SgDescriptor) -> Result<()> {
        loop {
            if descriptor.completed() {
                dmb(); // the complete flag acts as an acquire lock
                break;
            }

            // Wait for an interrupt that might indicate that the descriptor has
            // been completed.

            // enable irqs for uio driver
            self.dev_fd.write_all(&[1u8, 0, 0, 0])?;
            let mut buf = [0u8; 4];
            self.dev_fd.read_exact(&mut buf)?;
            unsafe {
                // check that there are no errors
                self.check_errors(ptr::read_volatile(self.base.offset(MM2S_DMASR)))?;
                // clear irqs in dma
                ptr::write_volatile(self.base.offset(MM2S_DMASR), 0x7000);
            }
        }
        Ok(())
    }

    #[cfg(feature = "scatter-gather")]
    pub fn wait_sg_complete_d2h(&mut self, descriptor: &SgDescriptor) -> Result<()> {
        loop {
            if descriptor.completed() {
                dmb(); // the complete flag acts as an acquire lock
                break;
            }

            // Wait for an interrupt that might indicate that the descriptor has
            // been completed.

            // enable irqs for uio driver
            self.dev_fd.write_all(&[1u8, 0, 0, 0])?;
            let mut buf = [0u8; 4];
            self.dev_fd.read_exact(&mut buf)?;
            unsafe {
                // check that there are no errors
                self.check_errors(ptr::read_volatile(self.base.offset(S2MM_DMASR)))?;
                // clear irqs in dma
                ptr::write_volatile(self.base.offset(S2MM_DMASR), 0x7000);
            }
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        unsafe {
            // reset controller
            ptr::write_volatile(self.base.offset(MM2S_DMACR), 0x0004);
            loop {
                if ptr::read_volatile(self.base.offset(MM2S_DMACR)) & 0x0004 == 0 {
                    break;
                }
            }
            // reset controller
            ptr::write_volatile(self.base.offset(S2MM_DMACR), 0x0004);
            loop {
                if ptr::read_volatile(self.base.offset(S2MM_DMACR)) & 0x0004 == 0 {
                    break;
                }
            }

            // clear irqs
            ptr::write_volatile(self.base.offset(S2MM_DMASR), 0x7000);
            ptr::write_volatile(self.base.offset(MM2S_DMASR), 0x7000);
        }
    }

    pub fn status_h2d(&self) {
        let mut c;
        unsafe {
            c = ptr::read_volatile(self.base.offset(MM2S_DMACR));
        }
        print!("h2d control: ");
        if c & 1 != 0 {
            print!("running, ");
        } else {
            print!("stopped, ");
        }
        if c & 4 != 0 {
            print!("resetting, ");
        }
        if c & 1 << 12 != 0 {
            print!("ioc_irq_en, ");
        }
        if c & 1 << 13 != 0 {
            print!("dly_irq_en, ");
        }
        if c & 1 << 14 != 0 {
            print!("err_irq_en, ");
        }
        println!();
        unsafe {
            c = ptr::read_volatile(self.base.offset(MM2S_DMASR));
        }
        print!("h2d status: ");
        if c & 1 != 0 {
            print!("halted, ");
        } else {
            print!("stopped, ");
        }
        if c & 2 != 0 {
            print!("idle, ");
        } else {
            print!("busy, ");
        }
        if c & 8 != 0 {
            print!("scatter gather, ");
        } else {
            print!("register mode, ");
        }
        if c & 1 << 4 != 0 {
            print!("internal error, ");
        }
        if c & 1 << 5 != 0 {
            print!("slave error, ");
        }
        if c & 1 << 6 != 0 {
            print!("decode error, ");
        }
        if c & 1 << 8 != 0 {
            print!("sg internal error, ");
        }
        if c & 1 << 9 != 0 {
            print!("sg slave error, ");
        }
        if c & 1 << 10 != 0 {
            print!("sg dec error, ");
        }
        if c & 1 << 12 != 0 {
            print!("ioc_irq, ");
        }
        if c & 1 << 13 != 0 {
            print!("dly_irq, ");
        }
        if c & 1 << 14 != 0 {
            print!("err_irq, ");
        }
        println!();
    }

    pub fn status_d2h(&self) {
        let mut c;
        unsafe {
            c = ptr::read_volatile(self.base.offset(S2MM_DMACR));
        }
        print!("d2h control: ");
        if c & 1 != 0 {
            print!("running, ");
        } else {
            print!("stopped, ");
        }
        if c & 4 != 0 {
            print!("resetting, ");
        }
        if c & 1 << 12 != 0 {
            print!("ioc_irq_en, ");
        }
        if c & 1 << 13 != 0 {
            print!("dly_irq_en, ");
        }
        if c & 1 << 14 != 0 {
            print!("err_irq_en, ");
        }
        println!();
        unsafe {
            c = ptr::read_volatile(self.base.offset(S2MM_DMASR));
        }
        print!("d2h status: ");
        if c & 1 != 0 {
            print!("halted, ");
        } else {
            print!("stopped, ");
        }
        if c & 2 != 0 {
            print!("idle, ");
        } else {
            print!("busy, ");
        }
        if c & 8 != 0 {
            print!("scatter gather, ");
        } else {
            print!("register mode, ");
        }
        if c & 1 << 4 != 0 {
            print!("internal error, ");
        }
        if c & 1 << 5 != 0 {
            print!("slave error, ");
        }
        if c & 1 << 6 != 0 {
            print!("decode error, ");
        }
        if c & 1 << 8 != 0 {
            print!("sg internal error, ");
        }
        if c & 1 << 9 != 0 {
            print!("sg slave error, ");
        }
        if c & 1 << 10 != 0 {
            print!("sg dec error, ");
        }
        if c & 1 << 12 != 0 {
            print!("ioc_irq, ");
        }
        if c & 1 << 13 != 0 {
            print!("dly_irq, ");
        }
        if c & 1 << 14 != 0 {
            print!("err_irq, ");
        }
        println!();
    }

    pub fn wait_d2h(&mut self) -> Result<()> {
        let mut buf = [0u8; 4];
        self.dev_fd.read_exact(&mut buf)?;
        Ok(())
    }

    pub fn wait_h2d(&mut self) -> Result<()> {
        let mut buf = [0u8; 4];
        self.dev_fd.read_exact(&mut buf)?;
        Ok(())
    }

    pub fn size_d2h(&self) -> usize {
        unsafe { ptr::read_volatile(self.base.offset(S2MM_LENGTH)) as usize }
    }

    #[cfg(feature = "scatter-gather")]
    fn check_errors(&self, status: u32) -> Result<()> {
        let dma_int_err = status & (1 << 4) != 0;
        let dma_slv_err = status & (1 << 5) != 0;
        let dma_dec_err = status & (1 << 6) != 0;
        let sg_int_err = status & (1 << 8) != 0;
        let sg_slv_err = status & (1 << 9) != 0;
        let sg_dec_err = status & (1 << 10) != 0;
        if dma_int_err {
            anyhow::bail!("DMA internal error: DMASR 0x{:08x}", status);
        }
        if dma_slv_err {
            anyhow::bail!("DMA slave error: DMASR 0x{:08x}", status);
        }
        if dma_dec_err {
            anyhow::bail!("DMA decode error: DMASR 0x{:08x}", status);
        }
        if sg_int_err {
            anyhow::bail!("SG internal error: DMASR 0x{:08x}", status);
        }
        if sg_slv_err {
            anyhow::bail!("SG slave error: DMASR 0x{:08x}", status);
        }
        if sg_dec_err {
            anyhow::bail!("SG decode error: DMASR 0x{:08x}", status);
        }
        Ok(())
    }
}

impl Drop for AxiDma {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.base as *mut libc::c_void, self.size);
        }
    }
}

unsafe impl Send for AxiDma {}

//! Scatter-Gather loopback example.
//!
//! This example is intended to be run with two AXI-DMAs connected together as a
//! loopback (perhaps with an AXI4-Stream Data FIFO in between). The following
//! is an example of how the device tree should look like:
//!
//! ```dts
//! fpga-axi@0 {
//!     compatible = "simple-bus";
//!     #address-cells = <0x01>;
//!     #size-cells = <0x01>;
//!     ranges;
//!
//!     h2d-dma@40400000 {
//!         compatible = "uio_pdrv_genirq";
//!         interrupt-parent = <0x01>;
//!         interrupts = <0x00 0x1d 0x04>;
//!         reg = <0x40400000 0x10000>;
//!     };
//!
//!     d2h-dma@40410000 {
//!         compatible = "uio_pdrv_genirq";
//!         interrupt-parent = <0x01>;
//!         interrupts = <0x00 0x1e 0x04>;
//!         reg = <0x40410000 0x10000>;
//!     };
//! };
//!
//! udmabuf_descriptors {
//!     compatible = "ikwzm,u-dma-buf";
//!     size = <0x1000>;
//!     sync-mode = <2>;
//!     sync-always;
//! };
//!
//! udmabuf_h2d0 {
//!     compatible = "ikwzm,u-dma-buf";
//!     size = <0x800000>;
//!     sync-mode = <2>;
//!     sync-always;
//! };
//!
//! udmabuf_h2d1 {
//!     compatible = "ikwzm,u-dma-buf";
//!     size = <0x800000>;
//!     sync-mode = <2>;
//!     sync-always;
//! };
//!
//! udmabuf_d2h0 {
//!     compatible = "ikwzm,u-dma-buf";
//!     size = <0x800000>;
//!     sync-mode = <2>;
//!     sync-direction = <2>;
//!     sync-offset = <0x0>;
//!     sync-size = <0x800000>;
//! };
//!
//! udmabuf_d2h1 {
//!     compatible = "ikwzm,u-dma-buf";
//!     size = <0x800000>;
//!     sync-mode = <2>;
//!     sync-direction = <2>;
//!     sync-offset = <0x0>;
//!     sync-size = <0x800000>;
//! };
//! ```
//!
//! Two H2D buffers and two D2H buffers are used as ping-pong buffers. The
//! descriptors for each pair of buffers point to each other, so the AXI DMA
//! never enters the idle state if the application is fast enough processing the
//! buffers.
//!
//! For best throughput, the H2D buffers use uncached memory with
//! write-combining. This gives good throughput, because in this example the
//! buffers are filled sequentially. The D2H buffers use cached memory with
//! manual cache invalidation, since reading an uncached buffer sequentially to
//! check its contents is very slow.

use anyhow::Result;
use std::convert::TryFrom;
use xilinx_dma::AxiDma;
use xilinx_dma::DmaBuffer;
use xilinx_dma::SgDescriptor;
use xilinx_dma::SG_DESCRIPTOR_LEN;

fn main() -> Result<()> {
    let descriptor_buffer = DmaBuffer::new("udmabuf_descriptors")?;
    let mut h2d0 = DmaBuffer::new("udmabuf_h2d0")?;
    let mut h2d1 = DmaBuffer::new("udmabuf_h2d1")?;
    let mut d2h0 = DmaBuffer::new("udmabuf_d2h0")?;
    let mut d2h1 = DmaBuffer::new("udmabuf_d2h1")?;
    let mut h2d_dma = AxiDma::new("uio0")?;
    let mut d2h_dma = AxiDma::new("uio1")?;

    // Set up descriptors

    // Create 4 descriptors
    let descriptors_base_virt = descriptor_buffer.slice::<u32>().as_mut_ptr();
    let descriptors_base_phys = descriptor_buffer.phys_addr();
    let mut descriptors = (0..4)
        .map(|j| unsafe {
            SgDescriptor::from_base_ptr(
                descriptors_base_virt.add(j * SG_DESCRIPTOR_LEN / std::mem::size_of::<u32>()),
                descriptors_base_phys + j * SG_DESCRIPTOR_LEN,
            )
        })
        .collect::<Vec<_>>();

    // Descriptors 0 and 1 are used for h2d, and point to each other
    let phys_addr = descriptors[1].phys_addr();
    descriptors[0].set_next_descriptor(phys_addr);
    let phys_addr = descriptors[0].phys_addr();
    descriptors[1].set_next_descriptor(phys_addr);
    descriptors[0].set_buffer_address(h2d0.phys_addr());
    descriptors[0].set_buffer_length(u32::try_from(h2d0.size()).unwrap());
    descriptors[1].set_buffer_address(h2d1.phys_addr());
    descriptors[1].set_buffer_length(u32::try_from(h2d1.size()).unwrap());

    for descriptor in &mut descriptors[..2] {
        descriptor.set_sof(true);
        descriptor.set_eof(true);
        descriptor.clear_status();
        // Set the completed flag so that we know that the buffer is initially
        // free. This is required by wait_sg_complete_h2d.
        descriptor.set_completed(true);
    }

    // Descriptors 2 and 3 are used for d2h, and point to each other
    let phys_addr = descriptors[3].phys_addr();
    descriptors[2].set_next_descriptor(phys_addr);
    let phys_addr = descriptors[2].phys_addr();
    descriptors[3].set_next_descriptor(phys_addr);
    descriptors[2].set_buffer_address(d2h0.phys_addr());
    descriptors[2].set_buffer_length(u32::try_from(d2h0.size()).unwrap());
    descriptors[3].set_buffer_address(d2h1.phys_addr());
    descriptors[3].set_buffer_length(u32::try_from(d2h1.size()).unwrap());
    let mut descriptor3 = descriptors.pop().unwrap();
    let mut descriptor2 = descriptors.pop().unwrap();
    let mut descriptor1 = descriptors.pop().unwrap();
    let mut descriptor0 = descriptors.pop().unwrap();

    let total_transfer: u64 = 1_000_000_000;

    let receive_thread = std::thread::spawn(move || {
        let mut checker = DataChecker::new();
        let mut current = (&mut descriptor2, &mut d2h0);
        let mut other = (&mut descriptor3, &mut d2h1);
        let mut remaining = total_transfer;
        d2h_dma.reset();
        d2h_dma.enqueue_sg_d2h(current.0)?;
        d2h_dma.enqueue_sg_d2h(other.0)?;
        loop {
            d2h_dma.wait_sg_complete_d2h(current.0)?;
            let transferred_bytes = current.0.transferred_bytes();
            assert_eq!(transferred_bytes % std::mem::size_of::<u32>() as u32, 0);
            let transferred_items =
                usize::try_from(transferred_bytes).unwrap() / std::mem::size_of::<u32>();
            // Invalidate cache of D2H buffer.
            current.1.sync_for_cpu()?;
            checker.check_buffer(&current.1.slice::<u32>()[..transferred_items]);
            remaining -= u64::from(transferred_bytes);
            if remaining == 0 {
                break;
            }
            d2h_dma.enqueue_sg_d2h(current.0)?;
            std::mem::swap(&mut current, &mut other);
        }
        Ok::<(), anyhow::Error>(())
    });

    let mut generator = DataGenerator::new();
    let mut current = (&mut descriptor0, &mut h2d0);
    let mut other = (&mut descriptor1, &mut h2d1);
    let mut remaining = total_transfer;
    let start = std::time::Instant::now();
    h2d_dma.reset();
    loop {
        h2d_dma.wait_sg_complete_h2d(current.0)?;
        if remaining == 0 {
            break;
        }
        generator.fill_buffer(current.1);
        let buffer_size = u64::try_from(current.1.size()).unwrap();
        if remaining < buffer_size {
            current
                .0
                .set_buffer_length(u32::try_from(remaining).unwrap());
            remaining = 0;
        } else {
            remaining -= buffer_size;
        };
        // The H2D buffer uses uncached memory, so there is no need to
        // invalidate the cache.
        h2d_dma.enqueue_sg_h2d(current.0)?;
        std::mem::swap(&mut current, &mut other);
    }
    h2d_dma.wait_sg_complete_h2d(other.0)?;
    let elapsed = start.elapsed();

    receive_thread.join().unwrap()?;

    println!("transferred {total_transfer} bytes in {elapsed:?}");
    let bps = (8 * total_transfer) as f64 / elapsed.as_secs_f64();
    println!("average data rate: {bps:.3e} bits/second");

    Ok(())
}

#[derive(Debug, Default)]
struct DataGenerator {
    counter: u32,
}

impl DataGenerator {
    fn new() -> DataGenerator {
        DataGenerator::default()
    }

    fn fill_buffer(&mut self, buffer: &DmaBuffer) {
        for x in buffer.slice::<u32>() {
            *x = self.counter;
            self.counter = self.counter.wrapping_add(1);
        }
    }
}

#[derive(Debug, Default)]
struct DataChecker {
    counter: u32,
}

impl DataChecker {
    fn new() -> DataChecker {
        DataChecker::default()
    }

    fn check_buffer(&mut self, buffer: &[u32]) {
        for &x in buffer {
            assert_eq!(x, self.counter);
            self.counter = self.counter.wrapping_add(1);
        }
    }
}

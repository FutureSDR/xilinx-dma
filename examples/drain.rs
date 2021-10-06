use anyhow::Result;
use xilinx_dma::AxiDma;
use xilinx_dma::DmaBuffer;

fn main() -> Result<()> {
    let dma_buffer = DmaBuffer::new("udmabuf0")?;
    println!("{:?}", dma_buffer);

    let mut dma_h2d = AxiDma::new("uio4")?;
    let mut dma_d2h = AxiDma::new("uio5")?;
    println!("{:?}", dma_d2h);

    dma_h2d.reset();
    dma_d2h.reset();

    dma_d2h.start_d2h(&dma_buffer, dma_buffer.size())?;
    std::thread::sleep(std::time::Duration::from_secs_f64(0.1));
    dma_d2h.start_d2h(&dma_buffer, dma_buffer.size())?;
    std::thread::sleep(std::time::Duration::from_secs_f64(0.1));
    dma_d2h.start_d2h(&dma_buffer, dma_buffer.size())?;
    std::thread::sleep(std::time::Duration::from_secs_f64(0.1));

    dma_h2d.status_h2d();
    dma_d2h.status_d2h();

    Ok(())
}

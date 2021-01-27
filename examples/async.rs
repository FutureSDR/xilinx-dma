use anyhow::Result;
use xilinx_dma::DmaBuff;
use xilinx_dma::AxiDmaAsync;

fn main() -> Result<()> {

    let dma_buffer_h2d = DmaBuff::new("udmabuf0")?;
    let dma_buffer_d2h = DmaBuff::new("udmabuf1")?;
    println!("{:?}", dma_buffer_h2d);
    println!("{:?}", dma_buffer_d2h);

    // do not use the whole buffer
    let max_items = 1024;
    let items = std::cmp::min(max_items, dma_buffer_h2d.size()/4);
    let items = std::cmp::min(items, dma_buffer_d2h.size()/4);

    let slice_h2d = &mut dma_buffer_h2d.slice::<u32>()[0..items];
    let slice_d2h = &mut dma_buffer_d2h.slice::<u32>()[0..items];

    for i in slice_d2h.iter_mut() {
        *i = 0;
    }

    for i in slice_h2d.iter_mut() {
        *i = fastrand::u32(0..1024);
    }

    let mut dma_h2d = AxiDmaAsync::new("uio4")?;
    let mut dma_d2h = AxiDmaAsync::new("uio5")?;
    println!("{:?}", dma_h2d);
    println!("{:?}", dma_d2h);

    async_io::block_on(async {
        dma_h2d.start_h2d(&dma_buffer_h2d, items*4).await?;
        dma_d2h.start_d2h(&dma_buffer_d2h, items*4).await?;
        println!("transfers started");

        dma_h2d.wait_h2d().await?;
        println!("h2d done");
        dma_d2h.wait_d2h().await?;
        println!("d2h done");
        Result::<()>::Ok(())
    })?;

    for i in 0..items {
        assert_eq!(slice_d2h[i], slice_h2d[i] + 123);
    }

    Ok(())
}

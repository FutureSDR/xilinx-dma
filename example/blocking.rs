fn main() -> std::io::Result<()> {

    let rx_dma = DmaBuf::new("udmabuf0");
    println!("rx dma phys addr: {:#x?}", rx_dma.phys_addr);
    println!("rx dma size: {}", rx_dma.size);
    println!("rx dma debug vma: {}", rx_dma.debug_vma);
    println!("rx dma sync mode: {}", rx_dma.sync_mode);
    println!("rx buffer addr: {:?}", rx_dma.buffer);

    println!("{:?}", rx_dma);

    let tx_dma = DmaBuf::new("udmabuf1");
    println!("tx dma phys addr: {:#x?}", tx_dma.phys_addr);
    println!("tx dma size: {}", tx_dma.size);
    println!("tx dma debug vma: {}", tx_dma.debug_vma);
    println!("tx dma sync mode: {}", tx_dma.sync_mode);
    println!("tx buffer addr: {:?}", tx_dma.buffer);

    let rx_buffer;
    let tx_buffer;

    unsafe {
        rx_buffer = slice::from_raw_parts_mut(rx_dma.buffer as *mut u32, rx_dma.size / 4);
        tx_buffer = slice::from_raw_parts_mut(tx_dma.buffer as *mut u32, tx_dma.size / 4);
    }

    for i in rx_buffer.iter_mut() {
        *i = 0;
    }

    let mut rng = rand::thread_rng();
    let mut n = rng.gen_range(0,100);
    println!("rand n: {}", n);
    for i in tx_buffer.iter_mut() {
        *i = n;
        n += 1;
    }

    let mut dev_fd_tx = OpenOptions::new().read(true).write(true).open("/dev/uio4").unwrap();

    let mut size_f = File::open("/sys/class/uio/uio4/maps/map0/size").unwrap();
    let mut buf = String::new();
    size_f.read_to_string(&mut buf).unwrap();
    let buf = buf.trim().trim_start_matches("0x");
    let size = usize::from_str_radix(buf, 16).unwrap();
    println!("size of dev: {}", size);

    let dev;
    unsafe {
        dev = libc::mmap(0 as *mut libc::c_void, size, libc::PROT_READ|libc::PROT_WRITE, libc::MAP_SHARED, dev_fd_tx.as_raw_fd(), 0);
        if dev == libc::MAP_FAILED {
            panic!("mapping dma buffer into virtual memory failed");
        }
    }

    let base_tx = dev as usize;

    let mut dev_fd_rx = OpenOptions::new().read(true).write(true).open("/dev/uio5").unwrap();

    let mut size_f = File::open("/sys/class/uio/uio5/maps/map0/size").unwrap();
    let mut buf = String::new();
    size_f.read_to_string(&mut buf).unwrap();
    let buf = buf.trim().trim_start_matches("0x");
    let size = usize::from_str_radix(buf, 16).unwrap();
    println!("size of dev: {}", size);

    let dev;
    unsafe {
        dev = libc::mmap(0 as *mut libc::c_void, size, libc::PROT_READ|libc::PROT_WRITE, libc::MAP_SHARED, dev_fd_rx.as_raw_fd(), 0);
        if dev == libc::MAP_FAILED {
            panic!("mapping dma buffer into virtual memory failed");
        }
    }

    let base_rx = dev as usize;

    let tx_size : usize = 1024 * 15;

    unsafe {

        ptr::write_volatile((base_tx + MM2S_DMACR) as *mut u32, 0x0004);
        ptr::write_volatile((base_rx + S2MM_DMACR) as *mut u32, 0x0004);

        // clear irqs in dma
        ptr::write_volatile((base_tx + MM2S_DMASR) as *mut u32, 0x1000);
        ptr::write_volatile((base_rx + S2MM_DMASR) as *mut u32, 0x1000);

        // enable irqs for uio driver
        let a = [1u8, 0, 0, 0];
        dev_fd_tx.write(&a).unwrap();
        dev_fd_rx.write(&a).unwrap();

        // Configure AXIDMA - MM2S (PS -> PL)
        ptr::write_volatile((base_tx + MM2S_DMACR) as *mut u32, 0x1001);
        ptr::write_volatile((base_tx + MM2S_SA) as *mut u32, (tx_dma.phys_addr & 0xffffffff) as u32);
        ptr::write_volatile((base_tx + MM2S_SA_MSB) as *mut u32, (tx_dma.phys_addr >> 32) as u32);
        // ptr::write_volatile((base_tx + MM2S_LENGTH) as *mut u32, tx_dma.size as u32);
        ptr::write_volatile((base_tx + MM2S_LENGTH) as *mut u32, tx_size as u32);

        // Configure AXIDMA - S2MM (PL -> PS)
        ptr::write_volatile((base_rx + S2MM_DMACR) as *mut u32, 0x1001);
        ptr::write_volatile((base_rx + S2MM_DA) as *mut u32, (rx_dma.phys_addr & 0xffffffff) as u32);
        ptr::write_volatile((base_rx + S2MM_DA_MSB) as *mut u32, (rx_dma.phys_addr >> 32) as u32);
        // ptr::write_volatile((base_rx + S2MM_LENGTH) as *mut u32, rx_dma.size as u32);
        ptr::write_volatile((base_rx + S2MM_LENGTH) as *mut u32, tx_size as u32);
    }

    let mut buf = [0u8; 4];
    println!("waiting for DMA");
    dev_fd_tx.read(&mut buf).unwrap();
    println!("tx complete");
    dev_fd_rx.read(&mut buf).unwrap();
    println!("rx complete");

    // let mut poll = Poll::new()?;

    // // Register the listener
    // poll.registry().register(
    //     &mut SourceFd(&dev_fd_rx.as_raw_fd()),
    //     Token(0),
    //     Interest::READABLE).unwrap();

    // let mut events = Events::with_capacity(128);

    // poll.poll(&mut events, None).unwrap();

    // let ring = rio::new().expect("create uring");
    // let mut data: &mut [u8] = &mut [0; 4];
    // let completion = ring.read_at(&dev_fd_rx, &mut data, 0);
    // completion.wait().unwrap();

    // let mut io_uring = iou::IoUring::new(32).unwrap();
    // let mut buf1 = [0u8; 4];
    // let mut bufs = [std::io::IoSliceMut::new(&mut buf1)];

    // unsafe {
    //     let mut sq = io_uring.sq();
    //     let mut sqe = sq.prepare_sqe().unwrap();
    //     sqe.prep_read_vectored(dev_fd_rx.as_raw_fd(), &mut bufs[..], 0);
    //     sq.submit()?;
    // }

    // let mut cq = io_uring.cq();
    // let _ = cq.wait_for_cqe()?;

    // for i in (0..rx_dma.size/4).rev() {
    for i in 0..tx_size/4 {
        if rx_buffer[i] != tx_buffer[i] + 123 {
            println!("!!!!!!!! values do not match at index {}", i);
            break;
        }
    }

    unsafe {
        libc::munmap(dev, size);
        libc::munmap(rx_dma.buffer, rx_dma.size);
        libc::munmap(tx_dma.buffer, tx_dma.size);
    }

    Ok(())
}

const MM2S_DMACR: usize = 0x0;
const MM2S_DMASR: usize = 0x4;
const MM2S_SA: usize = 0x18;
const MM2S_SA_MSB: usize = 0x1C;
const MM2S_LENGTH: usize = 0x28;
const S2MM_DMACR: usize = 0x30;
const S2MM_DMASR: usize = 0x34;
const S2MM_DA: usize = 0x48;
const S2MM_DA_MSB: usize = 0x4C;
const S2MM_LENGTH: usize = 0x58;

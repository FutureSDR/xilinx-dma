use anyhow::Result;
use std::fmt;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::mem;
use std::os::unix::io::AsRawFd;
use std::slice;

pub struct DmaBuffer {
    name: String,
    size: usize,
    phys_addr: usize,
    buffer: *mut libc::c_void,
    sync_mode: bool,
    debug_vma: bool,
}

impl fmt::Debug for DmaBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "DmaBuffer ({})", &self.name)?;
        writeln!(f, "  size: {:#x?}", &self.size)?;
        writeln!(f, "  phys_addr: {:#x?}", &self.phys_addr)?;
        writeln!(f, "  buffer: {:?}", &self.buffer)?;
        writeln!(f, "  sync_mode: {:?}", &self.sync_mode)?;
        write!(f, "  debug_vma: {:?}", &self.debug_vma)
    }
}

impl DmaBuffer {
    pub fn new(name: &str) -> Result<DmaBuffer> {
        let phy_f = format!("/sys/class/u-dma-buf/{}/phys_addr", name);
        let mut phy_f = File::open(phy_f)?;
        let mut buff = String::new();
        phy_f.read_to_string(&mut buff)?;
        let buff = buff.trim().trim_start_matches("0x");
        let phys_addr = usize::from_str_radix(buff, 16)?;

        let size_f = format!("/sys/class/u-dma-buf/{}/size", name);
        let mut size_f = File::open(size_f)?;
        let mut buff = String::new();
        size_f.read_to_string(&mut buff)?;
        let buff = buff.trim();
        let size = buff.parse::<usize>()?;

        let debug_f = format!("/sys/class/u-dma-buf/{}/debug_vma", name);
        let mut debug_f = File::open(debug_f)?;
        let mut buff = String::new();
        debug_f.read_to_string(&mut buff)?;
        let debug_vma = buff.trim() != "0";

        let sync_f = format!("/sys/class/u-dma-buf/{}/sync_mode", name);
        let mut sync_f = File::open(sync_f)?;
        let mut buff = String::new();
        sync_f.read_to_string(&mut buff)?;
        let sync_mode = buff.trim() != "0";

        let dev = format!("/dev/{}", name);
        let dev = OpenOptions::new().read(true).write(true).open(dev)?;

        let buffer;
        unsafe {
            buffer = libc::mmap(
                std::ptr::null_mut::<libc::c_void>(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                dev.as_raw_fd(),
                0,
            );
            if buffer == libc::MAP_FAILED {
                anyhow::bail!("mapping dma buffer into virtual memory failed");
            }
        }

        Ok(DmaBuffer {
            name: name.to_string(),
            size,
            phys_addr,
            buffer,
            sync_mode,
            debug_vma,
        })
    }

    #[allow(clippy::mut_from_ref)]
    pub fn slice<T>(&self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self.buffer as *mut T, self.size / mem::size_of::<T>()) }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn phys_addr(&self) -> usize {
        self.phys_addr
    }

    pub fn buffer(&self) -> *mut libc::c_void {
        self.buffer
    }

    pub fn sync_mode(&self) -> bool {
        self.sync_mode
    }

    pub fn debug_vma(&self) -> bool {
        self.debug_vma
    }
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.buffer, self.size);
        }
    }
}

unsafe impl Send for DmaBuffer {}
unsafe impl Sync for DmaBuffer {}

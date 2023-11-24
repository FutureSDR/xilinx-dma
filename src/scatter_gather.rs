use std::ptr;

const NXTDESC: isize = 0x0 / 4;
const NXTDESC_MSB: isize = 0x4 / 4;
const BUFFER_ADDRESS: isize = 0x8 / 4;
const BUFFER_ADDRESS_MSB: isize = 0xC / 4;
const CONTROL: isize = 0x18 / 4;
const STATUS: isize = 0x1C / 4;

#[derive(Debug)]
pub struct SgDescriptor {
    base: *mut u32,
    phys: usize,
}

// Write access to the SgDescriptor requries a mutable reference, so it can even
// be Sync.
unsafe impl Send for SgDescriptor {}
unsafe impl Sync for SgDescriptor {}

// Descriptors are aligned to 16 words, even though only 8 or 8+5 words are used
pub const SG_DESCRIPTOR_LEN: usize = 16 * 4;

impl SgDescriptor {
    pub unsafe fn from_base_ptr(base: *mut u32, phys_addr: usize) -> SgDescriptor {
        SgDescriptor {
            base,
            phys: phys_addr,
        }
    }

    pub fn base_ptr(&self) -> *mut u32 {
        self.base
    }

    pub fn phys_addr(&self) -> usize {
        self.phys
    }

    // About volatile accesses:
    //
    // Non-volatile writes to the descriptor are effective as
    // parts of the side-effects of each method when the method
    // returns. Reordering and write combining does not matter. The only thing
    // that matters is the the descriptor contains the data when the DMA reads
    // it. Therefore, all the writes to the descriptor can be non-volatile.
    //
    // Non-volatile reads observe the effects of previous non-volatile writes
    // (in program order), so for all the descriptor fields except STATUS it is
    // possible to use non-volatile reads, since the DMA does not write to
    // them. On the other hand, the DMA writes to STATUS, so all reads of this
    // field must be volatile to prevent the compiler from removing a read
    // because the field was previously read and not written to by the program.

    pub fn next_descriptor(&self) -> usize {
        unsafe {
            let lsbs = ptr::read(self.base.offset(NXTDESC)) as usize;
            if cfg!(target_pointer_width = "64") {
                let msbs = ptr::read(self.base.offset(NXTDESC_MSB)) as usize;
                (msbs << 32) | lsbs
            } else {
                lsbs
            }
        }
    }

    pub fn set_next_descriptor(&mut self, addr: usize) {
        assert_eq!(addr & 0x3f, 0); // descriptors must be 16-word aligned
        unsafe {
            #[allow(clippy::identity_op)]
            ptr::write(self.base.offset(NXTDESC), (addr & 0xffff_ffff) as u32);
            ptr::write(
                self.base.offset(NXTDESC_MSB),
                (addr & !0xffff_ffff).wrapping_shr(32) as u32,
            );
        }
    }

    pub fn buffer_address(&self) -> usize {
        unsafe {
            let lsbs = ptr::read(self.base.offset(BUFFER_ADDRESS)) as usize;
            if cfg!(target_pointer_width = "64") {
                let msbs = ptr::read(self.base.offset(BUFFER_ADDRESS_MSB)) as usize;
                (msbs << 32) | lsbs
            } else {
                lsbs
            }
        }
    }

    pub fn set_buffer_address(&mut self, addr: usize) {
        unsafe {
            ptr::write(
                self.base.offset(BUFFER_ADDRESS),
                (addr & 0xffff_ffff) as u32,
            );
            ptr::write(
                self.base.offset(BUFFER_ADDRESS_MSB),
                (addr & !0xffff_ffff).wrapping_shr(32) as u32,
            );
        }
    }

    pub fn buffer_length(&self) -> u32 {
        unsafe { ptr::read(self.base.offset(CONTROL)) & 0x3ffffff }
    }

    pub fn set_buffer_length(&mut self, length: u32) {
        // the buffer length field has only 26 bits
        assert!(length <= 0x4000000);
        unsafe {
            let ctrl = ptr::read(self.base.offset(CONTROL));
            ptr::write(self.base.offset(CONTROL), (ctrl & !0x3ffffff) | length);
        }
    }

    pub fn eof(&self) -> bool {
        unsafe { ptr::read(self.base.offset(CONTROL)) & (1 << 26) != 0 }
    }

    pub fn set_eof(&mut self, eof: bool) {
        unsafe {
            let ctrl = ptr::read(self.base.offset(CONTROL));
            ptr::write(
                self.base.offset(CONTROL),
                (ctrl & !(1 << 26)) | (u32::from(eof) << 26),
            );
        }
    }

    pub fn sof(&self) -> bool {
        unsafe { ptr::read(self.base.offset(CONTROL)) & (1 << 27) != 0 }
    }

    pub fn set_sof(&mut self, eof: bool) {
        unsafe {
            let ctrl = ptr::read(self.base.offset(CONTROL));
            ptr::write(
                self.base.offset(CONTROL),
                (ctrl & !(1 << 27)) | (u32::from(eof) << 27),
            );
        }
    }

    pub fn transferred_bytes(&self) -> u32 {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & 0x3ffffff }
    }

    pub fn status_rxeof(&self) -> bool {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & (1 << 26) != 0 }
    }

    pub fn status_rxsof(&self) -> bool {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & (1 << 27) != 0 }
    }

    pub fn dma_internal_error(&self) -> bool {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & (1 << 28) != 0 }
    }

    pub fn dma_slave_error(&self) -> bool {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & (1 << 29) != 0 }
    }

    pub fn dma_decode_error(&self) -> bool {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & (1 << 30) != 0 }
    }

    pub fn completed(&self) -> bool {
        unsafe { ptr::read_volatile(self.base.offset(STATUS)) & (1 << 31) != 0 }
    }

    pub fn set_completed(&self, completed: bool) {
        unsafe {
            let status = ptr::read_volatile(self.base.offset(STATUS));
            ptr::write(
                self.base.offset(STATUS),
                (status & 0x7fffffff) | (u32::from(completed) << 31),
            );
        }
    }

    pub fn clear_status(&mut self) {
        unsafe {
            ptr::write(self.base.offset(STATUS), 0);
        }
    }
}

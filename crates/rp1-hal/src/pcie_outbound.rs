//! Fixed Linux reserved DDR contract, proved on CM5/RP1 (2026-09-29).
//! M3 0x81000000 + x -> fabric 0x8001000000 + x -> PCIe 0x1021000000 + x.
//! CPU PA is 0x21000000 + x. Only this 64-KiB reservation is accessible here.
use core::{marker::PhantomData, ptr};

pub const SIZE: usize = 0x10000;
pub const REGION0: [u32; 8] = [0, 0x8000_0000, 0x0100_0000, 0x80,
    0x0100_ffff, 0x2100_0000, 0x10, 0];
pub const OPENAMP_REGION1: [u32; 8] = [0, 0x8000_0000, 0x0200_0000, 0x80,
    0x0200_ffff, 0x2110_0000, 0x10, 0];
const LOCAL: usize = 0x8100_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Alignment, Overflow, Bounds }

fn address(offset: usize, words: usize) -> Result<usize, Error> {
    if offset & 3 != 0 { return Err(Error::Alignment); }
    let bytes = words.checked_mul(4).ok_or(Error::Overflow)?;
    let end = offset.checked_add(bytes).ok_or(Error::Overflow)?;
    if offset > SIZE || end > SIZE { return Err(Error::Bounds); }
    LOCAL.checked_add(offset).ok_or(Error::Overflow)
}

/// Validate a request without issuing any MMIO/PCIe access. Byte accesses and
/// misaligned requests are deliberately unsupported by the proved u32 path.
pub fn validate_request(offset: usize, length: usize, width: usize) -> Result<(), Error> {
    if width != 4 || length & 3 != 0 { return Err(Error::Alignment); }
    address(offset, length / 4).map(|_| ())
}
pub fn barrier() {
    #[cfg(target_arch = "arm")]
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

/// Unique local owner. The Linux side must follow the publication protocol.
pub struct ReservedDdrWindow { _owner: PhantomData<*mut ()> }
impl ReservedDdrWindow {
    /// # Safety
    /// The caller must have read back region 0 == REGION0, restored the DBI
    /// selector, and excluded other configuration writers for this owner's
    /// lifetime. The Linux reservation/root window must match this module.
    /// No cacheable alias or competing payload owner may access the test area.
    pub unsafe fn from_verified_region0() -> Self { Self { _owner: PhantomData } }
    pub fn read32(&self, offset: usize) -> Result<u32, Error> {
        let p = address(offset, 1)? as *const u32;
        Ok(unsafe { ptr::read_volatile(p) })
    }
    pub fn write32(&mut self, offset: usize, value: u32) -> Result<(), Error> {
        let p = address(offset, 1)? as *mut u32;
        unsafe { ptr::write_volatile(p, value); } Ok(())
    }
    pub fn read_words(&self, offset: usize, dst: &mut [u32]) -> Result<(), Error> {
        let p = address(offset, dst.len())? as *const u32;
        for (i, v) in dst.iter_mut().enumerate() { *v = unsafe { p.add(i).read_volatile() }; }
        Ok(())
    }
    pub fn write_words(&mut self, offset: usize, src: &[u32]) -> Result<(), Error> {
        let p = address(offset, src.len())? as *mut u32;
        for (i, v) in src.iter().enumerate() { unsafe { p.add(i).write_volatile(*v); } }
        Ok(())
    }
    /// Drain prior posted writes using a non-posted read of the same DDR window.
    /// REGION0 selects Memory, function 0, TC0, RO=0, IDO=0 and No-Snoop=0.
    /// PCIe default ordering prevents this read passing those writes. DSB alone
    /// only drains the M3/AXI side; the returned read completion is essential.
    /// The caller must publish BAR2 response only after this method returns.
    pub fn complete_posted_write(&mut self) -> Result<u32, Error> {
        self.complete_posted_write_at(0)
    }
    /// Complete via a word owned by this producer. Duplex users must not read
    /// a peer's unpublished buffer merely to drain their own posted writes.
    pub fn complete_posted_write_at(&mut self, offset: usize) -> Result<u32, Error> {
        address(offset, 1)?;
        barrier();
        let value = self.read32(offset)?;
        barrier(); Ok(value)
    }
}

#[cfg(test)]
mod tests {
    #[test] fn invalid_requests_issue_no_transaction() {
        use super::*;
        // These methods would fault on a host if they touched the RP1 aperture.
        let mut w = unsafe { ReservedDdrWindow::from_verified_region0() };
        assert_eq!(w.write32(SIZE, 1), Err(Error::Bounds));
        assert_eq!(w.read32(2), Err(Error::Alignment));
        assert_eq!(w.write_words(SIZE - 4, &[1,2]), Err(Error::Bounds));
        assert_eq!(w.read_words(usize::MAX - 3, &mut [0]), Err(Error::Overflow));
        for offset in (0..SIZE).step_by(4) { assert!(validate_request(offset,4,4).is_ok()); }
        for page in [4096,16384] { assert!(validate_request(page-4,8,4).is_ok()); }
        assert!(validate_request(0,4,1).is_err());
        assert!(validate_request(0,3,4).is_err());
    }
    #[test] fn fixed_window_bounds() {
        use super::*;
        assert_eq!(address(0, 1), Ok(LOCAL));
        assert_eq!(address(SIZE - 4, 1), Ok(LOCAL + SIZE - 4));
        assert_eq!(address(SIZE, 0), Ok(LOCAL + SIZE));
        assert_eq!(address(2, 1), Err(Error::Alignment));
        assert_eq!(address(SIZE, 1), Err(Error::Bounds));
        assert_eq!(address(SIZE - 4, 2), Err(Error::Bounds));
        assert_eq!(address(0, usize::MAX), Err(Error::Overflow));
        assert_eq!(address(usize::MAX - 3, 1), Err(Error::Overflow));
        assert_eq!(REGION0[4] - REGION0[2] + 1, SIZE as u32);
        assert_eq!((u64::from(REGION0[6]) << 32) | u64::from(REGION0[5]), 0x10_2100_0000);
    }
}

//! Plain R1 cold prerequisite, never SCMI protocol/rate-set or warm adoption.
//! The selected PRIM writes/readbacks are from the existing active-DMAC proof;
//! no DMA access, UART driver/pin setup or new reset value is included here.
const PLL_RESET: u32 = 1 << 29;
const UART_RESET: u32 = 1 << 26;
const PRIM: usize = 0x4002_0010;
const ADDRESSES: [usize; 13] = [
    0x4001_4000, 0x4001_4018, 0x4001_4004, 0x4001_401c,
    0x4002_0000, 0x4002_0004, 0x4002_0008, 0x4002_000c,
    PRIM, 0x4002_0014, 0x4001_8054, 0x4001_8058, 0x4001_8060,
];

#[cfg(not(test))]
fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
#[cfg(not(test))]
fn write_prim(value: u32) {
    unsafe {
        (PRIM as *mut u32).write_volatile(value);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn snapshot() -> [u32; 13] { ADDRESSES.map(read) }

/// Before any clock/reset write, require stable cold PLL, UART clock and both
/// selected resets. Running 0x77010 AND already-correct 0x51010 fail closed.
pub fn prepare() -> Result<(), &'static str> {
    let before = snapshot();
    if before != snapshot()
        || before[0] & PLL_RESET == 0 || before[1] & PLL_RESET != 0
        || before[2] & UART_RESET == 0 || before[3] & UART_RESET != 0
        || before[4..] != [1, 0x3f, 0, 0, 0x77000, 0x8001_0000, 0, 1, 1]
    { return Err("not stable cold defaults"); }

    crate::release_pll_sys_reset_bit29().map_err(|_| "PLL reset")?;
    if crate::pll_sys_core_lock_transition().decision != crate::PllSysCoreLockDecision::Locked {
        return Err("PLL core lock");
    }
    if snapshot()[4..10] != [0x8000_0001, 4, 20, 0, 0x77000, 0x8001_0000] {
        return Err("PLL locked tuple");
    }
    // Two bounded writes, each followed by DSB and exact readback. Do not use
    // enable_pll_sys_pri_ph_bit4(): that intentionally retains R2's /7 /7.
    write_prim(0x51000);
    if read(PRIM) != 0x51000 { return Err("PLL divider readback"); }
    write_prim(0x51010);
    if read(PRIM) != 0x51010 { return Err("PLL phase readback"); }
    crate::release_uart0_reset_bank1_bit26().map_err(|_| "UART clock/reset")?;

    let after = snapshot();
    if after != snapshot()
        || after[0] & PLL_RESET != 0 || after[1] & PLL_RESET == 0
        || after[2] & UART_RESET != 0 || after[3] & UART_RESET == 0
        || after[4..] != [0x8000_0001, 4, 20, 0, 0x51010, 0x8001_0000, 0x1000_0840, 1, 1]
    { return Err("final clock/reset tuple"); }
    Ok(())
}

// Compile this same prepare body on the host. Only the MMIO primitives and
// existing hardware helpers are substituted; rejection must cause NO writes.
#[cfg(test)]
use tests::{read, write_prim, release_pll_sys_reset_bit29, pll_sys_core_lock_transition,
    release_uart0_reset_bank1_bit26, PllSysCoreLockDecision};
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    const COLD: [u32; 13] = [PLL_RESET, 0, UART_RESET, 0, 1, 0x3f, 0, 0,
        0x77000, 0x8001_0000, 0, 1, 1];
    struct Mock { registers: [u32; 13], writes: Vec<u32>, reject_prim: u32, reads: usize, drift: bool }
    thread_local! { static HW: RefCell<Mock> = RefCell::new(Mock {
        registers: COLD, writes: Vec::new(), reject_prim: 0, reads: 0, drift: false }); }
    fn seed(registers: [u32; 13]) {
        HW.with(|h| *h.borrow_mut() = Mock { registers, writes: Vec::new(), reject_prim: 0, reads: 0, drift: false });
    }
    pub fn read(address: usize) -> u32 {
        HW.with(|h| { let mut h = h.borrow_mut(); h.reads += 1;
            let value = h.registers[ADDRESSES.iter().position(|&a| a == address).unwrap()];
            if h.drift && h.reads == 26 { value ^ 1 } else { value }
        })
    }
    pub fn write_prim(value: u32) {
        HW.with(|h| { let mut h = h.borrow_mut(); h.writes.push(value);
            if h.reject_prim != value { h.registers[8] = value; }
        });
    }
    pub fn release_pll_sys_reset_bit29() -> Result<(), ()> {
        HW.with(|h| { let mut h = h.borrow_mut(); h.writes.push(PLL_RESET);
            h.registers[0] &= !PLL_RESET; h.registers[1] |= PLL_RESET; }); Ok(())
    }
    #[derive(PartialEq)] pub enum PllSysCoreLockDecision { Locked }
    pub struct Lock { pub decision: PllSysCoreLockDecision }
    pub fn pll_sys_core_lock_transition() -> Lock {
        HW.with(|h| { let mut h = h.borrow_mut(); h.writes.push(20);
            h.registers[4..8].copy_from_slice(&[0x8000_0001, 4, 20, 0]); });
        Lock { decision: PllSysCoreLockDecision::Locked }
    }
    pub fn release_uart0_reset_bank1_bit26() -> Result<(), ()> {
        HW.with(|h| { let mut h = h.borrow_mut(); h.writes.push(UART_RESET);
            h.registers[2] &= !UART_RESET; h.registers[3] |= UART_RESET;
            h.registers[10] = 0x1000_0840; }); Ok(())
    }
    #[test] fn cold_defaults_take_only_selected_sequence() {
        seed(COLD); assert_eq!(prepare(), Ok(()));
        HW.with(|h| assert_eq!(h.borrow().writes, [PLL_RESET, 20, 0x51000, 0x51010, UART_RESET]));
        assert!(prepare().is_err()); // No warm replay, even at the desired rate.
        HW.with(|h| assert_eq!(h.borrow().writes.len(), 5));
    }
    #[test] fn running_and_mismatched_snapshots_never_write() {
        for prim in [0x77010, 0x51010, 0x51000] {
            let mut s = COLD; s[4..9].copy_from_slice(&[0x8000_0001, 4, 20, 0, prim]);
            seed(s); assert!(prepare().is_err()); HW.with(|h| assert!(h.borrow().writes.is_empty()));
        }
        for i in 0..13 {
            let mut s = COLD; s[i] ^= match i { 0|1 => PLL_RESET, 2|3 => UART_RESET, _ => 1 };
            seed(s); assert!(prepare().is_err()); HW.with(|h| assert!(h.borrow().writes.is_empty()));
        }
        seed(COLD); HW.with(|h| h.borrow_mut().drift = true);
        assert!(prepare().is_err()); HW.with(|h| assert!(h.borrow().writes.is_empty()));
    }
    #[test] fn divider_or_phase_readback_failure_stops_before_uart() {
        for (reject, count) in [(0x51000, 3), (0x51010, 4)] {
            seed(COLD); HW.with(|h| h.borrow_mut().reject_prim = reject);
            assert!(prepare().is_err()); HW.with(|h| assert_eq!(h.borrow().writes.len(), count));
        }
    }
}

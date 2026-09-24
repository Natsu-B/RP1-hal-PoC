//! Read-only adoption of the narrowly proven UART/XOSC and PLL_SYS tuples.
//! No cold initialization, PLL search or rate/gate writer lives here.
use crate::scmi_clock::{ClockHardware, Error, PhysicalState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UartClockSnapshot {
    pub ctrl: u32,
    pub div_int: u32,
    pub sel: u32,
}

impl UartClockSnapshot {
    /// Only nominal 50 MHz XOSC/DIV1 is admitted by the existing PL011 27/8
    /// divisor. SEL=1 is the observed one-hot selector, not AUXSRC's value 2.
    /// External baud accuracy remains a separate hardware measurement.
    pub fn matches(&self, expected_hz: u32) -> bool {
        expected_hz == 50_000_000 && self.ctrl == 0x1000_0840
            && self.div_int == 1 && self.sel == 1
    }
}

pub fn snapshot_uart_clock() -> UartClockSnapshot {
    UartClockSnapshot { ctrl: read(0x4001_8054), div_int: read(0x4001_8058), sel: read(0x4001_8060) }
}

pub fn adopt_uart_clock(expected_hz: u32) -> Result<UartClockSnapshot, UartClockSnapshot> {
    let a = snapshot_uart_clock();
    let b = snapshot_uart_clock();
    if a == b && b.matches(expected_hz) { Ok(b) } else { Err(b) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SysPllSnapshot {
    pub cs: u32, pub pwr: u32, pub fb_int: u32, pub fb_frac: u32, pub prim: u32,
}

impl SysPllSnapshot {
    pub fn known_apb(&self) -> Result<PhysicalState, Error> {
        // Existing core lock tuple, 50MHz * 20 / (5 * 1) / PRI_PH's fixed 2.
        // A different (even mathematically equivalent) tuple is not admitted.
        if self.cs != 0x8000_0001 || self.pwr != 4 || self.fb_int != 20
            || self.fb_frac != 0 || self.prim & !0x10 != 0x0005_1000 {
            return Err(Error::Hardware);
        }
        Ok(PhysicalState { rate_hz: 100_000_000, enabled: self.prim & 0x10 != 0 })
    }
}

fn sys_pll() -> SysPllSnapshot {
    SysPllSnapshot { cs: read(0x4002_0000), pwr: read(0x4002_0004),
        fb_int: read(0x4002_0008), fb_frac: read(0x4002_000c), prim: read(0x4002_0010) }
}

/// Initial SCMI backend. Registers are read twice; drift fails closed.
/// Even if Server enables vote handling, this backend never writes a gate.
pub struct ReadOnlyUartApb;
impl ClockHardware for ReadOnlyUartApb {
    fn read(&mut self, rp1_id: u32) -> Result<PhysicalState, Error> {
        if rp1_id != 6 { return Err(Error::NotFound); }
        let a = sys_pll(); let b = sys_pll();
        if a != b { return Err(Error::Hardware); }
        b.known_apb()
    }
    fn enable(&mut self, _: u32, _: bool) -> Result<(), Error> { Err(Error::Denied) }
}

fn read(address: usize) -> u32 {
    // Only this module's fixed allowlisted registers call this private helper.
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn xosc_selector_is_not_auxsrc_number() {
        let mut s = UartClockSnapshot { ctrl: 0x1000_0840, div_int: 1, sel: 1 };
        assert!(s.matches(50_000_000)); assert!(!s.matches(48_000_000));
        s.sel = 4; assert!(!s.matches(50_000_000));
        s.sel = 1; s.ctrl &= !(1 << 11); assert!(!s.matches(50_000_000));
        s.ctrl = 0x840; assert!(!s.matches(50_000_000));
    }
    #[test] fn apb_includes_phase_fixed_divider_and_exact_tuple() {
        let mut s = SysPllSnapshot { cs: 0x8000_0001, pwr: 4, fb_int: 20, fb_frac: 0, prim: 0x51010 };
        assert_eq!(s.known_apb(), Ok(PhysicalState { rate_hz: 100_000_000, enabled: true }));
        s.prim &= !0x10; assert!(!s.known_apb().unwrap().enabled);
        s.fb_frac = 1; assert_eq!(s.known_apb(), Err(Error::Hardware));
    }
    #[test] fn generated_policy_matches_admitted_readonly_backends() {
        for c in crate::clock_profile_generated::CLOCKS {
            if c.scmi_id.is_some() {
                assert_eq!(c.rp1_id, 6);
                let s = SysPllSnapshot { cs: 0x8000_0001, pwr: 4, fb_int: 20, fb_frac: 0, prim: 0x51010 };
                assert_eq!(s.known_apb().unwrap().rate_hz, u64::from(c.rate_hz));
            }
            if c.name == "uart" {
                assert!(UartClockSnapshot { ctrl: 0x1000_0840, div_int: 1, sel: 1 }.matches(c.rate_hz));
            }
        }
    }
}

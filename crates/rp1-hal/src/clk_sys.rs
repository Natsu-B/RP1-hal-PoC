//! Bounded clk_sys source switching; never programs a PLL or a divider.
//! Use before starting either core's timing-sensitive work, with other clock
//! writers excluded. Recalibrate CPU-based timers after a successful switch.
use crate::clock_adopt::SysPllSnapshot;

const CTRL: usize = 0x4001_8014;
const DIV_INT: usize = 0x4001_8018;
const SEL: usize = 0x4001_8020;
const PLL_CS: usize = 0x4002_0000;
const PLL_PWR: usize = 0x4002_0004;
const PLL_FB_INT: usize = 0x4002_0008;
const PLL_FB_FRAC: usize = 0x4002_000c;
const PLL_PRIM: usize = 0x4002_0010;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source { Xosc, PllSys200Mhz }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    PllUnlocked,
    UnsupportedPll,
    UnsupportedDivider,
    /// The source write was issued; the current clock is not established.
    SelectTimeout { selected: u32 },
}

/// Select 50 MHz XOSC or the already locked 200 MHz PLL_SYS primary output.
/// Requires clk_sys DIV_INT=1; the PLL path admits only the established
/// integer 1 GHz VCO /5 /1 tuple. APB's independent phase gate is untouched.
/// A timeout does not undo the source write. XOSC recovery needs no PLL lock.
pub fn switch(source: Source) -> Result<u32, Error> {
    switch_with(&mut Mmio, source, 100_000)
}

trait Registers {
    fn read(&mut self, address: usize) -> u32;
    fn write_ctrl(&mut self, value: u32);
}
struct Mmio;
impl Registers for Mmio {
    fn read(&mut self, address: usize) -> u32 {
        unsafe { core::ptr::read_volatile(address as *const u32) }
    }
    fn write_ctrl(&mut self, value: u32) {
        unsafe { core::ptr::write_volatile(CTRL as *mut u32, value); }
    }
}

fn switch_with(regs: &mut impl Registers, source: Source, polls: u32) -> Result<u32, Error> {
    if regs.read(DIV_INT) != 1 { return Err(Error::UnsupportedDivider); }
    if source == Source::PllSys200Mhz {
        let pll = SysPllSnapshot { cs: regs.read(PLL_CS), pwr: regs.read(PLL_PWR),
            fb_int: regs.read(PLL_FB_INT), fb_frac: regs.read(PLL_FB_FRAC),
            prim: regs.read(PLL_PRIM) };
        if pll.cs & (1 << 31) == 0 { return Err(Error::PllUnlocked); }
        pll.known_apb().map_err(|_| Error::UnsupportedPll)?;
    }
    let (src, selected, hz) = match source {
        Source::Xosc => (0, 1, 50_000_000),
        Source::PllSys200Mhz => (2, 4, 200_000_000),
    };
    let ctrl = regs.read(CTRL);
    regs.write_ctrl((ctrl & !3) | src);
    let mut last = 0;
    for _ in 0..polls {
        last = regs.read(SEL);
        if last == selected { return Ok(hz); }
    }
    Err(Error::SelectTimeout { selected: last })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake {
        values: [u32; 7],
        selections: [u32; 3],
        reads: usize,
        writes: usize,
    }
    impl Fake {
        fn new() -> Self {
            Self { values: [0x1000_0800, 1, 0x8000_0001, 4, 20, 0, 0x51010],
                selections: [1, 0, 4], reads: 0, writes: 0 }
        }
    }
    impl Registers for Fake {
        fn read(&mut self, address: usize) -> u32 {
            if address == SEL {
                let value = self.selections[self.reads.min(2)];
                self.reads += 1;
                return value;
            }
            let index = [CTRL, DIV_INT, PLL_CS, PLL_PWR, PLL_FB_INT, PLL_FB_FRAC, PLL_PRIM]
                .iter().position(|&a| a == address).unwrap();
            self.values[index]
        }
        fn write_ctrl(&mut self, value: u32) {
            self.values[0] = value;
            self.writes += 1;
        }
    }

    #[test] fn waits_for_exact_one_hot_selection_and_preserves_ctrl_bits() {
        let mut r = Fake::new();
        let original = r.values;
        assert_eq!(switch_with(&mut r, Source::PllSys200Mhz, 3), Ok(200_000_000));
        assert_eq!(r.reads, 3);
        assert_eq!(r.writes, 1);
        assert_eq!(r.values[0], original[0] | 2);
        assert_eq!(&r.values[1..], &original[1..]);
    }

    #[test] fn rejects_unlocked_or_unproven_pll_without_writing() {
        for (index, value, error) in [
            (2, 1, Error::PllUnlocked), (2, 0x8000_0002, Error::UnsupportedPll),
            (3, 5, Error::UnsupportedPll), (3, 0xc, Error::UnsupportedPll),
            (4, 21, Error::UnsupportedPll), (5, 1, Error::UnsupportedPll),
            (6, 0x41010, Error::UnsupportedPll), (6, 0x52010, Error::UnsupportedPll),
        ] {
            let mut r = Fake::new(); r.values[index] = value;
            assert_eq!(switch_with(&mut r, Source::PllSys200Mhz, 3), Err(error));
            assert_eq!(r.writes, 0); assert_eq!(r.reads, 0);
        }
    }

    #[test] fn phase_gate_is_independent_of_primary_output() {
        let mut r = Fake::new(); r.values[6] &= !0x10;
        assert_eq!(switch_with(&mut r, Source::PllSys200Mhz, 3), Ok(200_000_000));
    }

    #[test] fn xosc_recovery_does_not_require_a_locked_pll() {
        let mut r = Fake::new(); r.values[0] |= 2; r.values[2] = 0;
        r.selections = [4, 0, 1];
        assert_eq!(switch_with(&mut r, Source::Xosc, 3), Ok(50_000_000));
        assert_eq!(r.values[0], 0x1000_0800);
    }

    #[test] fn requires_divide_by_one_for_both_sources() {
        for source in [Source::Xosc, Source::PllSys200Mhz] {
            for divider in [0, 2, u32::MAX] {
                let mut r = Fake::new(); r.values[1] = divider;
                assert_eq!(switch_with(&mut r, source, 3), Err(Error::UnsupportedDivider));
                assert_eq!(r.writes, 0);
            }
        }
    }

    #[test] fn timeout_is_bounded_and_reports_the_last_selection() {
        for (source, selected) in [(Source::PllSys200Mhz, 2), (Source::Xosc, 5)] {
            let mut r = Fake::new(); r.selections = [selected; 3];
            assert_eq!(switch_with(&mut r, source, 7), Err(Error::SelectTimeout { selected }));
            assert_eq!(r.reads, 7); assert_eq!(r.writes, 1);
        }
    }
}

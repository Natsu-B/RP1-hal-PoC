//! Cold-only proc0 admission; pending bits are deliberately left untouched.
#[inline]
pub fn inactive(enabled: [u32; 2], active: [u32; 2], ipsr: u32) -> bool {
    enabled[0] | enabled[1] | active[0] | active[1] | ipsr == 0
}

#[cfg(target_arch = "arm")]
#[inline(never)]
#[unsafe(link_section = ".amp_observation_codec")]
pub unsafe fn check() -> bool {
    let ipsr: u32;
    unsafe {
        core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack));
        let enabled = [(0xe000_e100 as *const u32).read_volatile(),
                       (0xe000_e104 as *const u32).read_volatile()];
        let active = [(0xe000_e300 as *const u32).read_volatile(),
                      (0xe000_e304 as *const u32).read_volatile()];
        inactive(enabled, active, ipsr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn any_enabled_or_active_irq_is_refused() {
        assert!(inactive([0; 2], [0; 2], 0));
        for bank in 0..2 {
            for bit in 0..32 {
                let mut changed = [0; 2]; changed[bank] = 1 << bit;
                assert!(!inactive(changed, [0; 2], 0));
                assert!(!inactive([0; 2], changed, 0));
            }
        }
        assert!(!inactive([u32::MAX; 2], [u32::MAX; 2], 0));
    }
    #[test]
    fn exception_context_is_refused() {
        for ipsr in 1..80 { assert!(!inactive([0; 2], [0; 2], ipsr)); }
        assert!(!inactive([0; 2], [0; 2], u32::MAX));
    }
}

//! Exact-address, non-clearing observations around ordinary I2C1 host setup.

pub const MAGIC: u32 = 0x3152_3149; // I1R1, not an IRQ-delivery claim.
pub const WORDS: usize = 40;
pub const READY_MARKER: u32 = 465;
pub const SUCCESS_MARKER: u32 = 467;
pub const FAILURE_MARKER: u32 = 589;
const FAIL_SETUP: u32 = 0x302;

pub fn record(setup_ok: bool, before: [u32; 18], after: [u32; 18]) -> [u32; WORDS] {
    let mut words = [0; WORDS];
    words[..4].copy_from_slice(&[MAGIC, 1, if setup_ok { 1 } else { FAIL_SETUP }, 2]);
    words[4..22].copy_from_slice(&before);
    words[22..40].copy_from_slice(&after);
    words
}

#[cfg(target_arch = "arm")]
#[inline(never)]
pub fn capture(stage_id: u32) -> [u32; 18] {
    let base = rp1_hal::addr::I2C1_BASE;
    let wrapper = unsafe { core::ptr::read_volatile((base + 0x108) as *const u32) };
    let param = unsafe { core::ptr::read_volatile((base + 0xf4) as *const u32) };
    let version = unsafe { core::ptr::read_volatile((base + 0xf8) as *const u32) };
    let component_type = unsafe { core::ptr::read_volatile((base + 0xfc) as *const u32) };
    let source = rp1_hal::i2c::i2c1_irq_snapshot();
    let route = rp1_rt::i2c1_irq_route_snapshot();
    [
        stage_id,
        wrapper,
        param,
        version,
        component_type,
        source.raw_interrupt_status,
        source.masked_interrupt_status,
        source.interrupt_mask,
        source.abort_source,
        source.enable_status,
        route.vtor,
        route.iser0,
        route.iser1,
        route.ispr0,
        route.ispr1,
        route.iabr0,
        route.iabr1,
        route.primask,
    ]
}

#[cfg(target_arch = "arm")]
pub fn invalidate() {
    unsafe {
        core::ptr::write_volatile(rp1_hal::debug::MAILBOX_ADDR as *mut u32, 0);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(target_arch = "arm")]
#[inline(never)]
pub fn publish(words: [u32; WORDS]) {
    const _: () = assert!(WORDS * 4 == 160 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC); // Last body word is 0x2000_fc9c.
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_record_retains_both_complete_stages_even_on_setup_failure() {
        let mut before = core::array::from_fn(|i| 0x1000 + i as u32);
        let mut after = core::array::from_fn(|i| 0x2000 + i as u32);
        before[0] = 0;
        after[0] = 1;
        assert_eq!(MAGIC.to_le_bytes(), *b"I1R1");
        assert_eq!(WORDS * 4, 160);
        assert_eq!(
            [READY_MARKER, SUCCESS_MARKER, FAILURE_MARKER],
            [465, 467, 589]
        );
        for (ok, decision) in [(true, 1), (false, FAIL_SETUP)] {
            let words = record(ok, before, after);
            assert_eq!(words[..4], [MAGIC, 1, decision, 2]);
            assert_eq!(words[4..22], before);
            assert_eq!(words[22..40], after);
            assert_eq!([words[21], words[39]], [before[17], after[17]]);
        }
        assert_ne!(FAIL_SETUP, 1);
    }
}

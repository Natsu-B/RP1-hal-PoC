//! Isolated wrapper OR1 / real STOP / candidate NVIC8 gate. No RX claim.

pub const MAGIC: u32 = 0x3153_3149; // I1S1
pub const WORDS: usize = 128;
pub const READY_MARKER: u32 = 469;
pub const SUCCESS_MARKER: u32 = 471;
pub const FAILURE_MARKER: u32 = 591;
pub const PASS: u32 = 1;
const SAFE_NEGATIVE: u32 = 2;
const FAIL_PRESTATE: u32 = 0x401;
const FAIL_SETUP: u32 = 0x402;
const FAIL_WRAPPER: u32 = 0x403;
const FAIL_NO_START: u32 = 0x404;
const FAIL_ARM: u32 = 0x405;
const FAIL_START: u32 = 0x406;
const FAIL_SOURCE: u32 = 0x407;
const FAIL_SOURCE_STATE: u32 = 0x408;
const FAIL_ISR: u32 = 0x409;
const FAIL_CLEANUP: u32 = 0x40a;
const STOP: u32 = 1 << 9;
const ABRT: u32 = 1 << 6;
const IRQ_BIT: u32 = 1 << 8;
const QUIET_US: u32 = 4_000;
const DEADLINE_US: u32 = 100_000;
const POLL_LIMIT: usize = 2_000_000;
const PACKET: [u8; 20] = [
    0x44, 0x31, 0x44, 0x52, 0x01, 0x49, 0x01, 0x09, 0xdf, 0x9b, 0x57, 0x13, 0xe0, 0xac, 0x68, 0x24,
    0x31, 0x43, 0x32, 0x49,
];
const WITNESS: u32 = 0x8249_142d;

fn new_record() -> [u32; WORDS] {
    let mut words = [0; WORDS];
    words[..4].copy_from_slice(&[MAGIC, 1, FAIL_PRESTATE, 6]);
    for stage in 0..6 {
        words[8 + 18 * stage] = stage as u32;
    }
    words
}

fn store_stage(words: &mut [u32; WORDS], stage: [u32; 18]) {
    let id = stage[0] as usize;
    assert!(id < 6);
    words[8 + 18 * id..26 + 18 * id].copy_from_slice(&stage);
    words[4] |= 1 << id;
}

fn identity(stage: &[u32; 18], wrapper: u32) -> bool {
    stage[1..5] == [wrapper, 0x001f_1fea, 0x3230_322a, 0x4457_0140]
}

fn route(stage: &[u32; 18], primask: u32, enabled: u32, pending_allowed: u32) -> bool {
    primask <= 1
        && stage[10] == 0x2000_0000
        && stage[11] == enabled
        && stage[12] == 0
        && stage[13] & !pending_allowed == 0
        && stage[14] == 1 << 21
        && stage[15] == 0
        && stage[16] == 0
        && stage[17] == primask
}

fn baseline(stage: &[u32; 18], mask: u32, primask: u32) -> bool {
    identity(stage, 0) && stage[5..10] == [0, 0, mask, 0, 0] && route(stage, primask, 0, 0)
}

fn armed(stage: &[u32; 18], primask: u32) -> bool {
    identity(stage, 1)
        && stage[5] & (STOP | ABRT) == 0
        && stage[6..10] == [0, STOP, 0, 1]
        && route(stage, primask, 0, 0)
}

fn source(stage: &[u32; 18], primask: u32) -> bool {
    identity(stage, 1)
        && stage[5] & (STOP | ABRT) == STOP
        && stage[6..10] == [STOP, STOP, 0, 1]
        && route(stage, primask, 0, IRQ_BIT)
}

fn delivery_decision(
    count: u32,
    ipsr: u32,
    raw: u32,
    masked: u32,
    abort: u32,
    elapsed: u32,
) -> u32 {
    if count == 0 && elapsed >= DEADLINE_US {
        SAFE_NEGATIVE
    } else if count == 1
        && ipsr == 24
        && raw & (STOP | ABRT) == STOP
        && masked == STOP
        && abort == 0
    {
        PASS
    } else {
        FAIL_ISR
    }
}

#[cfg(target_arch = "arm")]
mod hardware {
    use super::*;
    use crate::i2c1_wrapper_readonly_proof::capture;
    use core::sync::atomic::{AtomicU32, Ordering};
    use rp1_hal::{
        gpio::Pin,
        i2c::{I2c1, I2c1Host},
    };

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static RAW: AtomicU32 = AtomicU32::new(0);
    static MASKED: AtomicU32 = AtomicU32::new(0);
    static ABORT: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn I2C1_IRQHandler() {
        unsafe { rp1_rt::mask_i2c1_irq8_one_entry() };
        let source = rp1_hal::i2c::i2c1_irq_snapshot();
        rp1_hal::i2c::i2c1_mask_stop_det_irq();
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let old = COUNT.load(Ordering::Relaxed);
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            RAW.store(source.raw_interrupt_status, Ordering::Relaxed);
            MASKED.store(source.masked_interrupt_status, Ordering::Relaxed);
            ABORT.store(source.abort_source, Ordering::Relaxed);
        }
        rp1_hal::i2c::i2c1_ack_stop_det_irq(source);
        COUNT.store(old.wrapping_add(1), Ordering::Release);
    }

    fn now() -> u32 {
        // Single established RAWL read: finite even if the timer stops.
        unsafe { core::ptr::read_volatile(0x400a_c028 as *const u32) }
    }

    fn quiet(mut valid: impl FnMut() -> bool) -> (bool, u32) {
        let start = now();
        for _ in 0..POLL_LIMIT {
            let ok = valid();
            let elapsed = now().wrapping_sub(start);
            if !ok || elapsed >= QUIET_US {
                return (ok, elapsed);
            }
            core::hint::spin_loop();
        }
        (false, now().wrapping_sub(start))
    }

    #[inline(never)]
    fn wrapper_or_once() -> bool {
        let address = 0x4007_4108 as *mut u32;
        unsafe {
            let before = core::ptr::read_volatile(address);
            if before != 0 {
                return false;
            }
            core::ptr::write_volatile(address, before | 1);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::read_volatile(address) == 1
        }
    }

    fn experiment(host: &mut I2c1Host, words: &mut [u32; WORDS]) -> u32 {
        let primask = words[5];
        words[127] |= 1 << 2;
        let wrapper_ok = wrapper_or_once();
        let post = capture(2);
        store_stage(words, post);
        if !wrapper_ok
            || !identity(&post, 1)
            || post[5..10] != [0; 5]
            || !route(&post, primask, 0, 0)
        {
            return FAIL_WRAPPER;
        }
        let arm_ok = host.arm_stop_det_irq_preserving_causes(0x2d).is_ok();
        let post = capture(2);
        store_stage(words, post);
        if !arm_ok || !armed(&post, primask) {
            return FAIL_ARM;
        }
        let (no_start, elapsed) =
            quiet(|| armed(&capture(2), primask) && COUNT.load(Ordering::Acquire) == 0);
        words[119] = elapsed;
        store_stage(words, capture(2));
        if !no_start {
            return FAIL_NO_START;
        }
        words[127] |= 1 << 3;

        match host.start_write(&PACKET) {
            Ok(queued) => {
                words[124] = 0x8000_0000
                    | (queued.last_command << 16)
                    | (u32::from(queued.bytes_queued) << 8)
                    | 0x2d;
                words[127] |= 1 << 4;
                if words[124] != WITNESS {
                    return FAIL_START;
                }
            }
            Err(_) => {
                store_stage(words, capture(3));
                return FAIL_START;
            }
        }
        let start = now();
        let mut asserted = false;
        for _ in 0..POLL_LIMIT {
            let observed = capture(3);
            store_stage(words, observed);
            words[120] = now().wrapping_sub(start);
            if !identity(&observed, 1)
                || !route(&observed, primask, 0, IRQ_BIT)
                || observed[5] & ABRT != 0
                || observed[7] != STOP
                || observed[8] != 0
                || observed[9] != 1
                || COUNT.load(Ordering::Acquire) != 0
            {
                return FAIL_SOURCE_STATE;
            }
            if source(&observed, primask) {
                asserted = true;
                break;
            }
            if words[120] >= DEADLINE_US {
                break;
            }
            core::hint::spin_loop();
        }
        if !asserted {
            return FAIL_SOURCE;
        }
        words[127] |= 1 << 5;
        unsafe { rp1_rt::enable_i2c1_irq8_after_source_asserted() };
        words[127] |= 1 << 6;
        let start = now();
        for _ in 0..POLL_LIMIT {
            words[121] = now().wrapping_sub(start);
            if COUNT.load(Ordering::Acquire) != 0 || words[121] >= DEADLINE_US {
                break;
            }
            core::hint::spin_loop();
        }
        let post = capture(4);
        store_stage(words, post);
        let count = COUNT.load(Ordering::Acquire);
        if !identity(&post, 1)
            || !route(&post, 0, if count == 0 { IRQ_BIT } else { 0 }, IRQ_BIT)
            || post[5] & ABRT != 0
            || post[8] != 0
            || post[9] != 1
            || (count == 0 && post[6..8] != [STOP, STOP])
            || (count != 0 && (post[5] & STOP != 0 || post[6..8] != [0, 0]))
        {
            return FAIL_ISR;
        }
        delivery_decision(
            count,
            IPSR.load(Ordering::Relaxed),
            RAW.load(Ordering::Relaxed),
            MASKED.load(Ordering::Relaxed),
            ABORT.load(Ordering::Relaxed),
            words[121],
        )
    }

    pub fn run(i2c: I2c1, sda: Pin<2>, scl: Pin<3>) -> u32 {
        let mut words = new_record();
        let before = capture(0);
        words[5] = before[17];
        store_stage(&mut words, before);
        // Exact measured baseline: never normalize an unexpected prestate.
        if !baseline(&before, 0x48ff, 0) {
            store_stage(&mut words, capture(5));
            words[125] = FAIL_PRESTATE;
            return publish(words);
        }
        let saved = match unsafe { rp1_rt::prepare_i2c1_irq8_one_entry() } {
            Some(saved) => saved,
            None => {
                store_stage(&mut words, capture(5));
                words[125] = FAIL_PRESTATE;
                return publish(words);
            }
        };
        words[127] |= 1;
        let setup = i2c.into_host_100khz(sda, scl);
        let after = capture(1);
        store_stage(&mut words, after);
        let primary = match setup {
            Ok(mut host) if baseline(&after, 0, saved.primask) => {
                words[127] |= 1 << 1;
                experiment(&mut host, &mut words)
            }
            _ => FAIL_SETUP,
        };
        unsafe { rp1_rt::mask_i2c1_irq8_one_entry() };
        let count_at_cleanup = COUNT.load(Ordering::Acquire);
        let primary = if (primary == SAFE_NEGATIVE && count_at_cleanup != 0)
            || (primary == PASS && count_at_cleanup != 1)
        {
            FAIL_ISR
        } else {
            primary
        };
        words[125] = primary;
        words[126] = count_at_cleanup;
        let disable_ok = rp1_hal::i2c::i2c1_disable_stop_det_irq().is_ok();
        unsafe { rp1_rt::restore_i2c1_irq8_one_entry(saved) };
        let wrapper_expected = u32::from(words[127] & (1 << 2) != 0);
        let (stable, elapsed) = quiet(|| {
            let observed = capture(5);
            COUNT.load(Ordering::Acquire) == count_at_cleanup
                && identity(&observed, wrapper_expected)
                && route(&observed, saved.primask, 0, 0)
                && observed[5] & (STOP | ABRT) == 0
                && observed[6..10] == [0; 4]
        });
        words[122] = elapsed;
        let final_stage = capture(5);
        store_stage(&mut words, final_stage);
        let status = u32::from(disable_ok)
            | (u32::from(final_stage[9] == 0) << 1)
            | (u32::from(final_stage[7] == 0) << 2)
            | (u32::from(final_stage[5] & (STOP | ABRT) == 0) << 3)
            | (u32::from(final_stage[6] == 0) << 4)
            | (u32::from(final_stage[8] == 0) << 5)
            | (u32::from(route(&final_stage, saved.primask, 0, 0)) << 6)
            | (u32::from(stable && elapsed >= QUIET_US) << 7)
            | (u32::from(identity(&final_stage, wrapper_expected)) << 8);
        words[123] = status;
        words[2] = if status == 0x1ff {
            primary
        } else {
            FAIL_CLEANUP
        };
        words[6] = COUNT.load(Ordering::Acquire);
        words[7] = IPSR.load(Ordering::Relaxed);
        words[116] = RAW.load(Ordering::Relaxed);
        words[117] = MASKED.load(Ordering::Relaxed);
        words[118] = ABORT.load(Ordering::Relaxed);
        publish(words)
    }

    #[inline(never)]
    fn publish(words: [u32; WORDS]) -> u32 {
        const _: () = assert!(WORDS * 4 == 512 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
        const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
        let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
        unsafe {
            for (index, word) in words.iter().enumerate().skip(1) {
                core::ptr::write_volatile(out.add(index), *word);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(out, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        words[2]
    }
}

#[cfg(target_arch = "arm")]
pub use hardware::run;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_route_and_decisions_reject_missing_source_bad_prestates_and_malformed_stages() {
        assert_eq!(MAGIC.to_le_bytes(), *b"I1S1");
        assert_eq!(WORDS * 4, 512);
        assert_eq!(
            [READY_MARKER, SUCCESS_MARKER, FAILURE_MARKER],
            [469, 471, 591]
        );
        assert_eq!(
            WITNESS,
            0x8000_0000 | ((u32::from(PACKET[19]) | STOP) << 16) | (20 << 8) | 0x2d
        );
        let stage = [
            0,
            0,
            0x001f_1fea,
            0x3230_322a,
            0x4457_0140,
            0,
            0,
            0x48ff,
            0,
            0,
            0x2000_0000,
            0,
            0,
            0,
            1 << 21,
            0,
            0,
            0,
        ];
        assert!(baseline(&stage, 0x48ff, 0));
        for index in 1..18 {
            let mut bad = stage;
            bad[index] ^= 1;
            assert!(!baseline(&bad, 0x48ff, 0), "field {index}");
        }
        let mut stopped = stage;
        stopped[1] = 1;
        stopped[7] = STOP;
        stopped[9] = 1;
        assert!(armed(&stopped, 0));
        assert!(!source(&stopped, 0));
        stopped[5] = STOP;
        stopped[6] = STOP;
        stopped[13] = IRQ_BIT;
        assert!(source(&stopped, 0));
        stopped[13] |= 1 << 9;
        assert!(!source(&stopped, 0));
        assert_eq!(delivery_decision(0, 0, 0, 0, 0, DEADLINE_US), SAFE_NEGATIVE);
        assert_eq!(delivery_decision(0, 0, 0, 0, 0, 0), FAIL_ISR);
        assert_eq!(delivery_decision(1, 24, STOP, STOP, 0, 1), PASS);
        for (count, ipsr, raw, masked, abort) in [
            (2, 24, STOP, STOP, 0),
            (1, 25, STOP, STOP, 0),
            (1, 24, STOP | ABRT, STOP, 0),
            (1, 24, 0, STOP, 0),
            (1, 24, STOP, 0, 0),
            (1, 24, STOP, STOP, 1),
        ] {
            assert_eq!(
                delivery_decision(count, ipsr, raw, masked, abort, 1),
                FAIL_ISR
            );
        }
        let mut words = new_record();
        assert_eq!(words[4], 0);
        store_stage(&mut words, stage);
        assert_eq!(words[4], 1);
        assert_eq!(words[8..26], stage);
        let mut bad = stage;
        bad[0] = 6;
        assert!(std::panic::catch_unwind(|| store_stage(&mut new_record(), bad)).is_err());
    }
}

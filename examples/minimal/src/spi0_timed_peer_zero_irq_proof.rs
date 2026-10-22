#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_miso_input_observation::{self as observation, Snapshot};

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0P1");
pub const WORDS: usize = 32;
const UNAVAILABLE: u32 = u32::MAX;
const INPUT_BITS: u32 = (1 << 17) | (1 << 18) | (1 << 19);
const FAIL_SETUP: u32 = 0x3a0;
const FAIL_APPLY: u32 = 0x3a1;
const FAIL_HIGH: u32 = 0x3a2;
const FAIL_LOW_TIMEOUT: u32 = 0x3a3;
const FAIL_LOW_GUARD: u32 = 0x3a4;
const FAIL_RELEASE_TIMEOUT: u32 = 0x3a5;
const FAIL_RELEASE_GUARD: u32 = 0x3a6;
const FAIL_IRQ: u32 = 0x3a7;
const FAIL_RX: u32 = 0x3a8;
const FAIL_INCOMPLETE: u32 = 0x3a9;

fn guard_ok(expected_pad: u32, regs: [u32; 4]) -> bool {
    expected_pad & 0xff == 0xfb
        && regs[0] == 0x80
        && regs[1] == expected_pad
        && regs[2] & (1 << 13) == 0
        && regs[3] & (1 << 9) == 0
}

fn level_ok(regs: [u32; 4], high: bool) -> bool {
    regs[2] & INPUT_BITS == if high { INPUT_BITS } else { 0 }
}

fn irq_ok(irq: [u32; 16]) -> bool {
    let before = irq[11];
    matches!(before, 0x4010 | 0x4018)
        && irq
            == [
                crate::spi0_irq19_one_entry_rx_proof::MAGIC,
                1,
                0,
                1,
                1,
                35,
                0x11,
                0x10,
                1,
                irq[9],
                1,
                before,
                before | 2,
                0x4014,
                before,
                0x307ff,
            ]
}

fn final_decision(expected_pad: u32, snapshot: Snapshot, irq: [u32; 16]) -> u32 {
    let gpio = observation::pack(0, snapshot);
    if gpio[3] != 7 {
        return FAIL_INCOMPLETE;
    }
    for stage in 0..3 {
        let i = 4 + stage * 4;
        let regs = [gpio[i], gpio[i + 1], gpio[i + 2], gpio[i + 3]];
        if !guard_ok(expected_pad, regs) || !level_ok(regs, stage != 1) {
            return [FAIL_HIGH, FAIL_LOW_GUARD, FAIL_RELEASE_GUARD][stage];
        }
    }
    if !irq_ok(irq) {
        FAIL_IRQ
    } else if irq[9] != 0x0001_0000 {
        FAIL_RX
    } else {
        1
    }
}

fn pack(decision: u32, snapshot: Snapshot, irq: [u32; 16]) -> [u32; WORDS] {
    let mut words = [UNAVAILABLE; WORDS];
    words[..16].copy_from_slice(&observation::pack(decision, snapshot));
    words[0] = MAGIC;
    words[16..].copy_from_slice(&irq);
    words
}

#[cfg(target_arch = "arm")]
fn read_gpio() -> [u32; 4] {
    // Reuse the existing exact CTRL/PAD/STATUS/RIOOE read set, with no writes.
    let mut snapshot = Snapshot::new();
    observation::record(&mut snapshot, 0);
    let words = observation::pack(0, snapshot);
    [words[4], words[5], words[6], words[7]]
}

#[cfg(target_arch = "arm")]
fn wait_level(expected_pad: u32, snapshot: &mut Snapshot, stage: usize) -> u32 {
    const TIMER_LOW: *const u32 = 0x400a_c028 as *const u32;
    let (high, wait_us, cap, timeout, guard_failure) = if stage == 1 {
        (
            false,
            30_000_000,
            300_000_000,
            FAIL_LOW_TIMEOUT,
            FAIL_LOW_GUARD,
        )
    } else {
        (
            true,
            3_000_000,
            30_000_000,
            FAIL_RELEASE_TIMEOUT,
            FAIL_RELEASE_GUARD,
        )
    };
    let start = unsafe { core::ptr::read_volatile(TIMER_LOW) };
    // First bound wins. The iteration cap is finite even with a stalled timer;
    // it is not a calibrated wall-clock deadline or hardware-fault watchdog.
    for _ in 0..cap {
        if unsafe { core::ptr::read_volatile(TIMER_LOW) }.wrapping_sub(start) >= wait_us {
            return timeout;
        }
        let regs = read_gpio();
        if !guard_ok(expected_pad, regs) {
            snapshot.record(stage, regs);
            return guard_failure;
        }
        if level_ok(regs, high) {
            snapshot.record(stage, regs);
            return 1;
        }
        core::hint::spin_loop();
    }
    timeout
}

#[cfg(target_arch = "arm")]
fn publish(decision: u32, snapshot: Snapshot, irq: [u32; 16]) -> u32 {
    const _: () = assert!(WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    let words = pack(decision, snapshot, irq);
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    decision
}

#[cfg(target_arch = "arm")]
pub fn publish_setup_error() {
    publish(FAIL_SETUP, Snapshot::new(), [UNAVAILABLE; 16]);
}

#[cfg(target_arch = "arm")]
pub fn run(host: &mut rp1_hal::spi::Spi0Host, ready: impl FnOnce()) -> u32 {
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        core::ptr::write_volatile(out, 0); // Invalidate any previous boot before READY.
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let mut snapshot = Snapshot::new();
    let mut irq = [UNAVAILABLE; 16];
    let expected_pad = match observation::apply_guarded_bias() {
        Ok(pad) => pad,
        Err(_) => return publish(FAIL_APPLY, snapshot, irq),
    };
    crate::delay_readback_units(8); // Same bounded settling as S0B1.
    let high = read_gpio();
    snapshot.record(0, high);
    if !guard_ok(expected_pad, high) || !level_ok(high, true) {
        return publish(FAIL_HIGH, snapshot, irq);
    }
    ready();
    let low = wait_level(expected_pad, &mut snapshot, 1);
    if low != 1 {
        return publish(low, snapshot, irq);
    }
    let irq_decision = crate::spi0_irq19_one_entry_rx_proof::run(host);
    // Copy all 16 final S0I2 words immediately, before observing release or
    // overwriting the shared mailbox. Preserve every IRQ field unchanged.
    for (i, word) in irq.iter_mut().enumerate() {
        *word = unsafe { core::ptr::read_volatile(out.add(i)) };
    }
    let release = wait_level(expected_pad, &mut snapshot, 2);
    let decision = if release != 1 {
        release
    } else if irq[1] != irq_decision {
        FAIL_IRQ
    } else {
        final_decision(expected_pad, snapshot, irq)
    };
    publish(decision, snapshot, irq)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_and_gates_require_guarded_high_low_high_and_exact_irq_zero() {
        let high = [0x80, 0xabcd_00fb, 0x0440_0000 | INPUT_BITS, 0x0040_0000];
        let low = [high[0], high[1], high[2] & !INPUT_BITS, high[3]];
        let irq = [
            0x3249_3053,
            1,
            0,
            1,
            1,
            35,
            0x11,
            0x10,
            1,
            0x0001_0000,
            1,
            0x4010,
            0x4012,
            0x4014,
            0x4010,
            0x307ff,
        ];
        let mut snapshot = Snapshot::new();
        let missing = pack(FAIL_SETUP, snapshot, [UNAVAILABLE; 16]);
        assert_eq!(&missing[..4], &[0x3150_3053, 1, FAIL_SETUP, 0]);
        assert_eq!(&missing[4..], &[UNAVAILABLE; 28]);
        for stage in 0..3 {
            assert_eq!(final_decision(high[1], snapshot, irq), FAIL_INCOMPLETE);
            snapshot.record(stage, if stage == 1 { low } else { high });
        }
        assert_eq!(final_decision(high[1], snapshot, irq), 1);
        let words = pack(1, snapshot, irq);
        assert_eq!(&words[..4], &[MAGIC, 1, 1, 7]);
        assert_eq!(&words[4..8], &high);
        assert_eq!(&words[8..12], &low);
        assert_eq!(&words[12..16], &high);
        assert_eq!(&words[16..], &irq);
        for i in 0..16 {
            let mut bad_irq = irq;
            bad_irq[i] ^= 1;
            assert_eq!(
                final_decision(high[1], snapshot, bad_irq),
                if i == 9 { FAIL_RX } else { FAIL_IRQ }
            );
        }
        for stage in 0..3 {
            for (reg, bit) in [
                (0, 0),
                (1, 0),
                (1, 2),
                (1, 3),
                (1, 6),
                (1, 7),
                (1, 31),
                (2, 13),
                (2, 17),
                (2, 18),
                (2, 19),
                (3, 9),
            ] {
                let mut bad_snapshot = snapshot;
                let mut regs = if stage == 1 { low } else { high };
                regs[reg] ^= 1 << bit;
                bad_snapshot.record(stage, regs);
                assert_ne!(final_decision(high[1], bad_snapshot, irq), 1);
            }
        }
        for expected in [0xfb ^ 4, 0xfb ^ 8, 0xfb ^ 64, 0xfb ^ 128] {
            assert!(!guard_ok(expected, [0x80, expected, INPUT_BITS, 0]));
        }
        assert_eq!(
            final_decision(high[1], snapshot, [UNAVAILABLE; 16]),
            FAIL_IRQ
        );
    }
}

#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_irq19_one_entry_rx_proof as irq;
use crate::spi0_miso_input_observation as observation;

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0R2");
pub const WORDS: usize = 52;
const UNAVAILABLE: u32 = u32::MAX;
const INPUT_BITS: u32 = (1 << 17) | (1 << 18) | (1 << 19);
const FAIL_SETUP: u32 = 0x3b0;
const FAIL_APPLY: u32 = 0x3b1;
const FAIL_HIGH: u32 = 0x3b2;
const FAIL_LOW_TIMEOUT: u32 = 0x3b3;
const FAIL_LOW_GUARD: u32 = 0x3b4;
const FAIL_RELEASE_TIMEOUT: u32 = 0x3b5;
const FAIL_RELEASE_GUARD: u32 = 0x3b6;
const FAIL_SETTLED_HIGH: u32 = 0x3b7;
const FAIL_IRQ0: u32 = 0x3b8;
const FAIL_RX0: u32 = 0x3b9;
const FAIL_IRQ1: u32 = 0x3ba;
const FAIL_RX1: u32 = 0x3bb;
const FAIL_POST_HIGH: u32 = 0x3bc;
const FAIL_INCOMPLETE: u32 = 0x3bd;

#[derive(Clone, Copy)]
struct Record {
    words: [u32; WORDS],
}

impl Record {
    fn new() -> Self {
        let mut words = [UNAVAILABLE; WORDS];
        words[..4].copy_from_slice(&[MAGIC, 1, FAIL_SETUP, 0]);
        Self { words }
    }

    fn gpio(&self, stage: usize) -> [u32; 4] {
        self.words[4 + stage * 4..8 + stage * 4].try_into().unwrap()
    }

    fn record_gpio(&mut self, stage: usize, regs: [u32; 4]) {
        self.words[4 + stage * 4..8 + stage * 4].copy_from_slice(&regs);
        self.words[3] |= 1 << stage;
    }

    fn irq(&self, round: usize) -> [u32; 16] {
        self.words[20 + round * 16..36 + round * 16]
            .try_into()
            .unwrap()
    }

    #[cfg(target_arch = "arm")]
    fn copy_irq(&mut self, round: usize) {
        let out = rp1_hal::debug::MAILBOX_ADDR as *const u32;
        // Copy all raw final S0I2 words before another publication or GPIO read.
        for (i, word) in self.words[20 + round * 16..36 + round * 16]
            .iter_mut()
            .enumerate()
        {
            *word = unsafe { core::ptr::read_volatile(out.add(i)) };
        }
        self.words[3] |= 1 << (4 + round);
    }
}

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

fn irq_decision(words: [u32; 16], round: usize) -> u32 {
    let before = words[11];
    if !matches!(before, 0x4010 | 0x4018)
        || words
            != [
                irq::MAGIC,
                1,
                round as u32,
                1,
                1,
                35,
                0x11,
                0x10,
                1,
                words[9],
                1,
                before,
                before | 2,
                0x4014,
                before,
                0x307ff,
            ]
    {
        [FAIL_IRQ0, FAIL_IRQ1][round]
    } else if words[9] != [0x0001_0000, 0x0001_00ff][round] {
        [FAIL_RX0, FAIL_RX1][round]
    } else {
        1
    }
}

fn second_gate(expected_pad: u32, record: &Record) -> u32 {
    if record.words[3] & 0x17 != 0x17 {
        return FAIL_INCOMPLETE;
    }
    for stage in 0..3 {
        let regs = record.gpio(stage);
        if !guard_ok(expected_pad, regs) || !level_ok(regs, stage != 1) {
            return [FAIL_HIGH, FAIL_LOW_GUARD, FAIL_SETTLED_HIGH][stage];
        }
    }
    irq_decision(record.irq(0), 0)
}

fn final_decision(expected_pad: u32, record: &Record) -> u32 {
    if record.words[3] != 0x3f {
        return FAIL_INCOMPLETE;
    }
    let first = second_gate(expected_pad, record);
    if first != 1 {
        return first;
    }
    let post = record.gpio(3);
    if !guard_ok(expected_pad, post) || !level_ok(post, true) {
        return FAIL_POST_HIGH;
    }
    if record.irq(1)[11] != record.irq(0)[14] {
        return FAIL_IRQ1;
    }
    irq_decision(record.irq(1), 1)
}

#[cfg(target_arch = "arm")]
fn read_gpio() -> [u32; 4] {
    // Existing fixed CTRL/PAD/STATUS/RIOOE reads; no GPIO writes.
    let mut snapshot = observation::Snapshot::new();
    observation::record(&mut snapshot, 0);
    let words = observation::pack(0, snapshot);
    [words[4], words[5], words[6], words[7]]
}

#[cfg(target_arch = "arm")]
fn wait_level(expected_pad: u32, record: &mut Record, stage: usize) -> u32 {
    const TIMER_LOW: *const u32 = 0x400a_c028 as *const u32;
    let (high, ticks, cap, timeout, guard_failure) = if stage == 1 {
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
    // First bound wins. The finite iteration cap also covers a stalled timer;
    // it is not a calibrated wall-clock bound or an MMIO-fault watchdog.
    for _ in 0..cap {
        if unsafe { core::ptr::read_volatile(TIMER_LOW) }.wrapping_sub(start) >= ticks {
            return timeout;
        }
        let regs = read_gpio();
        if !guard_ok(expected_pad, regs) {
            record.record_gpio(stage, regs);
            return guard_failure;
        }
        if level_ok(regs, high) {
            record.record_gpio(stage, regs);
            return 1;
        }
        core::hint::spin_loop();
    }
    timeout
}

#[cfg(target_arch = "arm")]
fn publish(decision: u32, mut record: Record) -> u32 {
    const _: () = assert!(WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    const _: () = assert!(WORDS * 4 == 208 && 0xfc00 + WORDS * 4 <= 0xff00);
    record.words[2] = decision;
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in record.words.iter().enumerate().skip(1) {
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
    publish(FAIL_SETUP, Record::new());
}

#[cfg(target_arch = "arm")]
pub fn run(host: &mut rp1_hal::spi::Spi0Host, ready: impl FnOnce()) -> u32 {
    unsafe {
        core::ptr::write_volatile(rp1_hal::debug::MAILBOX_ADDR as *mut u32, 0);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let mut record = Record::new();
    let expected_pad = match observation::apply_guarded_bias() {
        Ok(pad) => pad,
        Err(_) => return publish(FAIL_APPLY, record),
    };
    crate::delay_readback_units(8);
    let high = read_gpio();
    record.record_gpio(0, high);
    if !guard_ok(expected_pad, high) || !level_ok(high, true) {
        return publish(FAIL_HIGH, record);
    }
    ready();
    let low = wait_level(expected_pad, &mut record, 1);
    let first = if low == 1 {
        let returned = irq::run(host);
        record.copy_irq(0);
        if returned != record.irq(0)[1] {
            FAIL_IRQ0
        } else {
            irq_decision(record.irq(0), 0)
        }
    } else {
        low
    };

    // After READY, observe bounded release even when LOW/IRQ0 failed. This
    // evidence never drives, restores, drops or reconstructs the GPIO owner.
    let release = wait_level(expected_pad, &mut record, 2);
    if release != 1 {
        return publish(release, record);
    }
    crate::delay_readback_units(8);
    let settled = read_gpio();
    record.record_gpio(2, settled); // The actual settled pre-round1 sample.
    if !guard_ok(expected_pad, settled) || !level_ok(settled, true) {
        return publish(FAIL_SETTLED_HIGH, record);
    }
    if first != 1 {
        return publish(first, record);
    }
    let gate = second_gate(expected_pad, &record);
    if gate != 1 {
        return publish(gate, record);
    }

    let returned = irq::run_rearmed(host);
    record.copy_irq(1);
    record.record_gpio(3, read_gpio());
    let decision = if returned != record.irq(1)[1] {
        FAIL_IRQ1
    } else {
        final_decision(expected_pad, &record)
    };
    publish(decision, record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(before: u32) -> Record {
        let high = [0x80, 0xabcd_00fb, 0x0440_0000 | INPUT_BITS, 0x0040_0000];
        let mut record = Record::new();
        for stage in 0..4 {
            let mut regs = high;
            if stage == 1 {
                regs[2] &= !INPUT_BITS;
            }
            record.record_gpio(stage, regs);
        }
        for round in 0..2 {
            record.words[20 + round * 16..36 + round * 16].copy_from_slice(&[
                irq::MAGIC,
                1,
                round as u32,
                1,
                1,
                35,
                0x11,
                0x10,
                1,
                [0x0001_0000, 0x0001_00ff][round],
                1,
                before,
                before | 2,
                0x4014,
                before,
                0x307ff,
            ]);
            record.words[3] |= 1 << (4 + round);
        }
        record
    }

    #[test]
    fn schema_missing_words_and_exact_low_high_rounds() {
        let missing = Record::new();
        assert_eq!(WORDS, 52);
        assert_eq!(&missing.words[..4], &[0x3252_3053, 1, FAIL_SETUP, 0]);
        assert_eq!(&missing.words[4..], &[UNAVAILABLE; 48]);
        for before in [0x4010, 0x4018] {
            let record = complete(before);
            assert_eq!(record.words[3], 0x3f);
            assert_eq!(record.words[29], 0x0001_0000);
            assert_eq!(record.words[45], 0x0001_00ff);
            assert_eq!(final_decision(record.words[5], &record), 1);
        }
    }

    #[test]
    fn every_missing_stage_blocks_final_and_missing_prerequisite_blocks_second() {
        for stage in 0..6 {
            let mut record = complete(0x4010);
            record.words[3] &= !(1 << stage);
            assert_eq!(final_decision(record.words[5], &record), FAIL_INCOMPLETE);
            if stage != 3 && stage != 5 {
                assert_eq!(second_gate(record.words[5], &record), FAIL_INCOMPLETE);
            }
        }
        let mut record = complete(0x4010);
        record.words[3] = 0x17;
        assert_eq!(second_gate(record.words[5], &record), 1);
        assert_eq!(final_decision(record.words[5], &record), FAIL_INCOMPLETE);
        record.words[3] = 0x7f;
        assert_eq!(final_decision(record.words[5], &record), FAIL_INCOMPLETE);
    }

    #[test]
    fn every_irq_word_and_wrong_received_byte_is_rejected() {
        for round in 0..2 {
            for word in 0..16 {
                let mut record = complete(0x4010);
                record.words[20 + 16 * round + word] ^= 1;
                let expected = if word == 9 {
                    [FAIL_RX0, FAIL_RX1][round]
                } else {
                    [FAIL_IRQ0, FAIL_IRQ1][round]
                };
                assert_eq!(final_decision(record.words[5], &record), expected);
                if round == 0 {
                    assert_eq!(second_gate(record.words[5], &record), expected);
                }
            }
        }
        let mut changed_context = complete(0x4010);
        changed_context.words[36..52].copy_from_slice(&complete(0x4018).words[36..52]);
        assert_eq!(
            final_decision(changed_context.words[5], &changed_context),
            FAIL_IRQ1
        );
    }

    #[test]
    fn every_guard_and_input_level_is_required() {
        for stage in 0..4 {
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
                let mut record = complete(0x4010);
                record.words[4 + stage * 4 + reg] ^= 1 << bit;
                assert_ne!(final_decision(0xabcd_00fb, &record), 1);
                if stage < 3 {
                    assert_ne!(second_gate(0xabcd_00fb, &record), 1);
                }
            }
        }
        for expected in [0xfb ^ 4, 0xfb ^ 8, 0xfb ^ 64, 0xfb ^ 128] {
            assert!(!guard_ok(expected, [0x80, expected, INPUT_BITS, 0]));
        }
    }
}

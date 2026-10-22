#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_irq19_one_entry_rx_proof as irq;
#[cfg(target_arch = "arm")]
use crate::spi0_miso_input_observation as observation;

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0F2");
pub const WORDS: usize = 192;
const UNAVAILABLE: u32 = u32::MAX;
const INPUT_BITS: u32 = (1 << 17) | (1 << 18) | (1 << 19);
const SENTINEL: u8 = 0xc3;
const CANARIES: u32 = 0x5aa5_a55a;
const FAIL_SETUP: u32 = 0x3c0;
const FAIL_API: u32 = 0x3c1;
const FAIL_APPLY: u32 = 0x3c2;
const FAIL_GUARD: u32 = 0x3c3;
const FAIL_LOW_TIMEOUT: u32 = 0x3c4;
const FAIL_RELEASE_TIMEOUT: u32 = 0x3c5;
const FAIL_IRQ: u32 = 0x3c6;
const FAIL_RX: u32 = 0x3c7;
const FAIL_INCOMPLETE: u32 = 0x3c8;

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
        self.words[24 + round * 16..40 + round * 16]
            .try_into()
            .unwrap()
    }

    fn record_rx(&mut self, round: usize, bytes: &[u8; 256], canaries: u32) {
        self.words[62 + round] = canaries;
        for (dst, bytes) in self.words[64 + round * 64..128 + round * 64]
            .iter_mut()
            .zip(bytes.chunks_exact(4))
        {
            *dst = u32::from_le_bytes(bytes.try_into().unwrap());
        }
    }

    #[cfg(target_arch = "arm")]
    fn copy_irq(&mut self, round: usize) {
        let out = rp1_hal::debug::MAILBOX_ADDR as *const u32;
        for (i, word) in self.words[24 + round * 16..40 + round * 16]
            .iter_mut()
            .enumerate()
        {
            *word = unsafe { core::ptr::read_volatile(out.add(i)) };
        }
        self.words[3] |= 1 << (5 + round);
    }
}

fn guard_ok(expected_pad: u32, regs: [u32; 4], high: bool) -> bool {
    expected_pad & 0xff == 0xfb
        && regs[0] == 0x80
        && regs[1] == expected_pad
        && regs[2] & (1 << 13) == 0
        && regs[3] & (1 << 9) == 0
        && regs[2] & INPUT_BITS == if high { INPUT_BITS } else { 0 }
}

fn api_ok(record: &Record) -> bool {
    (2..=255).contains(&record.words[56]) && record.words[57..60] == [1, 1, 0xf]
}

fn round_decision(record: &Record, round: usize) -> u32 {
    if !api_ok(record) {
        return FAIL_API;
    }
    let n = record.words[56];
    let words = record.irq(round);
    let before = words[11];
    let byte = if round == 0 { 0 } else { 0xff };
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
                n,
                (n << 16) | byte,
                1,
                before,
                before | 2,
                0x4014,
                before,
                0x307ff,
            ]
    {
        return FAIL_IRQ;
    }
    if record.words[62 + round] != CANARIES || !(4_000..100_000).contains(&record.words[60 + round])
    {
        return FAIL_RX;
    }
    for (i, actual) in record.words[64 + round * 64..128 + round * 64]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .enumerate()
    {
        let expected = if i < n as usize { byte as u8 } else { SENTINEL };
        if actual != expected {
            return FAIL_RX;
        }
    }
    1
}

fn second_gate(expected_pad: u32, record: &Record) -> u32 {
    if record.words[3] & 0x2f != 0x2f {
        return FAIL_INCOMPLETE;
    }
    for stage in 0..4 {
        if !guard_ok(expected_pad, record.gpio(stage), matches!(stage, 0 | 3)) {
            return FAIL_GUARD;
        }
    }
    round_decision(record, 0)
}

fn final_decision(expected_pad: u32, record: &Record) -> u32 {
    if record.words[3] != 0x7f {
        return FAIL_INCOMPLETE;
    }
    let first = second_gate(expected_pad, record);
    if first != 1 {
        return first;
    }
    if !guard_ok(expected_pad, record.gpio(4), true) {
        return FAIL_GUARD;
    }
    if record.irq(1)[11] != record.irq(0)[14] {
        return FAIL_IRQ;
    }
    round_decision(record, 1)
}

#[cfg(target_arch = "arm")]
fn read_gpio() -> [u32; 4] {
    let mut snapshot = observation::Snapshot::new();
    observation::record(&mut snapshot, 0);
    let words = observation::pack(0, snapshot);
    [words[4], words[5], words[6], words[7]]
}

#[cfg(target_arch = "arm")]
fn timer() -> u32 {
    unsafe { core::ptr::read_volatile(0x400a_c028 as *const u32) }
}

#[cfg(target_arch = "arm")]
fn wait_level(expected_pad: u32, record: &mut Record, high: bool) -> u32 {
    let (ticks, cap, failure, stage) = if high {
        (3_000_000, 30_000_000, FAIL_RELEASE_TIMEOUT, 3)
    } else {
        (30_000_000, 300_000_000, FAIL_LOW_TIMEOUT, 1)
    };
    let start = timer();
    // Existing timer and finite iteration cap, not an MMIO-fault watchdog.
    for _ in 0..cap {
        if timer().wrapping_sub(start) >= ticks {
            return failure;
        }
        let regs = read_gpio();
        let observed_high = regs[2] & INPUT_BITS == INPUT_BITS;
        // Check electrical guard regardless of the awaited input level.
        let guarded = [regs[0], regs[1], regs[2] & !INPUT_BITS, regs[3]];
        if !guard_ok(expected_pad, guarded, false) {
            record.record_gpio(stage, regs);
            return FAIL_GUARD;
        }
        if observed_high == high && guard_ok(expected_pad, regs, high) {
            record.record_gpio(stage, regs);
            return 1;
        }
        core::hint::spin_loop();
    }
    failure
}

#[cfg(target_arch = "arm")]
fn reject_length(host: &mut rp1_hal::spi::Spi0Host, len: usize) -> Option<u16> {
    use rp1_hal::spi::{Spi0Error, Spi0RxError};
    let mut rx = [SENTINEL; 257];
    match host.prepare_irq_transfer(&[0xa5; 257][..len], &mut rx[..len]) {
        Err(Spi0RxError::Setup(Spi0Error::PayloadTooLong {
            len: actual,
            fifo_depth,
        })) if actual == len => Some(fifo_depth),
        Ok(mut unexpected) => {
            let _ = unexpected.abort();
            None
        }
        _ => None,
    }
}

#[cfg(target_arch = "arm")]
fn probe_capacity(host: &mut rp1_hal::spi::Spi0Host, record: &mut Record) -> bool {
    let before = irq::fifo_api_snapshot();
    let depth = reject_length(host, 257);
    let after = irq::fifo_api_snapshot();
    record.words[57] = u32::from(depth.is_some());
    record.words[59] = u32::from(before.0 == after.0) | (u32::from(before.1 == after.1) << 1);
    let Some(depth @ 2..=255) = depth else {
        return false;
    };
    record.words[56] = u32::from(depth);
    if record.words[59] != 3 {
        return false;
    }
    let before = irq::fifo_api_snapshot();
    let next_depth = reject_length(host, usize::from(depth) + 1);
    let after = irq::fifo_api_snapshot();
    record.words[58] = u32::from(next_depth == Some(depth));
    record.words[59] |=
        (u32::from(before.0 == after.0) << 2) | (u32::from(before.1 == after.1) << 3);
    api_ok(record)
}

#[repr(C)]
struct GuardedRx {
    before: u16,
    bytes: [u8; 256],
    after: u16,
}

#[cfg(target_arch = "arm")]
fn run_round(host: &mut rp1_hal::spi::Spi0Host, record: &mut Record, round: usize) -> u32 {
    let mut rx = GuardedRx {
        before: CANARIES as u16,
        bytes: [SENTINEL; 256],
        after: (CANARIES >> 16) as u16,
    };
    let n = record.words[56] as usize;
    let start = timer();
    let returned = if round == 0 {
        irq::run_fifo_low(host, &mut rx.bytes[..n])
    } else {
        irq::run_fifo_high(host, &mut rx.bytes[..n])
    };
    record.words[60 + round] = timer().wrapping_sub(start);
    record.copy_irq(round);
    let canaries = unsafe {
        u32::from(core::ptr::read_volatile(&rx.before))
            | (u32::from(core::ptr::read_volatile(&rx.after)) << 16)
    };
    record.record_rx(round, &rx.bytes, canaries);
    if returned != record.irq(round)[1] {
        FAIL_IRQ
    } else {
        round_decision(record, round)
    }
}

#[cfg(target_arch = "arm")]
#[inline(never)]
fn publish(decision: u32, mut record: Record) -> u32 {
    const _: () = assert!(WORDS * 4 == 768 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    const _: () = assert!(0xfc00 + WORDS * 4 == 0xff00);
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
    if !probe_capacity(host, &mut record) {
        return publish(FAIL_API, record);
    }
    let expected_pad = match observation::apply_guarded_bias() {
        Ok(pad) => pad,
        Err(_) => return publish(FAIL_APPLY, record),
    };
    crate::delay_readback_units(8);
    record.record_gpio(0, read_gpio());
    if !guard_ok(expected_pad, record.gpio(0), true) {
        return publish(FAIL_GUARD, record);
    }
    ready();
    let low = wait_level(expected_pad, &mut record, false);
    let first = if low == 1 {
        let first = run_round(host, &mut record, 0);
        record.record_gpio(2, read_gpio());
        if !guard_ok(expected_pad, record.gpio(2), false) {
            FAIL_GUARD
        } else {
            first
        }
    } else {
        low
    };
    // After READY always observe bounded release, even after first-round failure.
    // Retain the same host and GPIO ownership on every return path.
    let release = wait_level(expected_pad, &mut record, true);
    if release != 1 {
        return publish(release, record);
    }
    crate::delay_readback_units(8);
    record.record_gpio(3, read_gpio());
    if !guard_ok(expected_pad, record.gpio(3), true) {
        return publish(FAIL_GUARD, record);
    }
    if first != 1 {
        return publish(first, record);
    }
    let gate = second_gate(expected_pad, &record);
    if gate != 1 {
        return publish(gate, record);
    }
    let second = run_round(host, &mut record, 1);
    record.record_gpio(4, read_gpio());
    let decision = if second != 1 {
        second
    } else {
        final_decision(expected_pad, &record)
    };
    publish(decision, record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(n: u32, before: u32) -> Record {
        let mut record = Record::new();
        record.words[56..60].copy_from_slice(&[n, 1, 1, 0xf]);
        for stage in 0..5 {
            record.record_gpio(
                stage,
                [
                    0x80,
                    0xabcd_00fb,
                    if matches!(stage, 0 | 3 | 4) {
                        INPUT_BITS
                    } else {
                        0
                    },
                    0,
                ],
            );
        }
        for round in 0..2 {
            let byte = if round == 0 { 0 } else { 0xff };
            record.words[24 + round * 16..40 + round * 16].copy_from_slice(&[
                irq::MAGIC,
                1,
                round as u32,
                1,
                1,
                35,
                0x11,
                0x10,
                n,
                (n << 16) | byte,
                1,
                before,
                before | 2,
                0x4014,
                before,
                0x307ff,
            ]);
            record.words[3] |= 1 << (5 + round);
            record.words[60 + round] = 5_000;
            let mut rx = [SENTINEL; 256];
            rx[..n as usize].fill(byte as u8);
            record.record_rx(round, &rx, CANARIES);
        }
        record
    }

    #[test]
    fn fixed_abi_capacity_and_every_byte_are_checked() {
        assert_eq!(MAGIC, 0x3246_3053);
        assert_eq!(WORDS * 4, 768);
        assert_eq!(core::mem::size_of::<GuardedRx>(), 260);
        assert_eq!(core::mem::offset_of!(GuardedRx, bytes), 2);
        assert_eq!(core::mem::offset_of!(GuardedRx, after), 258);
        let missing = Record::new();
        assert_eq!(&missing.words[..4], &[MAGIC, 1, FAIL_SETUP, 0]);
        assert_eq!(&missing.words[4..], &[UNAVAILABLE; 188]);
        for n in [2, 64, 255] {
            for before in [0x4010, 0x4018] {
                let record = complete(n, before);
                assert_eq!(final_decision(0xabcd_00fb, &record), 1);
                for round in 0..2 {
                    for byte in 0..256 {
                        let mut bad = record;
                        bad.words[64 + round * 64 + byte / 4] ^= 1 << ((byte % 4) * 8);
                        assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_RX);
                    }
                }
            }
        }
    }

    #[test]
    fn every_required_field_blocks_false_pass_and_first_failure_blocks_second() {
        let record = complete(64, 0x4010);
        for word in 24..64 {
            let mut bad = record;
            bad.words[word] ^= if (60..62).contains(&word) { 0x20000 } else { 1 };
            assert_ne!(final_decision(0xabcd_00fb, &bad), 1, "word {word}");
        }
        for stage in 0..7 {
            let mut bad = record;
            bad.words[3] &= !(1 << stage);
            assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_INCOMPLETE);
            if !matches!(stage, 4 | 6) {
                assert_ne!(second_gate(0xabcd_00fb, &bad), 1);
            }
        }
        for stage in 0..5 {
            for (reg, bit) in [(0, 0), (1, 31), (2, 13), (2, 17), (2, 18), (2, 19), (3, 9)] {
                let mut bad = record;
                bad.words[4 + stage * 4 + reg] ^= 1 << bit;
                assert_ne!(final_decision(0xabcd_00fb, &bad), 1);
            }
        }
        for n in [0, 1, 256, UNAVAILABLE] {
            let mut bad = record;
            bad.words[56] = n;
            assert_eq!(second_gate(0xabcd_00fb, &bad), FAIL_API);
        }
        let mut bad = record;
        bad.words[40..56].copy_from_slice(&complete(64, 0x4018).words[40..56]);
        assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_IRQ);
    }
}

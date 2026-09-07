#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_irq19_one_entry_rx_proof as irq;
#[cfg(target_arch = "arm")]
use crate::spi0_miso_input_observation as observation;

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0V2");
pub const WORDS: usize = 112;
const N: usize = 4;
const FRAMES: [[u8; N]; 2] = [[0x69, 0x96, 0x3c, 1], [0x69, 0x96, 0x3c, 2]];
const UNAVAILABLE: u32 = u32::MAX;
const INPUT_BITS: u32 = (1 << 17) | (1 << 18) | (1 << 19);
const SENTINEL: u8 = 0xc3;
const CANARIES: u32 = 0x5aa5_a55a;
const READY_US: u32 = 30_000_000;
const RELEASE_US: u32 = 3_000_000;
const FAIL_SETUP: u32 = 0x3d0;
const FAIL_API: u32 = 0x3d1;
const FAIL_APPLY: u32 = 0x3d2;
const FAIL_GUARD: u32 = 0x3d3;
const FAIL_LOW_TIMEOUT: u32 = 0x3d4;
const FAIL_RELEASE_TIMEOUT: u32 = 0x3d5;
const FAIL_IRQ: u32 = 0x3d6;
const FAIL_RX: u32 = 0x3d7;
const FAIL_INCOMPLETE: u32 = 0x3d8;

#[derive(Clone, Copy)]
struct Record {
    words: [u32; WORDS],
}

impl Record {
    fn new() -> Self {
        let mut words = [UNAVAILABLE; WORDS];
        words[..4].copy_from_slice(&[MAGIC, 1, FAIL_SETUP, 0]);
        words[64] = N as u32;
        words[78..80].copy_from_slice(&[READY_US, RELEASE_US]);
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
        self.words[32 + round * 16..48 + round * 16]
            .try_into()
            .unwrap()
    }

    fn record_rx(&mut self, round: usize, bytes: &[u8; 64], canaries: u32) {
        self.words[70 + round] = canaries;
        for (dst, bytes) in self.words[80 + round * 16..96 + round * 16]
            .iter_mut()
            .zip(bytes.chunks_exact(4))
        {
            *dst = u32::from_le_bytes(bytes.try_into().unwrap());
        }
    }

    #[cfg(target_arch = "arm")]
    fn copy_irq(&mut self, round: usize) {
        let out = rp1_hal::debug::MAILBOX_ADDR as *const u32;
        for (i, word) in self.words[32 + round * 16..48 + round * 16]
            .iter_mut()
            .enumerate()
        {
            *word = unsafe { core::ptr::read_volatile(out.add(i)) };
        }
        self.words[74 + round] = self.irq(round)[9] >> 16;
        self.words[3] |= 1 << (7 + round);
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

fn round_decision(record: &Record, round: usize) -> u32 {
    if record.words[64] != N as u32
        || !(N as u32..=255).contains(&record.words[65])
        || record.words[78..80] != [READY_US, RELEASE_US]
    {
        return FAIL_API;
    }
    let words = record.irq(round);
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
                N as u32,
                ((N as u32) << 16) | 0x69,
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
    if record.words[66 + round] != 2_000
        || !(4_000..100_000).contains(&record.words[68 + round])
        || record.words[70 + round] != CANARIES
        || record.words[72 + round] != round as u32 + 1
        || record.words[74 + round] != N as u32
        || record.words[76 + round] != 0
    {
        return FAIL_RX;
    }
    for (i, actual) in record.words[80 + round * 16..96 + round * 16]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .enumerate()
    {
        if actual != if i < N { FRAMES[round][i] } else { SENTINEL } {
            return FAIL_RX;
        }
    }
    1
}

fn second_gate(expected_pad: u32, record: &Record) -> u32 {
    if record.words[3] & 0x8f != 0x8f {
        return FAIL_INCOMPLETE;
    }
    for stage in 0..4 {
        if !guard_ok(expected_pad, record.gpio(stage), stage != 1) {
            return FAIL_GUARD;
        }
    }
    round_decision(record, 0)
}

fn final_decision(expected_pad: u32, record: &Record) -> u32 {
    if record.words[3] != 0x1ff {
        return FAIL_INCOMPLETE;
    }
    let first = second_gate(expected_pad, record);
    if first != 1 {
        return first;
    }
    for stage in 4..7 {
        if !guard_ok(expected_pad, record.gpio(stage), stage != 4) {
            return FAIL_GUARD;
        }
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
fn wait_level(expected_pad: u32, record: &mut Record, stage: usize, high: bool) -> u32 {
    let (ticks, cap, failure) = if high {
        (RELEASE_US, 30_000_000, FAIL_RELEASE_TIMEOUT)
    } else {
        (READY_US, 300_000_000, FAIL_LOW_TIMEOUT)
    };
    let start = timer();
    // Existing timer and finite iteration cap, not an MMIO-fault watchdog.
    for _ in 0..cap {
        if timer().wrapping_sub(start) >= ticks {
            return failure;
        }
        let regs = read_gpio();
        let guarded = [regs[0], regs[1], regs[2] & !INPUT_BITS, regs[3]];
        if !guard_ok(expected_pad, guarded, false) {
            record.record_gpio(stage, regs);
            return FAIL_GUARD;
        }
        if guard_ok(expected_pad, regs, high) {
            record.record_gpio(stage, regs);
            return 1;
        }
        core::hint::spin_loop();
    }
    failure
}

#[cfg(target_arch = "arm")]
fn probe_capacity(host: &mut rp1_hal::spi::Spi0Host, record: &mut Record) -> bool {
    use rp1_hal::spi::{Spi0Error, Spi0RxError};
    let before = irq::fifo_api_snapshot();
    let mut rx = [SENTINEL; 257];
    let depth = match host.prepare_irq_transfer(&[0; 257], &mut rx) {
        Err(Spi0RxError::Setup(Spi0Error::PayloadTooLong {
            len: 257,
            fifo_depth,
        })) => Some(fifo_depth),
        Ok(mut unexpected) => {
            let _ = unexpected.abort();
            None
        }
        _ => None,
    };
    let after = irq::fifo_api_snapshot();
    if let Some(depth) = depth {
        record.words[65] = u32::from(depth);
    }
    before == after && (N as u32..=255).contains(&record.words[65])
}

#[repr(C)]
struct GuardedRx {
    before: u16,
    bytes: [u8; 64],
    after: u16,
}

#[cfg(target_arch = "arm")]
fn run_round(host: &mut rp1_hal::spi::Spi0Host, record: &mut Record, round: usize) -> u32 {
    let mut rx = GuardedRx {
        before: CANARIES as u16,
        bytes: [SENTINEL; 64],
        after: (CANARIES >> 16) as u16,
    };
    record.words[72 + round] = round as u32 + 1;
    record.words[76 + round] = 0; // TX configuration; not independent wire evidence.
    let start = timer();
    let returned = if round == 0 {
        irq::run_fifo_low(host, &mut rx.bytes[..N])
    } else {
        irq::run_fifo_high(host, &mut rx.bytes[..N])
    };
    record.words[68 + round] = timer().wrapping_sub(start);
    record.words[66 + round] = irq::fifo_api_snapshot().0[11]; // Existing BAUDR read.
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
    const _: () = assert!(WORDS * 4 == 448 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    const _: () = assert!(0xfc00 + WORDS * 4 == 0xfdc0);
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
pub fn run(host: &mut rp1_hal::spi::Spi0Host, mut ready: impl FnMut(usize)) -> u32 {
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
    for round in 0..2 {
        crate::delay_readback_units(8);
        record.record_gpio(round * 3, read_gpio());
        if !guard_ok(expected_pad, record.gpio(round * 3), true) {
            return publish(FAIL_GUARD, record);
        }
        if round == 1 {
            let gate = second_gate(expected_pad, &record);
            if gate != 1 {
                return publish(gate, record);
            }
        }
        ready(round);
        let low = wait_level(expected_pad, &mut record, round * 3 + 1, false);
        let result = if low == 1 {
            run_round(host, &mut record, round)
        } else {
            low
        };
        // Peer can retain its final LOW until finish deasserts CS. Always wait
        // for guarded release after READY, including mismatch/timeout paths.
        let release = wait_level(expected_pad, &mut record, round * 3 + 2, true);
        if release != 1 {
            return publish(release, record);
        }
        if result != 1 {
            return publish(result, record); // First failure forbids READY1/start1.
        }
    }
    crate::delay_readback_units(8);
    record.record_gpio(6, read_gpio());
    // Caller retains this same host and guarded pins on every terminal outcome.
    publish(final_decision(expected_pad, &record), record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(before: u32) -> Record {
        let mut record = Record::new();
        record.words[65] = 64;
        for stage in 0..7 {
            record.record_gpio(
                stage,
                [
                    0x80,
                    0xabcd_00fb,
                    if matches!(stage, 1 | 4) {
                        0
                    } else {
                        INPUT_BITS
                    },
                    0,
                ],
            );
        }
        for round in 0..2 {
            record.words[32 + round * 16..48 + round * 16].copy_from_slice(&[
                irq::MAGIC,
                1,
                round as u32,
                1,
                1,
                35,
                0x11,
                0x10,
                4,
                0x40069,
                1,
                before,
                before | 2,
                0x4014,
                before,
                0x307ff,
            ]);
            record.words[3] |= 1 << (7 + round);
            record.words[66 + round] = 2_000;
            record.words[68 + round] = 5_000;
            record.words[72 + round] = round as u32 + 1;
            record.words[74 + round] = 4;
            record.words[76 + round] = 0;
            let mut bytes = [SENTINEL; 64];
            bytes[..N].copy_from_slice(&FRAMES[round]);
            record.record_rx(round, &bytes, CANARIES);
        }
        record
    }

    #[test]
    fn fixed_abi_checks_every_byte_canary_field_and_first_round_gate() {
        assert_eq!(MAGIC, 0x3256_3053);
        assert_eq!(WORDS * 4, 448);
        assert_eq!(core::mem::size_of::<GuardedRx>(), 68);
        assert_eq!(core::mem::offset_of!(GuardedRx, bytes), 2);
        assert_eq!(core::mem::offset_of!(GuardedRx, after), 66);
        assert_eq!(final_decision(0xabcd_00fb, &Record::new()), FAIL_INCOMPLETE);
        for before in [0x4010, 0x4018] {
            let record = complete(before);
            assert_eq!(final_decision(0xabcd_00fb, &record), 1);
            for word in 32..80 {
                if word == 65 {
                    continue;
                } // D is an observed API bound, not fixed64.
                let mut bad = record;
                bad.words[word] ^= if matches!(word, 68 | 69) { 0x20000 } else { 1 };
                assert_ne!(final_decision(0xabcd_00fb, &bad), 1, "word {word}");
            }
            for round in 0..2 {
                for byte in 0..64 {
                    let mut bad = record;
                    bad.words[80 + round * 16 + byte / 4] ^= 1 << (byte % 4 * 8);
                    assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_RX);
                    if round == 0 {
                        assert_eq!(second_gate(0xabcd_00fb, &bad), FAIL_RX);
                    }
                }
            }
            for stage in 0..9 {
                let mut bad = record;
                bad.words[3] &= !(1 << stage);
                assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_INCOMPLETE);
                if stage < 4 || stage == 7 {
                    assert_ne!(second_gate(0xabcd_00fb, &bad), 1);
                }
            }
            for stage in 0..7 {
                for (reg, bit) in [(0, 0), (1, 31), (2, 13), (2, 17), (2, 18), (2, 19), (3, 9)] {
                    let mut bad = record;
                    bad.words[4 + stage * 4 + reg] ^= 1 << bit;
                    assert_ne!(final_decision(0xabcd_00fb, &bad), 1);
                    if stage < 4 {
                        assert_ne!(second_gate(0xabcd_00fb, &bad), 1);
                    }
                }
            }
            for depth in [0, 3, 256, UNAVAILABLE] {
                let mut bad = record;
                bad.words[65] = depth;
                assert_eq!(second_gate(0xabcd_00fb, &bad), FAIL_API);
            }
            let mut replay = record;
            replay.words[96] = record.words[80];
            assert_eq!(final_decision(0xabcd_00fb, &replay), FAIL_RX);
            let mut swapped = record;
            swapped.words.swap(80, 96);
            assert_eq!(second_gate(0xabcd_00fb, &swapped), FAIL_RX);
        }
        let mut changed_route = complete(0x4010);
        changed_route.words[48..64].copy_from_slice(&complete(0x4018).words[48..64]);
        assert_eq!(final_decision(0xabcd_00fb, &changed_route), FAIL_IRQ);
    }
}

#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_irq19_one_entry_rx_proof as irq;
#[cfg(target_arch = "arm")]
use crate::spi0_miso_input_observation as observation;

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0K2");
pub const WORDS: usize = 112;
const WAIT_US: u32 = 50_000;
const ITERATIONS: u32 = 12_500_000;
const CONFIG: [u32; 6] = [0x70000, 2_000, 0, 0, 0, 0];
const INPUT_BITS: u32 = 7 << 17;
const CANARIES: u32 = 0x5aa5_a55a;
const SENTINEL: u8 = 0xc3;
const FAIL_SETUP: u32 = 0x3e0;
const FAIL_GUARD: u32 = 0x3e1;
const FAIL_ROUTE: u32 = 0x3e2;
const FAIL_PREPARE: u32 = 0x3e3;
const FAIL_CONFIG: u32 = 0x3e4;
const FAIL_STATE: u32 = 0x3e5;
const FAIL_DEADLINE: u32 = 0x3e6;
const FAIL_ABORT: u32 = 0x3e7;
const FAIL_RX: u32 = 0x3e8;
const FAIL_IRQ: u32 = 0x3e9;
const FAIL_INCOMPLETE: u32 = 0x3ea;

#[derive(Clone, Copy)]
struct Record([u32; WORDS]);

impl Record {
    fn new() -> Self {
        let mut words = [u32::MAX; WORDS];
        words[..4].copy_from_slice(&[MAGIC, 1, FAIL_SETUP, 0]);
        words[92] = 0;
        words[110..112].copy_from_slice(&[WAIT_US, ITERATIONS]);
        Self(words)
    }

    fn gpio(&self, stage: usize) -> [u32; 4] {
        self.0[4 + stage * 4..8 + stage * 4].try_into().unwrap()
    }

    fn record_gpio(&mut self, stage: usize, regs: [u32; 4]) {
        self.0[4 + stage * 4..8 + stage * 4].copy_from_slice(&regs);
        self.0[3] |= 1 << stage;
    }

    fn spi(&self, stage: usize) -> [u32; 8] {
        self.0[20 + stage * 8..28 + stage * 8].try_into().unwrap()
    }

    fn record_spi(&mut self, stage: usize, regs: [u32; 8]) {
        self.0[20 + stage * 8..28 + stage * 8].copy_from_slice(&regs);
        self.0[3] |= 1 << (4 + stage);
    }
}

fn guard_ok(pad: u32, gpio: [u32; 4]) -> bool {
    pad & 0xff == 0xfb
        && gpio[0] == 0x80
        && gpio[1] == pad
        && gpio[2] & (1 << 13) == 0
        && gpio[2] & INPUT_BITS == INPUT_BITS
        && gpio[3] & (1 << 9) == 0
}

// SSIENR, SER, TXFLR, RXFLR, SR, IMR, ISR, RISR. No DR reads.
fn state_ok(stage: usize, s: [u32; 8]) -> bool {
    if stage == 4 {
        return s[..4] == [0, 0, 0, 0] && s[4] & 0x1f == 6 && s[5..7] == [0, 0] && s[7] & 0x1e == 0;
    }
    let (selected, tx, rx, status) = match stage {
        0 => (1, 0, 1, 0x0e),
        1 => (0, 0, 1, 0x0e),
        2 => (0, 1, 1, 0x0a),
        3 => (1, 0, 2, 0x0e),
        _ => return false,
    };
    s[..4] == [1, selected, tx, rx]
        && s[4] & 0x1f == status
        && s[5..7] == [0x1e, 0x10]
        && s[7] & !1 == 0x10
}

fn route_ok(r: &[u32]) -> bool {
    r.len() == 8 && r[..7] == [0x2000_0000, 0, 0, 0, 1 << 21, 0, 0] && r[7] <= 1
}

fn final_decision(pad: u32, record: &Record) -> u32 {
    let w = &record.0;
    if w[3] != 0xffff || w[110..112] != [WAIT_US, ITERATIONS] {
        return FAIL_INCOMPLETE;
    }
    if (0..4).any(|stage| !guard_ok(pad, record.gpio(stage))) {
        return FAIL_GUARD;
    }
    if w[76..82] != CONFIG {
        return FAIL_CONFIG;
    }
    if (0..5).any(|stage| !state_ok(stage, record.spi(stage))) || w[92] != 1 {
        return FAIL_STATE;
    }
    if w[86..91].iter().any(|ticks| *ticks >= WAIT_US) || !(4_000..100_000).contains(&w[91]) {
        return FAIL_DEADLINE;
    }
    if w[93] != 1 {
        return FAIL_ABORT;
    }
    if w[82..86] != [CANARIES, 0xc3c3_c3c3, CANARIES, 0xc3c3_c3ff] {
        return FAIL_RX;
    }
    if !route_ok(&w[94..102]) || w[94..102] != w[102..110] {
        return FAIL_ROUTE;
    }
    let before = 0x4010 | (w[101] << 3);
    if w[60..76]
        != [
            irq::MAGIC,
            1,
            0,
            1,
            1,
            35,
            0x11,
            0x10,
            1,
            0x100ff,
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
    1
}

#[repr(C)]
struct GuardedRx {
    before: u16,
    bytes: [u8; 4],
    after: u16,
}

impl GuardedRx {
    fn new() -> Self {
        Self {
            before: CANARIES as u16,
            bytes: [SENTINEL; 4],
            after: (CANARIES >> 16) as u16,
        }
    }

    fn record(&self, record: &mut Record, normal: bool) {
        let index = if normal { 84 } else { 82 };
        record.0[index] = unsafe {
            u32::from(core::ptr::read_volatile(&self.before))
                | (u32::from(core::ptr::read_volatile(&self.after)) << 16)
        };
        record.0[index + 1] = u32::from_le_bytes(unsafe { core::ptr::read_volatile(&self.bytes) });
        record.0[3] |= 1 << if normal { 15 } else { 14 };
    }
}

#[cfg(target_arch = "arm")]
fn timer() -> u32 {
    unsafe { core::ptr::read_volatile(0x400a_c028 as *const u32) }
}

#[cfg(target_arch = "arm")]
fn read32(offset: usize) -> u32 {
    unsafe { core::ptr::read_volatile((rp1_hal::addr::SPI0_BASE + offset) as *const u32) }
}

#[cfg(target_arch = "arm")]
fn write32(offset: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile((rp1_hal::addr::SPI0_BASE + offset) as *mut u32, value);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(target_arch = "arm")]
fn read_spi() -> [u32; 8] {
    [0x08, 0x10, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34].map(read32)
}

#[cfg(target_arch = "arm")]
fn read_config() -> [u32; 6] {
    [0, 0x14, 0x18, 0x1c, 0x4c, 0x108].map(read32)
}

#[cfg(target_arch = "arm")]
fn read_gpio() -> [u32; 4] {
    let mut snapshot = observation::Snapshot::new();
    observation::record(&mut snapshot, 0);
    let w = observation::pack(0, snapshot);
    [w[4], w[5], w[6], w[7]]
}

#[cfg(target_arch = "arm")]
fn read_route() -> [u32; 8] {
    let r = rp1_rt::spi0_irq_route_snapshot();
    [
        r.vtor, r.iser0, r.iser1, r.ispr0, r.ispr1, r.iabr0, r.iabr1, r.primask,
    ]
}

#[cfg(target_arch = "arm")]
fn wait_state(pad: u32, record: &mut Record, stage: usize) -> u32 {
    let start = timer();
    let mut result = FAIL_DEADLINE;
    // Both bounds apply; this does not provide a watchdog for a faulting MMIO read.
    for _ in 0..ITERATIONS {
        if timer().wrapping_sub(start) >= WAIT_US {
            break;
        }
        let s = read_spi();
        record.record_spi(stage, s);
        if !guard_ok(pad, read_gpio()) {
            result = FAIL_GUARD;
            break;
        }
        if read_config() != CONFIG {
            result = FAIL_CONFIG;
            break;
        }
        if state_ok(stage, s) {
            result = 1;
            break;
        }
        // Only transfer-in-progress is waitable. A lost retained byte, extra
        // frame, disabled SSI, changed SER/mask or fault stops without injection.
        let (ser, min_rx, max_rx) = if stage == 0 { (1, 0, 1) } else { (1, 1, 2) };
        if !matches!(stage, 0 | 3)
            || s[0] != 1
            || s[1] != ser
            || s[2] > 1
            || !(min_rx..=max_rx).contains(&s[3])
            || s[5] != 0x1e
            || s[6] & !0x10 != 0
            || s[7] & 0x0e != 0
        {
            result = FAIL_STATE;
            break;
        }
        core::hint::spin_loop();
    }
    record.0[86 + stage] = timer().wrapping_sub(start);
    if result == 1 && record.0[86 + stage] >= WAIT_US {
        FAIL_DEADLINE
    } else {
        result
    }
}

#[cfg(target_arch = "arm")]
fn retained_steps(
    transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>,
    pad: u32,
    record: &mut Record,
) -> u32 {
    record.0[76..82].copy_from_slice(&read_config());
    record.0[3] |= 1 << 9;
    if record.0[76..82] != CONFIG || read_spi()[..4] != [1, 0, 1, 0] {
        return FAIL_CONFIG;
    }
    if !guard_ok(pad, read_gpio()) {
        return FAIL_GUARD;
    }
    if transfer.start().is_err() {
        return FAIL_STATE;
    }
    let first = wait_state(pad, record, 0);
    if first != 1 {
        return first;
    }
    write32(0x10, 0); // SER only: do not disable, finish, prepare or read DR.
    let retained = wait_state(pad, record, 1);
    if retained != 1 {
        return retained;
    }
    // Exactly one fixed extra DR write, gated by idle/TX space/retained RX.
    // ponytail: one extra byte only; larger RX-capacity/overflow work is a later cohort.
    write32(0x60, 0);
    record.0[92] += 1;
    let queued = wait_state(pad, record, 2);
    if queued != 1 {
        return queued;
    }
    write32(0x10, 1);
    wait_state(pad, record, 3)
}

#[cfg(target_arch = "arm")]
fn retained_round(
    host: &mut rp1_hal::spi::Spi0Host,
    pad: u32,
    record: &mut Record,
    marker: &mut impl FnMut(usize),
) -> u32 {
    let before = read_route();
    record.0[94..102].copy_from_slice(&before);
    record.0[3] |= 1 << 11;
    if !route_ok(&before) {
        return FAIL_ROUTE;
    }
    let saved = match unsafe { rp1_rt::prepare_spi0_irq19_one_entry(before[7]) } {
        Some(saved) => saved,
        None => return FAIL_ROUTE,
    };
    let mut rx = GuardedRx::new();
    let result = match host.prepare_irq_transfer(&[0xa5], &mut rx.bytes[..1]) {
        Ok(mut transfer) => {
            let mut result = retained_steps(&mut transfer, pad, record);
            record.record_gpio(1, read_gpio());
            if !guard_ok(pad, record.gpio(1)) {
                result = FAIL_GUARD;
            }
            if result == 1 {
                marker(1);
            }
            unsafe { rp1_rt::mask_spi0_irq19_one_entry() };
            let start = timer();
            let aborted = transfer.abort();
            record.0[90] = timer().wrapping_sub(start);
            record.0[93] = u32::from(
                aborted.is_ok()
                    && transfer.state()
                        == rp1_hal::spi::Spi0RxState::Failed(rp1_hal::spi::Spi0RxError::Cancelled)
                    && transfer.received().is_empty(),
            );
            record.0[3] |= 1 << 10;
            record.record_spi(4, read_spi()); // Actual checked cleanup, before Drop.
            if record.0[93] != 1 || !state_ok(4, record.spi(4)) {
                FAIL_ABORT
            } else {
                result
            }
        }
        Err(_) => FAIL_PREPARE,
    };
    rx.record(record, false);
    record.record_gpio(2, read_gpio());
    unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
    record.0[102..110].copy_from_slice(&read_route());
    record.0[3] |= 1 << 12;
    if record.0[94..102] != record.0[102..110] {
        return FAIL_ROUTE;
    }
    if !guard_ok(pad, record.gpio(2)) {
        return FAIL_GUARD;
    }
    if record.0[82..84] != [CANARIES, 0xc3c3_c3c3] {
        return FAIL_RX;
    }
    if record.0[3] & (1 << 10) != 0 && record.0[90] >= WAIT_US {
        return FAIL_DEADLINE;
    }
    result
}

#[cfg(target_arch = "arm")]
#[inline(never)]
fn publish(decision: u32, mut record: Record) -> u32 {
    const _: () = assert!(WORDS * 4 == 448 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    record.0[2] = decision;
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in record.0.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC); // Terminal-only, all payload immutable.
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    decision
}

#[cfg(target_arch = "arm")]
pub fn publish_setup_error() {
    publish(FAIL_SETUP, Record::new());
}

#[cfg(target_arch = "arm")]
pub fn run(host: &mut rp1_hal::spi::Spi0Host, mut marker: impl FnMut(usize)) -> u32 {
    unsafe {
        core::ptr::write_volatile(rp1_hal::debug::MAILBOX_ADDR as *mut u32, 0);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let mut record = Record::new();
    let pad = match observation::apply_guarded_bias() {
        Ok(pad) => pad,
        Err(_) => return publish(FAIL_GUARD, record),
    };
    crate::delay_readback_units(8);
    record.record_gpio(0, read_gpio());
    if !guard_ok(pad, record.gpio(0)) {
        return publish(FAIL_GUARD, record);
    }
    marker(0);
    let retained = retained_round(host, pad, &mut record, &mut marker);
    if retained != 1 {
        return publish(retained, record);
    }
    // This same owner is reused only after checked abort and route restoration.
    marker(2);
    let mut normal = GuardedRx::new();
    let start = timer();
    let decision = irq::run_retained_rearm(host, (&mut normal.bytes[..1]).try_into().unwrap());
    record.0[91] = timer().wrapping_sub(start);
    for (i, word) in record.0[60..76].iter_mut().enumerate() {
        *word = unsafe {
            core::ptr::read_volatile((rp1_hal::debug::MAILBOX_ADDR as *const u32).add(i))
        };
    }
    record.0[3] |= 1 << 13;
    normal.record(&mut record, true);
    record.record_gpio(3, read_gpio());
    if decision != 1 || record.0[61] != decision {
        return publish(FAIL_IRQ, record);
    }
    publish(final_decision(pad, &record), record)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(primask: u32) -> Record {
        let mut r = Record::new();
        for stage in 0..4 {
            r.record_gpio(stage, [0x80, 0xabcd_00fb, INPUT_BITS, 0]);
        }
        for (stage, s) in [
            [1, 1, 0, 1, 14, 30, 16, 17],
            [1, 0, 0, 1, 14, 30, 16, 17],
            [1, 0, 1, 1, 10, 30, 16, 16],
            [1, 1, 0, 2, 14, 30, 16, 17],
            [0, 0, 0, 0, 6, 0, 0, 1],
        ]
        .into_iter()
        .enumerate()
        {
            r.record_spi(stage, s);
        }
        let before = 0x4010 | (primask << 3);
        r.0[60..76].copy_from_slice(&[
            irq::MAGIC,
            1,
            0,
            1,
            1,
            35,
            17,
            16,
            1,
            0x100ff,
            1,
            before,
            before | 2,
            0x4014,
            before,
            0x307ff,
        ]);
        r.0[76..82].copy_from_slice(&CONFIG);
        r.0[82..86].copy_from_slice(&[CANARIES, 0xc3c3_c3c3, CANARIES, 0xc3c3_c3ff]);
        r.0[86..94].copy_from_slice(&[100, 1, 1, 100, 1, 4_200, 1, 1]);
        r.0[94..102].copy_from_slice(&[0x2000_0000, 0, 0, 0, 1 << 21, 0, 0, primask]);
        r.0.copy_within(94..102, 102);
        r.0[3] = 0xffff;
        r
    }

    #[test]
    fn retained_gate_cleanup_irq_buffer_and_bounds_fail_closed() {
        assert_eq!(MAGIC, 0x324b_3053);
        assert_eq!(WORDS * 4, 448);
        assert_eq!(core::mem::size_of::<GuardedRx>(), 8);
        assert_eq!(core::mem::offset_of!(GuardedRx, bytes), 2);
        assert_eq!(core::mem::offset_of!(GuardedRx, after), 6);
        assert_ne!(final_decision(0xabcd_00fb, &Record::new()), 1);
        for primask in [0, 1] {
            let r = complete(primask);
            assert_eq!(final_decision(0xabcd_00fb, &r), 1);
            for flag in 0..16 {
                let mut bad = r;
                bad.0[3] ^= 1 << flag;
                assert_ne!(final_decision(0xabcd_00fb, &bad), 1);
            }
            for word in 20..112 {
                let mut bad = r;
                bad.0[word] ^= if (86..92).contains(&word) {
                    0x20000
                } else if [27, 35, 43, 51, 59].contains(&word) {
                    8
                } else {
                    1
                };
                assert_ne!(final_decision(0xabcd_00fb, &bad), 1, "word {word}");
            }
            for stage in 0..4 {
                for (reg, bit) in [(0, 0), (1, 31), (2, 13), (2, 17), (2, 18), (2, 19), (3, 9)] {
                    let mut bad = r;
                    bad.0[4 + stage * 4 + reg] ^= 1 << bit;
                    assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_GUARD);
                }
            }
            for word in [82, 83, 84, 85] {
                for bit in 0..32 {
                    let mut bad = r;
                    bad.0[word] ^= 1 << bit;
                    assert_eq!(final_decision(0xabcd_00fb, &bad), FAIL_RX);
                }
            }
            for stage in 0..4 {
                for bit in [2, 4, 8] {
                    let mut s = r.spi(stage);
                    s[7] |= bit;
                    assert!(!state_ok(stage, s));
                }
            }
        }
    }
}

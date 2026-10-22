#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_irq19_one_entry_rx_proof as irq;
#[cfg(target_arch = "arm")]
use crate::spi0_miso_input_observation as observation;

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0C2");
pub const WORDS: usize = 160;
const DEADLINE_US: u32 = 2_000;
const WAIT_US: u32 = 50_000;
const ITERATIONS: u32 = 12_500_000;
const CONFIG: [u32; 6] = [0x70000, 2_000, 0, 0, 0, 0];
const INPUT_BITS: u32 = 7 << 17;
const CANARIES: u32 = 0x5aa5_a55a;
const PASS_FLAGS: u32 = 0x00ff_ffff;
const REARM_FLAGS: u32 = PASS_FLAGS & !((1 << 4) | (1 << 5) | (1 << 14) | (1 << 16));
const FAIL_SETUP: u32 = 0x420;
const FAIL_GUARD: u32 = 0x421;
const FAIL_ROUTE: u32 = 0x422;
const FAIL_PREPARE: u32 = 0x423;
const FAIL_CONFIG: u32 = 0x424;
const FAIL_STATE: u32 = 0x425;
const FAIL_DEADLINE: u32 = 0x426;
const FAIL_ABORT: u32 = 0x427;
const FAIL_RX: u32 = 0x428;
const FAIL_IRQ: u32 = 0x429;
const FAIL_INCOMPLETE: u32 = 0x42a;
const DEADLINE_NOT_EXERCISED: u32 = 0x42b;
const FAIL_CAPACITY: u32 = 0x42c;

#[derive(Clone, Copy)]
struct Record([u32; WORDS]);

impl Record {
    fn new() -> Self {
        let mut w = [u32::MAX; WORDS];
        w[..5].copy_from_slice(&[MAGIC, 1, FAIL_SETUP, 0, 64]);
        w[6..9].copy_from_slice(&[DEADLINE_US, WAIT_US, ITERATIONS]);
        w[10] = 0;
        w[155..158].fill(0); // Counts of actual successful environment checks.
        Self(w)
    }

    fn gpio(&self, stage: usize) -> [u32; 4] {
        self.0[12 + stage * 4..16 + stage * 4].try_into().unwrap()
    }

    fn record_gpio(&mut self, stage: usize, s: [u32; 4]) {
        self.0[12 + stage * 4..16 + stage * 4].copy_from_slice(&s);
        self.0[3] |= 1 << stage;
    }

    fn spi(&self, stage: usize) -> [u32; 8] {
        self.0[36 + stage * 8..44 + stage * 8].try_into().unwrap()
    }

    fn record_spi(&mut self, stage: usize, s: [u32; 8]) {
        self.0[36 + stage * 8..44 + stage * 8].copy_from_slice(&s);
        self.0[3] |= 1 << (6 + stage);
    }
}

fn guard_ok(pad: u32, s: [u32; 4]) -> bool {
    pad & 0xff == 0xfb
        && s[0] == 0x80
        && s[1] == pad
        && s[2] & (1 << 13) == 0
        && s[2] & INPUT_BITS == INPUT_BITS
        && s[3] & (1 << 9) == 0
}

fn route_ok(r: &[u32]) -> bool {
    r.len() == 8 && r[..7] == [0x2000_0000, 0, 0, 0, 1 << 21, 0, 0] && r[7] <= 1
}

// Sequential SSIENR/SER/TXFLR/RXFLR/SR/IMR/ISR/RISR; never DR.
fn active_ok(s: [u32; 8]) -> bool {
    s[0..2] == [1, 1]
        && s[2] < 64
        && (1..64).contains(&s[3])
        && s[4] & 0x19 == 9
        && s[5..7] == [0x1e, 0x10]
        && s[7] & !1 == 0x10
}

fn prepared_ok(s: [u32; 8]) -> bool {
    s[..4] == [1, 0, 64, 0] && s[4] & 0x1f == 0 && s[5..7] == [0, 0] && s[7] & !1 == 0
}

fn cleanup_ok(s: [u32; 8]) -> bool {
    s[..4] == [0, 0, 0, 0] && s[4] & 0x1f == 6 && s[5..7] == [0, 0] && s[7] & !1 == 0
}

fn bound_exit(elapsed: u32, iterations: u32) -> u32 {
    if elapsed >= WAIT_US {
        2
    } else if iterations >= ITERATIONS {
        3
    } else {
        0
    }
}

fn cancellation_decision(r: &Record) -> u32 {
    let w = &r.0;
    if w[3] & REARM_FLAGS != REARM_FLAGS || w[3] & !PASS_FLAGS != 0 {
        return FAIL_INCOMPLETE;
    }
    if w[4..9] != [64, 64, DEADLINE_US, WAIT_US, ITERATIONS] {
        return FAIL_CAPACITY;
    }
    if (0..4).any(|stage| !guard_ok(w[9], r.gpio(stage))) {
        return FAIL_GUARD;
    }
    if w[76..82] != CONFIG || !(2..ITERATIONS).contains(&w[10]) || w[155..158] != [w[10] + 2; 3] {
        return FAIL_CONFIG;
    }
    if !route_ok(&w[82..90]) || w[82..90] != w[90..98] {
        return FAIL_ROUTE;
    }
    if !prepared_ok(r.spi(0)) || w[151..155] != [1, 2, 2, 2] {
        return FAIL_STATE;
    }
    if (1..4).any(|stage| !active_ok(r.spi(stage))) || w[158] & 0x19 != 9 {
        return DEADLINE_NOT_EXERCISED;
    }
    // FIFO tuples are sequential, not atomic: do not require TX+RX == 64.
    if r.spi(1)[2] < r.spi(2)[2]
        || r.spi(2)[2] < r.spi(3)[2]
        || r.spi(1)[3] > r.spi(2)[3]
        || r.spi(2)[3] > r.spi(3)[3]
    {
        return FAIL_STATE;
    }
    if w[11] != 1
        || w[134] > w[135]
        || w[135] >= DEADLINE_US
        || w[136] < DEADLINE_US
        || w[136..140].windows(2).any(|v| v[0] > v[1])
        || w[139] > w[159]
        || w[159] >= 2 * DEADLINE_US
        || bound_exit(w[159], w[10]) != 0
        || w[140] >= WAIT_US
        || !(4_000..WAIT_US).contains(&w[141])
    {
        return FAIL_DEADLINE;
    }
    if w[143..146] != [1, 0x105, 0] || !cleanup_ok(r.spi(4)) {
        return FAIL_ABORT;
    }
    if w[114..130] != [0xc3c3_c3c3; 16] || w[130] != CANARIES {
        return FAIL_RX;
    }
    if w[146..151] != [0; 5] {
        return FAIL_IRQ;
    }
    1
}

fn final_decision(r: &Record) -> u32 {
    let result = cancellation_decision(r);
    if result != 1 {
        return result;
    }
    let w = &r.0;
    if w[3] != PASS_FLAGS {
        return FAIL_INCOMPLETE;
    }
    if (4..6).any(|stage| !guard_ok(w[9], r.gpio(stage))) {
        return FAIL_GUARD;
    }
    if w[131..133] != [CANARIES, 0xc3c3_c3ff] {
        return FAIL_RX;
    }
    if !(4_000..100_000).contains(&w[142]) {
        return FAIL_DEADLINE;
    }
    let before = 0x4010 | (w[89] << 3);
    if w[98..114]
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
struct GuardedRx<const N: usize> {
    before: u16,
    bytes: [u8; N],
    after: u16,
}

impl<const N: usize> GuardedRx<N> {
    fn new() -> Self {
        Self {
            before: CANARIES as u16,
            bytes: [0xc3; N],
            after: (CANARIES >> 16) as u16,
        }
    }

    fn record(&self, r: &mut Record, normal: bool) {
        r.0[if normal { 131 } else { 130 }] = unsafe {
            u32::from(core::ptr::read_volatile(&self.before))
                | (u32::from(core::ptr::read_volatile(&self.after)) << 16)
        };
        for (i, bytes) in self.bytes.chunks_exact(4).enumerate() {
            r.0[if normal { 132 } else { 114 } + i] = u32::from_le_bytes(unsafe {
                core::ptr::read_volatile(bytes.as_ptr() as *const [u8; 4])
            });
        }
        r.0[3] |= 1 << if normal { 16 } else { 15 };
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
    observation::pack(0, snapshot)[4..8].try_into().unwrap()
}

#[cfg(target_arch = "arm")]
fn read_route() -> [u32; 8] {
    let s = rp1_rt::spi0_irq_route_snapshot();
    [
        s.vtor, s.iser0, s.iser1, s.ispr0, s.ispr1, s.iabr0, s.iabr1, s.primask,
    ]
}

#[cfg(target_arch = "arm")]
fn state_code(s: rp1_hal::spi::Spi0RxState) -> u32 {
    use rp1_hal::spi::{Spi0RxError, Spi0RxState};
    match s {
        Spi0RxState::Prepared => 1,
        Spi0RxState::Active => 2,
        Spi0RxState::RxComplete => 3,
        Spi0RxState::Complete => 4,
        Spi0RxState::Failed(Spi0RxError::Cancelled) => 0x105,
        Spi0RxState::Failed(_) => 5,
    }
}

#[cfg(target_arch = "arm")]
fn environment(r: &mut Record, gpio: [u32; 4]) -> u32 {
    if !guard_ok(r.0[9], gpio) {
        r.0[11] = 5;
        return FAIL_GUARD;
    }
    r.0[156] += 1;
    if read_config() != CONFIG {
        r.0[11] = 6;
        return FAIL_CONFIG;
    }
    r.0[155] += 1;
    if read_route() != r.0[82..90] || irq::deadline_irq_observation() != (0, 0) {
        r.0[11] = 7;
        return FAIL_ROUTE;
    }
    r.0[157] += 1;
    1
}

#[cfg(target_arch = "arm")]
fn active_steps(transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>, r: &mut Record) -> u32 {
    r.0[76..82].copy_from_slice(&read_config());
    r.0[3] |= 1 << 11;
    r.record_spi(0, read_spi());
    r.0[151] = state_code(transfer.state());
    if !prepared_ok(r.spi(0)) || r.0[151] != 1 {
        return FAIL_STATE;
    }
    let check = environment(r, read_gpio());
    if check != 1 {
        return check;
    }
    // No long marker, foreground DR read/write or slot publication in this interval.
    let start = timer();
    r.0[133] = start;
    if transfer.start().is_err() {
        r.0[11] = 9;
        return FAIL_STATE;
    }
    loop {
        let begin = timer().wrapping_sub(start);
        let bound = bound_exit(begin, r.0[10]);
        if bound != 0 {
            r.0[11] = bound;
            return FAIL_DEADLINE;
        }
        r.0[10] += 1;
        let spi = read_spi();
        let gpio = read_gpio();
        let state = state_code(transfer.state());
        let check = environment(r, gpio);
        let end = timer().wrapping_sub(start);
        if check != 1 {
            return check;
        }
        if begin >= DEADLINE_US {
            r.record_spi(2, spi);
            r.record_gpio(2, gpio);
            r.0[136..138].copy_from_slice(&[begin, end]);
            r.0[153] = state;
            r.0[3] |= 1 << 21;
            if !active_ok(spi) || state != 2 || r.0[3] & (1 << 20) == 0 {
                r.0[11] = 4;
                return DEADLINE_NOT_EXERCISED;
            }
            if end >= 2 * DEADLINE_US {
                r.0[11] = 2;
                return FAIL_DEADLINE;
            }
            break;
        }
        if state != 2
            || spi[0..2] != [1, 1]
            || spi[2] > 64
            || spi[3] >= 64
            || spi[5] != 0x1e
            || spi[6] & !0x10 != 0
            || spi[7] & !0x11 != 0
        {
            r.0[11] = 8;
            return FAIL_STATE;
        }
        if active_ok(spi) && end < DEADLINE_US && r.0[3] & (1 << 20) == 0 {
            r.record_spi(1, spi);
            r.record_gpio(1, gpio);
            r.0[134..136].copy_from_slice(&[begin, end]);
            r.0[152] = state;
            r.0[3] |= 1 << 20;
        }
        core::hint::spin_loop();
    }
    r.0[138] = timer().wrapping_sub(start);
    r.record_spi(3, read_spi());
    r.record_gpio(3, read_gpio());
    r.0[154] = state_code(transfer.state());
    let check = environment(r, r.gpio(3));
    r.0[150] = irq::deadline_irq_observation().1;
    r.0[139] = timer().wrapping_sub(start);
    r.0[3] |= 1 << 22;
    if check != 1 {
        return check;
    }
    if !active_ok(r.spi(3)) || r.0[154] != 2 {
        r.0[11] = 4;
        return DEADLINE_NOT_EXERCISED;
    }
    if r.0[139] >= 2 * DEADLINE_US {
        r.0[11] = 2;
        return FAIL_DEADLINE;
    }
    r.0[11] = 1;
    1
}

#[cfg(target_arch = "arm")]
fn checked_abort(transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>, r: &mut Record) {
    let start = timer();
    r.0[143] = u32::from(transfer.abort().is_ok());
    r.0[140] = timer().wrapping_sub(start);
    r.0[144] = state_code(transfer.state());
    r.0[145] = transfer.received().len() as u32;
    r.0[3] |= 1 << 17;
    r.record_spi(4, read_spi()); // Before Drop: checked result and actual cleanup.
}

#[cfg(target_arch = "arm")]
fn cancel_round(host: &mut rp1_hal::spi::Spi0Host, r: &mut Record) -> u32 {
    r.0[82..90].copy_from_slice(&read_route());
    (r.0[146], r.0[149]) = irq::deadline_irq_observation();
    r.0[3] |= 1 << 12;
    if !route_ok(&r.0[82..90]) || (r.0[146], r.0[149]) != (0, 0) {
        return FAIL_ROUTE;
    }
    let saved = match unsafe { rp1_rt::prepare_spi0_irq19_one_entry(r.0[89]) } {
        Some(saved) => saved,
        None => return FAIL_ROUTE,
    };
    let mut rx = GuardedRx::<64>::new();
    let result = match host.prepare_irq_transfer(&[0xa5; 64], &mut rx.bytes) {
        Ok(mut transfer) => {
            let mut result = active_steps(&mut transfer, r);
            unsafe { rp1_rt::mask_spi0_irq19_one_entry() };
            // Immediate BUSY recheck follows the full timestamp-bracketed tuple.
            if result == 1 {
                r.0[158] = read32(0x28);
                r.0[159] = timer().wrapping_sub(r.0[133]);
                r.0[3] |= 1 << 23;
                if r.0[158] & 0x19 != 9 || r.0[159] >= 2 * DEADLINE_US {
                    r.0[11] = 4;
                    result = DEADLINE_NOT_EXERCISED;
                }
            }
            checked_abort(&mut transfer, r);
            if r.0[143..146] != [1, 0x105, 0] || !cleanup_ok(r.spi(4)) {
                FAIL_ABORT
            } else {
                result
            }
        }
        // No object returned: internal Drop is not a checked-cleanup claim.
        // Terminal negative; parent must restore the controlled baseline.
        Err(_) => FAIL_PREPARE,
    };
    rx.record(r, false);
    r.0[147] = irq::deadline_irq_observation().0;
    unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
    r.0[90..98].copy_from_slice(&read_route());
    r.0[3] |= 1 << 13;
    if r.0[82..90] != r.0[90..98] {
        return FAIL_ROUTE;
    }
    result
}

#[cfg(target_arch = "arm")]
fn capacity64(host: &mut rp1_hal::spi::Spi0Host, r: &mut Record) -> bool {
    use rp1_hal::spi::{Spi0Error, Spi0RxError};
    let before = (read_spi(), read_config(), read_route(), read_gpio());
    let mut rx = [0xc3; 257];
    r.0[5] = match host.prepare_irq_transfer(&[0xa5; 257], &mut rx) {
        Err(Spi0RxError::Setup(Spi0Error::PayloadTooLong {
            len: 257,
            fifo_depth,
        })) => u32::from(fifo_depth),
        Ok(mut unexpected) => {
            checked_abort(&mut unexpected, r);
            0
        }
        _ => 0,
    };
    r.0[3] |= 1 << 19;
    r.0[5] == 64 && before == (read_spi(), read_config(), read_route(), read_gpio())
}

#[cfg(target_arch = "arm")]
fn quiet_interval(r: &mut Record) -> bool {
    let start = timer();
    for i in 0..ITERATIONS {
        let elapsed = timer().wrapping_sub(start);
        r.0[141] = elapsed;
        r.0[148] = irq::deadline_irq_observation().0;
        if bound_exit(elapsed, i) != 0
            || r.0[148] != 0
            || irq::deadline_irq_observation().1 != 0
            || read_route() != r.0[82..90]
            || read_config() != CONFIG
            || !cleanup_ok(read_spi())
            || !guard_ok(r.0[9], read_gpio())
        {
            return false;
        }
        if elapsed >= 4_000 {
            r.0[3] |= 1 << 18;
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

#[cfg(target_arch = "arm")]
#[inline(never)]
fn publish(decision: u32, mut r: Record) -> u32 {
    const _: () = assert!(WORDS * 4 == 640 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    r.0[2] = decision;
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in r.0.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC); // Terminal only, last body store fc00+636=fe7c.
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
    let mut r = Record::new();
    r.0[9] = match observation::apply_guarded_bias() {
        Ok(pad) => pad,
        Err(_) => return publish(FAIL_GUARD, r),
    };
    crate::delay_readback_units(8);
    r.record_gpio(0, read_gpio());
    if !guard_ok(r.0[9], r.gpio(0)) {
        return publish(FAIL_GUARD, r);
    }
    if !capacity64(host, &mut r) {
        return publish(FAIL_CAPACITY, r);
    }
    marker(0);
    let result = cancel_round(host, &mut r);
    if result != 1 {
        return publish(result, r);
    }
    if !quiet_interval(&mut r) {
        return publish(FAIL_IRQ, r);
    }
    let result = cancellation_decision(&r);
    if result != 1 {
        return publish(result, r);
    }
    marker(1); // Checked cancellation and quiet interval are complete, SPI disabled.
    marker(2);
    r.record_gpio(4, read_gpio());
    if !guard_ok(r.0[9], r.gpio(4)) {
        return publish(FAIL_GUARD, r);
    }
    let mut normal = GuardedRx::<4>::new();
    let start = timer();
    let result = irq::run_deadline_rearm(host, (&mut normal.bytes[..1]).try_into().unwrap());
    r.0[142] = timer().wrapping_sub(start);
    for (i, word) in r.0[98..114].iter_mut().enumerate() {
        *word = unsafe {
            core::ptr::read_volatile((rp1_hal::debug::MAILBOX_ADDR as *const u32).add(i))
        };
    }
    r.0[3] |= 1 << 14;
    normal.record(&mut r, true);
    // Normal helper has only these outer GPIO samples, not continuous guard monitoring.
    r.record_gpio(5, read_gpio());
    if result != 1 {
        return publish(FAIL_IRQ, r);
    }
    publish(final_decision(&r), r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(primask: u32) -> Record {
        let mut r = Record::new();
        r.0[5] = 64;
        r.0[9] = 0xabcd_00fb;
        r.0[10..12].copy_from_slice(&[25, 1]);
        for i in 0..6 {
            r.record_gpio(i, [0x80, r.0[9], INPUT_BITS, 0]);
        }
        for (i, s) in [
            [1, 0, 64, 0, 0, 0, 0, 0],
            [1, 1, 62, 1, 11, 30, 16, 16],
            [1, 1, 57, 6, 11, 30, 16, 16],
            [1, 1, 57, 6, 11, 30, 16, 16],
            [0, 0, 0, 0, 6, 0, 0, 1],
        ]
        .into_iter()
        .enumerate()
        {
            r.record_spi(i, s);
        }
        r.0[76..82].copy_from_slice(&CONFIG);
        r.0[82..90].copy_from_slice(&[0x2000_0000, 0, 0, 0, 1 << 21, 0, 0, primask]);
        r.0.copy_within(82..90, 90);
        let before = 0x4010 | (primask << 3);
        r.0[98..114].copy_from_slice(&[
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
        r.0[114..130].fill(0xc3c3_c3c3);
        r.0[130..143].copy_from_slice(&[
            CANARIES,
            CANARIES,
            0xc3c3_c3ff,
            0xffff_ff00,
            350,
            390,
            2000,
            2040,
            2042,
            2080,
            8,
            4000,
            4300,
        ]);
        r.0[143..155].copy_from_slice(&[1, 0x105, 0, 0, 0, 0, 0, 0, 1, 2, 2, 2]);
        r.0[155..160].copy_from_slice(&[27, 27, 27, 11, 2085]);
        r.0[3] = PASS_FLAGS;
        r
    }

    #[test]
    fn cancellation_deadline_progress_cleanup_and_full_buffer_fail_closed() {
        assert_eq!(MAGIC, 0x3243_3053);
        assert_ne!(MAGIC, irq::MAGIC);
        assert_eq!(WORDS * 4, 640);
        assert_eq!(core::mem::size_of::<GuardedRx<64>>(), 68);
        assert_eq!(core::mem::offset_of!(GuardedRx<64>, bytes), 2);
        assert_eq!(core::mem::offset_of!(GuardedRx<64>, after), 66);
        assert_ne!(final_decision(&Record::new()), 1);
        for primask in [0, 1] {
            let r = complete(primask);
            assert_eq!(final_decision(&r), 1);
            for bit in 0..32 {
                let mut bad = r;
                bad.0[3] ^= 1 << bit;
                assert_ne!(final_decision(&bad), 1, "validity {bit}");
            }
            for i in 114..133 {
                for bit in 0..32 {
                    let mut bad = r;
                    bad.0[i] ^= 1 << bit;
                    assert_eq!(final_decision(&bad), FAIL_RX, "buffer {i}/{bit}");
                }
            }
            for stage in 0..6 {
                for (reg, bit) in [(0, 0), (1, 31), (2, 13), (2, 17), (2, 18), (2, 19), (3, 9)] {
                    let mut bad = r;
                    bad.0[12 + stage * 4 + reg] ^= 1 << bit;
                    assert_eq!(final_decision(&bad), FAIL_GUARD);
                }
            }
            for stage in 1..4 {
                for (reg, value) in [
                    (0, 0),
                    (1, 0),
                    (2, 64),
                    (3, 0),
                    (3, 64),
                    (4, 10),
                    (4, 3),
                    (4, 27),
                    (5, 0),
                    (6, 8),
                    (7, 24),
                    (7, 18),
                    (7, 20),
                ] {
                    let mut bad = r;
                    bad.0[36 + stage * 8 + reg] = value;
                    assert_eq!(final_decision(&bad), DEADLINE_NOT_EXERCISED);
                }
            }
            for (word, value) in [
                (10, ITERATIONS),
                (11, 2),
                (11, 3),
                (11, 4),
                (134, 400),
                (135, 2000),
                (136, 1999),
                (137, 2043),
                (138, 2039),
                (139, 4000),
                (140, WAIT_US),
                (141, 3999),
                (141, WAIT_US),
                (142, 3999),
                (142, 100000),
                (159, 2079),
                (159, 4000),
            ] {
                let mut bad = r;
                bad.0[word] = value;
                assert_ne!(final_decision(&bad), 1, "timing {word}/{value}");
            }
            for word in (4..9).chain(76..114).chain(143..158) {
                let mut bad = r;
                bad.0[word] ^= 1;
                assert_ne!(final_decision(&bad), 1, "evidence {word}");
            }
            for word in 68..76 {
                let mut bad = r;
                bad.0[word] ^= if word == 75 { 8 } else { 1 };
                assert_eq!(final_decision(&bad), FAIL_ABORT, "cleanup {word}");
            }
            let mut bad = r;
            bad.0[158] &= !1;
            assert_eq!(final_decision(&bad), DEADLINE_NOT_EXERCISED);
            for status in [3, 27] {
                let mut bad = r;
                bad.0[158] = status;
                assert_eq!(final_decision(&bad), DEADLINE_NOT_EXERCISED);
            }
            for status in [1, 2, 4, 8, 16] {
                let mut bad = r;
                bad.0[40] = status;
                assert_eq!(final_decision(&bad), FAIL_STATE);
            }
            let mut bad = r;
            bad.0[54] = 63;
            assert_eq!(final_decision(&bad), FAIL_STATE);
            let mut bad = r;
            bad.0[63] = 5;
            assert_eq!(final_decision(&bad), FAIL_STATE);
            // Incidental TXEI and non-atomic FIFO occupancy are not fake exact sums.
            let mut good = r;
            good.0[55] = 7;
            good.0[63] = 8;
            for word in [43, 51, 59, 67, 75] {
                good.0[word] ^= 1;
            }
            assert_eq!(final_decision(&good), 1);
        }
        assert_eq!(bound_exit(0, ITERATIONS), 3); // Stalled timer, iteration bound.
        assert_eq!(bound_exit(WAIT_US, 0), 2);
        assert_eq!(bound_exit(WAIT_US - 1, ITERATIONS - 1), 0);
        let mut actual = GuardedRx::<64>::new();
        actual.bytes[63] = 0;
        let mut r = complete(0);
        actual.record(&mut r, false);
        assert_eq!(r.0[129], 0x00c3_c3c3);
        assert_eq!(final_decision(&r), FAIL_RX);
    }
}

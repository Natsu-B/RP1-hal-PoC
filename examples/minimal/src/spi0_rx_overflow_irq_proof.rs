#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

use crate::spi0_irq19_one_entry_rx_proof as irq;
#[cfg(target_arch = "arm")]
use crate::spi0_miso_input_observation as observation;
#[cfg(target_arch = "arm")]
use core::sync::atomic::{AtomicU32, Ordering};

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0O2");
pub const WORDS: usize = 192;
const WAIT_US: u32 = 50_000;
const ITERATIONS: u32 = 12_500_000;
const CONFIG: [u32; 6] = [0x70000, 2_000, 0, 0, 0, 0];
const INPUT_BITS: u32 = 7 << 17;
const CANARIES: u32 = 0x5aa5_a55a;
const FAULT8: u32 = 0x10008;
const FAIL_SETUP: u32 = 0x400;
const FAIL_GUARD: u32 = 0x401;
const FAIL_ROUTE: u32 = 0x402;
const FAIL_PREPARE: u32 = 0x403;
const FAIL_CONFIG: u32 = 0x404;
const FAIL_STATE: u32 = 0x405;
const FAIL_DEADLINE: u32 = 0x406;
const FAIL_CLEANUP: u32 = 0x407;
const FAIL_RX: u32 = 0x408;
const FAIL_IRQ: u32 = 0x409;
const FAIL_INCOMPLETE: u32 = 0x40a;
const FAIL_CAPACITY: u32 = 0x40b;

#[derive(Clone, Copy)]
struct Record([u32; WORDS]);

impl Record {
    fn new() -> Self {
        let mut w = [u32::MAX; WORDS];
        w[..4].copy_from_slice(&[MAGIC, 1, FAIL_SETUP, 0]);
        w[115] = 0;
        w[186..192].copy_from_slice(&[0, 0, WAIT_US, ITERATIONS, 0, 0]);
        Self(w)
    }

    fn gpio(&mut self, stage: usize, regs: [u32; 4]) {
        self.0[4 + stage * 4..8 + stage * 4].copy_from_slice(&regs);
        self.0[3] |= 1 << stage;
    }

    fn spi(&mut self, stage: usize, regs: [u32; 8]) {
        self.0[20 + stage * 8..28 + stage * 8].copy_from_slice(&regs);
        self.0[3] |= 1 << (4 + stage);
    }

    fn route(&mut self, stage: usize, regs: [u32; 8]) {
        self.0[128 + stage * 8..136 + stage * 8].copy_from_slice(&regs);
        self.0[3] |= 1 << (14 + stage);
    }
}

fn guard_ok(pad: u32, gpio: &[u32]) -> bool {
    pad & 0xff == 0xfb
        && gpio[0] == 0x80
        && gpio[1] == pad
        && gpio[2] & (1 << 13) == 0
        && gpio[2] & INPUT_BITS == INPUT_BITS
        && gpio[3] & (1 << 9) == 0
}

// SSIENR, SER, TXFLR, RXFLR, SR, IMR, ISR, RISR. Never DR.
fn state_ok(stage: usize, s: &[u32]) -> bool {
    if stage >= 7 {
        return s[..4] == [0, 0, 0, 0]
            && s[4] & 0x1f == 6
            && s[5..7] == [0, 0]
            // stop/disable may already clear RXOI; do not assume ICR causality.
            && if stage == 7 { s[7] & !9 == 0 } else { s[7] & !1 == 0 };
    }
    let (ser, tx, sr, imr, isr, raw) = match stage {
        0 => (1, 0, 0x1e, 0x1e, 0x10, 0x10),
        1 => (0, 0, 0x1e, 0x1e, 0x10, 0x10),
        2 | 3 => (0, 0, 0x1e, 0x0e, 0, 0x10),
        4 => (0, 1, 0x1a, 0x0e, 0, 0x10),
        5 | 6 => (1, 0, 0x1e, 0x0e, 8, 0x18),
        _ => return false,
    };
    s[..4] == [1, ser, tx, 64] && s[4] & 0x1f == sr && s[5..7] == [imr, isr] && s[7] & !1 == raw
}

fn route_ok(route: &[u32], primask: u32, pending: u32, active: u32) -> bool {
    primask <= 1 && route == [0x2000_0000, 0, 0, pending, 1 << 21, active, 0, primask]
}

fn handler_precheck(old: u32, ipsr: u32, spi: &[u32], has_transfer: bool) -> bool {
    old == 0 && ipsr == 35 && state_ok(6, spi) && has_transfer
}

fn error_decision(r: &Record) -> u32 {
    let w = &r.0;
    // All error-round observations, excluding final GPIO and normal telemetry.
    if w[3] & 0xe7fff7 != 0xe7fff7 {
        return FAIL_INCOMPLETE;
    }
    if (0..3).any(|s| !guard_ok(w[127], &w[4 + s * 4..8 + s * 4])) {
        return FAIL_GUARD;
    }
    if w[108..114] != CONFIG {
        return FAIL_CONFIG;
    }
    if (0..9).any(|s| !state_ok(s, &w[20 + s * 8..28 + s * 8])) || w[115] != 1 {
        return FAIL_STATE;
    }
    if w[114] != 64 {
        return FAIL_CAPACITY;
    }
    if w[116..124] != [1, 35, FAULT8, 0, 8, FAULT8, 0, 1] {
        return FAIL_IRQ;
    }
    if w[124] >= WAIT_US
        || !(4_000..WAIT_US).contains(&w[125])
        || w[179..186].iter().any(|v| *v >= WAIT_US)
    {
        return FAIL_DEADLINE;
    }
    if w[186..190] != [0, 1, WAIT_US, ITERATIONS] {
        return FAIL_CLEANUP;
    }
    if w[160] != CANARIES || w[161..177] != [0xc3c3_c3c3; 16] {
        return FAIL_RX;
    }
    let primask = w[135];
    if !route_ok(&w[128..136], primask, 0, 0)
        || !route_ok(&w[136..144], primask, 1 << 19, 0)
        || !route_ok(&w[144..152], 0, 0, 1 << 19)
        || w[128..136] != w[152..160]
    {
        return FAIL_ROUTE;
    }
    1
}

fn final_decision(r: &Record) -> u32 {
    let error = error_decision(r);
    if error != 1 {
        return error;
    }
    let w = &r.0;
    if w[3] != 0xffffff || w[190..192] != [7, 0] {
        return FAIL_INCOMPLETE;
    }
    if !guard_ok(w[127], &w[16..20]) {
        return FAIL_GUARD;
    }
    if !(4_000..100_000).contains(&w[126]) {
        return FAIL_DEADLINE;
    }
    if w[177..179] != [CANARIES, 0xc3c3_c3ff] {
        return FAIL_RX;
    }
    let before = 0x4010 | (w[135] << 3);
    if w[92..108]
        != [
            irq::MAGIC,
            1,
            1,
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
        let index = if normal { 177 } else { 160 };
        r.0[index] = unsafe {
            u32::from(core::ptr::read_volatile(&self.before))
                | (u32::from(core::ptr::read_volatile(&self.after)) << 16)
        };
        for (i, bytes) in self.bytes.chunks_exact(4).enumerate() {
            let actual = unsafe { core::ptr::read_volatile(bytes.as_ptr() as *const [u8; 4]) };
            r.0[index + 1 + i] = u32::from_le_bytes(actual);
        }
        r.0[3] |= 1 << if normal { 19 } else { 18 };
    }
}

#[cfg(target_arch = "arm")]
static SLOT: irq::TransferSlot = irq::TransferSlot::new();
#[cfg(target_arch = "arm")]
static ACTIVE: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static HANDLER: [AtomicU32; 28] = [const { AtomicU32::new(u32::MAX) }; 28];

#[cfg(target_arch = "arm")]
pub fn error_round_active() -> bool {
    ACTIVE.load(Ordering::Acquire) == 1
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
    [8, 0x10, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34].map(read32)
}

#[cfg(target_arch = "arm")]
fn read_config() -> [u32; 6] {
    [0, 0x14, 0x18, 0x1c, 0x4c, 0x108].map(read32)
}

#[cfg(target_arch = "arm")]
fn read_gpio() -> [u32; 4] {
    let mut s = observation::Snapshot::new();
    observation::record(&mut s, 0);
    observation::pack(0, s)[4..8].try_into().unwrap()
}

#[cfg(target_arch = "arm")]
fn read_route() -> [u32; 8] {
    let r = rp1_rt::spi0_irq_route_snapshot();
    [
        r.vtor, r.iser0, r.iser1, r.ispr0, r.ispr1, r.iabr0, r.iabr1, r.primask,
    ]
}

#[cfg(target_arch = "arm")]
fn error_code(error: rp1_hal::spi::Spi0RxError) -> u32 {
    use rp1_hal::spi::Spi0RxError;
    match error {
        Spi0RxError::InterruptFault(bits) => bits,
        Spi0RxError::Cancelled => 0x100,
        Spi0RxError::CleanupReadback => 0x101,
        _ => 0xffff,
    }
}

#[cfg(target_arch = "arm")]
fn state_code(state: rp1_hal::spi::Spi0RxState) -> u32 {
    use rp1_hal::spi::Spi0RxState;
    match state {
        Spi0RxState::Prepared => 1,
        Spi0RxState::Active => 2,
        Spi0RxState::RxComplete => 3,
        Spi0RxState::Complete => 4,
        Spi0RxState::Failed(error) => 0x10000 | error_code(error),
    }
}

#[cfg(target_arch = "arm")]
pub unsafe fn on_irq() {
    let ipsr: u32;
    unsafe {
        core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        rp1_rt::mask_spi0_irq19_one_entry();
    }
    // One IRQ19 writer, no exclusive retry. Foreground cannot access SLOT until masked/withdrawn.
    let old = COUNT.load(Ordering::Relaxed);
    COUNT.store(old.wrapping_add(1), Ordering::Release);
    if old != 0 {
        return;
    } // First evidence is immutable; count still rejects replay.
    let spi = read_spi();
    let route = read_route();
    for (i, v) in spi.into_iter().chain(route).enumerate() {
        HANDLER[if i < 8 { i } else { i + 8 }].store(v, Ordering::Relaxed);
    }
    HANDLER[24].store(ipsr, Ordering::Relaxed);
    let ptr = unsafe { SLOT.get_for_isr() };
    if !handler_precheck(old, ipsr, &spi, ptr.is_some()) {
        HANDLER[27].store(FAIL_IRQ, Ordering::Relaxed);
        return;
    }
    let transfer = unsafe { &mut *ptr.unwrap() };
    let state = state_code(transfer.on_interrupt()); // Production error-first service, not abort.
    let received = transfer.received().len() as u32;
    HANDLER[25].store(state, Ordering::Relaxed);
    HANDLER[26].store(received, Ordering::Relaxed);
    HANDLER[27].store(
        if state == FAULT8 && received == 0 {
            0
        } else {
            FAIL_IRQ
        },
        Ordering::Relaxed,
    );
    for (word, actual) in HANDLER[8..16].iter().zip(read_spi()) {
        word.store(actual, Ordering::Relaxed);
    }
}

#[cfg(target_arch = "arm")]
fn wait_state(r: &mut Record, stage: usize) -> u32 {
    let start = timer();
    let mut decision = FAIL_DEADLINE;
    for _ in 0..ITERATIONS {
        if timer().wrapping_sub(start) >= WAIT_US {
            break;
        }
        let s = read_spi();
        r.spi(stage, s);
        let wrapper = u32::from(stage >= 3);
        let mut config = CONFIG;
        config[5] = wrapper;
        if !guard_ok(r.0[127], &read_gpio()) {
            decision = FAIL_GUARD;
            break;
        }
        if read_config() != config {
            decision = FAIL_CONFIG;
            break;
        }
        let route = read_route();
        let pending = if stage == 5 { 1 << 19 } else { 0 };
        if state_ok(stage, &s) && route_ok(&route, r.0[135], pending, 0) {
            decision = 1;
            break;
        }
        // Only moving prefill or the one extra frame may wait. Reject lost RX,
        // wrong SER/mask/route or any unexpected fault before further injection.
        let waitable = if stage == 0 {
            s[0] == 1
                && s[1] == 1
                && s[2] <= 64
                && s[3] <= 64
                && s[5] == 0x1e
                && s[6] & !0x10 == 0
                && s[7] & 0x0e == 0
                && route_ok(&route, r.0[135], 0, 0)
        } else if stage == 5 {
            s[0] == 1
                && s[1] == 1
                && s[2] <= 1
                && s[3] == 64
                && s[5] == 0x0e
                && s[6] & !8 == 0
                && s[7] & 6 == 0
                && (route_ok(&route, r.0[135], 0, 0) || route_ok(&route, r.0[135], 1 << 19, 0))
        } else {
            false
        };
        if !waitable {
            decision = FAIL_STATE;
            break;
        }
        core::hint::spin_loop();
    }
    r.0[179 + stage] = timer().wrapping_sub(start);
    if r.0[179 + stage] >= WAIT_US {
        FAIL_DEADLINE
    } else {
        decision
    }
}

#[cfg(target_arch = "arm")]
fn inject(transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>, r: &mut Record) -> u32 {
    r.0[108..114].copy_from_slice(&read_config());
    r.0[3] |= 1 << 13;
    if r.0[108..114] != CONFIG
        || read_spi()[..4] != [1, 0, 64, 0]
        || !route_ok(&read_route(), r.0[135], 0, 0)
    {
        return FAIL_CONFIG;
    }
    if !guard_ok(r.0[127], &read_gpio()) {
        return FAIL_GUARD;
    }
    if transfer.start().is_err() {
        return FAIL_STATE;
    }
    let full = wait_state(r, 0);
    if full != 1 {
        return full;
    } // Actual RXFLR64 AND SR.RF_FULL, not API capacity.
    write32(0x10, 0);
    let retained = wait_state(r, 1);
    if retained != 1 {
        return retained;
    }
    write32(0x2c, 0x0e); // Same normal start, remove RXFI only after full+SER0.
    let masked = wait_state(r, 2);
    if masked != 1 {
        return masked;
    }
    write32(0x108, 1);
    let wrapper = wait_state(r, 3);
    if wrapper != 1 {
        return wrapper;
    }
    // ponytail: this fixed experiment queues exactly one extra byte, never a refill loop.
    write32(0x60, 0);
    r.0[115] += 1;
    let queued = wait_state(r, 4);
    if queued != 1 {
        return queued;
    }
    write32(0x10, 1);
    wait_state(r, 5)
}

#[cfg(target_arch = "arm")]
fn checked_abort(transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>, r: &mut Record) {
    r.0[186] = if transfer.abort().is_ok()
        && transfer.state()
            == rp1_hal::spi::Spi0RxState::Failed(rp1_hal::spi::Spi0RxError::Cancelled)
        && state_ok(8, &read_spi())
    {
        1
    } else {
        2
    };
}

#[cfg(target_arch = "arm")]
fn service_error(
    transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>,
    r: &mut Record,
    saved: rp1_rt::Spi0Irq19OneEntrySaved,
) -> u32 {
    r.route(1, read_route());
    if !route_ok(&r.0[136..144], r.0[135], 1 << 19, 0) {
        return FAIL_ROUTE;
    }
    COUNT.store(0, Ordering::Relaxed);
    for word in &HANDLER {
        word.store(u32::MAX, Ordering::Relaxed);
    }
    ACTIVE.store(1, Ordering::Release);
    unsafe {
        SLOT.publish(transfer);
    }
    let start = timer();
    unsafe {
        rp1_rt::enable_spi0_irq19_one_entry_after_source_asserted(saved);
    }
    for _ in 0..ITERATIONS {
        if COUNT.load(Ordering::Acquire) != 0 || timer().wrapping_sub(start) >= WAIT_US {
            break;
        }
        if !guard_ok(r.0[127], &read_gpio()) {
            break;
        }
        core::hint::spin_loop();
    }
    unsafe {
        rp1_rt::mask_spi0_irq19_one_entry();
        SLOT.withdraw();
    }
    r.0[185] = timer().wrapping_sub(start);
    let data = HANDLER.each_ref().map(|v| v.load(Ordering::Acquire));
    r.spi(6, data[..8].try_into().unwrap());
    r.spi(7, data[8..16].try_into().unwrap());
    r.route(2, data[16..24].try_into().unwrap());
    r.0[116..120].copy_from_slice(&[COUNT.load(Ordering::Acquire), data[24], data[25], data[26]]);
    r.0[122] = data[27];
    let start = timer();
    for _ in 0..ITERATIONS {
        if timer().wrapping_sub(start) >= 4_000 || !guard_ok(r.0[127], &read_gpio()) {
            break;
        }
        core::hint::spin_loop();
    }
    r.0[125] = timer().wrapping_sub(start);
    r.0[123] = COUNT.load(Ordering::Acquire);
    r.0[3] |= 1 << 22;
    ACTIVE.store(0, Ordering::Release); // IRQ19 remains masked and slot withdrawn.
    if r.0[116..120] != [1, 35, FAULT8, 0] || data[27] != 0 {
        return FAIL_IRQ;
    }
    // Capture the original fault before finish/cleanup; abort would relabel it.
    let start = timer();
    r.0[120] = transfer.finish().map_or_else(error_code, |_| 0);
    r.0[124] = timer().wrapping_sub(start);
    r.0[121] = state_code(transfer.state());
    r.0[3] |= 1 << 21;
    r.spi(8, read_spi());
    r.0[187] = u32::from(r.0[120..122] == [8, FAULT8] && state_ok(8, &r.0[84..92]));
    if r.0[187] != 1 { FAIL_CLEANUP } else { 1 }
}

#[cfg(target_arch = "arm")]
fn error_round(
    host: &mut rp1_hal::spi::Spi0Host,
    r: &mut Record,
    marker: &mut impl FnMut(usize),
) -> u32 {
    r.route(0, read_route());
    if !route_ok(&r.0[128..136], r.0[135], 0, 0) {
        return FAIL_ROUTE;
    }
    let saved = match unsafe { rp1_rt::prepare_spi0_irq19_one_entry(r.0[135]) } {
        Some(saved) => saved,
        None => return FAIL_ROUTE,
    };
    let mut rx = GuardedRx::<64>::new();
    let result = match host.prepare_irq_transfer(&[0xa5; 64], &mut rx.bytes) {
        Ok(mut transfer) => {
            let mut result = inject(&mut transfer, r);
            r.gpio(1, read_gpio());
            if !guard_ok(r.0[127], &r.0[8..12]) {
                result = FAIL_GUARD;
            }
            if result == 1 {
                marker(1);
                r.0[190] |= 2;
                result = service_error(&mut transfer, r, saved);
            }
            unsafe {
                rp1_rt::mask_spi0_irq19_one_entry();
                SLOT.withdraw();
            }
            ACTIVE.store(0, Ordering::Release);
            // Expected finish was already captured. Any other exit receives checked abort.
            if r.0[187] != 1 {
                checked_abort(&mut transfer, r);
            }
            if r.0[186] == 2 {
                result = FAIL_CLEANUP;
            }
            result
        }
        Err(_) => FAIL_PREPARE,
    };
    rx.record(r, false);
    r.gpio(2, read_gpio());
    unsafe {
        rp1_rt::restore_spi0_irq19_one_entry(saved);
    }
    r.route(3, read_route());
    if r.0[128..136] != r.0[152..160] {
        return FAIL_ROUTE;
    }
    if result != 1 {
        return result;
    }
    let mut final_config = CONFIG;
    final_config[5] = 1;
    if read_config() != final_config {
        return FAIL_CONFIG;
    }
    error_decision(r)
}

#[cfg(target_arch = "arm")]
fn capacity64(host: &mut rp1_hal::spi::Spi0Host, r: &mut Record) -> bool {
    use rp1_hal::spi::{Spi0Error, Spi0RxError};
    let before = (read_spi(), read_config(), read_route(), read_gpio());
    let mut rx = [0xc3; 257];
    let capacity = match host.prepare_irq_transfer(&[0xa5; 257], &mut rx) {
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
    r.0[114] = capacity;
    r.0[3] |= 1 << 23;
    capacity == 64 && before == (read_spi(), read_config(), read_route(), read_gpio())
}

#[cfg(target_arch = "arm")]
#[inline(never)]
fn publish(decision: u32, mut record: Record) -> u32 {
    const _: () = assert!(WORDS * 4 == 768 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
    record.0[2] = decision;
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in record.0.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC); // Terminal-only; last word is 0x2000fefc, never ff00.
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
    r.0[127] = match observation::apply_guarded_bias() {
        Ok(pad) => pad,
        Err(_) => return publish(FAIL_GUARD, r),
    };
    crate::delay_readback_units(8);
    r.gpio(0, read_gpio());
    if !guard_ok(r.0[127], &r.0[4..8]) {
        return publish(FAIL_GUARD, r);
    }
    if !capacity64(host, &mut r) {
        return publish(FAIL_CAPACITY, r);
    }
    marker(0);
    r.0[190] |= 1;
    let result = error_round(host, &mut r, &mut marker);
    if result != 1 {
        return publish(result, r);
    }
    marker(2);
    r.0[190] |= 4;
    let mut normal = GuardedRx::<4>::new();
    let start = timer();
    let normal_result = irq::run_overflow_rearm(host, (&mut normal.bytes[..1]).try_into().unwrap());
    r.0[126] = timer().wrapping_sub(start);
    for (i, word) in r.0[92..108].iter_mut().enumerate() {
        *word = unsafe {
            core::ptr::read_volatile((rp1_hal::debug::MAILBOX_ADDR as *const u32).add(i))
        };
    }
    r.0[3] |= 1 << 20;
    normal.record(&mut r, true);
    r.gpio(3, read_gpio());
    if normal_result != 1 {
        return publish(FAIL_IRQ, r);
    }
    publish(final_decision(&r), r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete(primask: u32) -> Record {
        let mut r = Record::new();
        let w = &mut r.0;
        w[3] = 0xffffff;
        for gpio in w[4..20].chunks_exact_mut(4) {
            gpio.copy_from_slice(&[0x80, 0xabcd00fb, INPUT_BITS, 0]);
        }
        for (i, s) in [
            [1, 1, 0, 64, 30, 30, 16, 17],
            [1, 0, 0, 64, 30, 30, 16, 17],
            [1, 0, 0, 64, 30, 14, 0, 17],
            [1, 0, 0, 64, 30, 14, 0, 17],
            [1, 0, 1, 64, 26, 14, 0, 16],
            [1, 1, 0, 64, 30, 14, 8, 25],
            [1, 1, 0, 64, 30, 14, 8, 25],
            [0, 0, 0, 0, 6, 0, 0, 1],
            [0, 0, 0, 0, 6, 0, 0, 1],
        ]
        .into_iter()
        .enumerate()
        {
            w[20 + i * 8..28 + i * 8].copy_from_slice(&s);
        }
        let before = 0x4010 | (primask << 3);
        w[92..108].copy_from_slice(&[
            irq::MAGIC,
            1,
            1,
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
        w[108..114].copy_from_slice(&CONFIG);
        w[114..128].copy_from_slice(&[
            64, 1, 1, 35, FAULT8, 0, 8, FAULT8, 0, 1, 1, 4_100, 4_200, 0xabcd00fb,
        ]);
        for (i, (p, pending, active)) in [
            (primask, 0, 0),
            (primask, 1 << 19, 0),
            (0, 0, 1 << 19),
            (primask, 0, 0),
        ]
        .into_iter()
        .enumerate()
        {
            w[128 + i * 8..136 + i * 8].copy_from_slice(&[
                0x20000000,
                0,
                0,
                pending,
                1 << 21,
                active,
                0,
                p,
            ]);
        }
        w[160] = CANARIES;
        w[161..177].fill(0xc3c3_c3c3);
        w[177..179].copy_from_slice(&[CANARIES, 0xc3c3_c3ff]);
        w[179..186].fill(100);
        w[187] = 1;
        w[190] = 7;
        r
    }

    #[test]
    fn production_predicates_require_actual_full_fault8_cleanup_all64_bytes_and_rearm() {
        assert_eq!(MAGIC, 0x324f3053);
        assert_eq!(WORDS * 4, 768);
        assert_eq!(core::mem::size_of::<GuardedRx<64>>(), 68);
        assert_eq!(core::mem::offset_of!(GuardedRx<64>, bytes), 2);
        assert_eq!(core::mem::offset_of!(GuardedRx<64>, after), 66);
        assert_ne!(final_decision(&Record::new()), 1);
        for primask in [0, 1] {
            let r = complete(primask);
            assert_eq!(error_decision(&r), 1);
            assert_eq!(final_decision(&r), 1);
            for bit in 0..24 {
                let mut bad = r;
                bad.0[3] ^= 1 << bit;
                assert_ne!(final_decision(&bad), 1, "valid bit {bit}");
            }
            for word in 20..192 {
                let mut bad = r;
                bad.0[word] ^= if [124, 125, 126, 179, 180, 181, 182, 183, 184, 185].contains(&word)
                {
                    0x20000
                } else if word < 92 && (word - 20) % 8 == 7 {
                    4
                } else {
                    1
                };
                assert_ne!(final_decision(&bad), 1, "word {word}");
            }
            for stage in 0..4 {
                for (reg, bit) in [(0, 0), (1, 31), (2, 13), (2, 17), (2, 18), (2, 19), (3, 9)] {
                    let mut bad = r;
                    bad.0[4 + stage * 4 + reg] ^= 1 << bit;
                    assert_ne!(final_decision(&bad), 1);
                }
            }
            for word in 160..179 {
                for bit in 0..32 {
                    let mut bad = r;
                    bad.0[word] ^= 1 << bit;
                    assert_ne!(final_decision(&bad), 1);
                }
            }
            let source = &r.0[68..76];
            assert!(handler_precheck(0, 35, source, true));
            assert!(!handler_precheck(1, 35, source, true));
            assert!(!handler_precheck(0, 19, source, true));
            assert!(!handler_precheck(0, 35, source, false));
            let mut stopped_with_latch = r;
            stopped_with_latch.0[83] |= 8;
            assert_eq!(final_decision(&stopped_with_latch), 1); // No isolated ICR claim.
        }
    }
}

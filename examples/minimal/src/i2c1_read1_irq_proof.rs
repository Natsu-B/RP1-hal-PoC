//! One fresh-boot READ|STOP, real RX_FULL gate, exactly one ISR FIFO pop.
//! ponytail: gated one-entry proof; multi-entry async/rearm needs a new contract.

use rp1_hal::i2c::{I2C1_READ1_COMMAND, I2C1_READ1_FATAL as FATAL, I2C1_READ1_MASK as MASK};

pub const MAGIC: u32 = 0x3158_3149; // I1X1, distinct from readonly I1R1 and STOP I1S1.
pub const WORDS: usize = 192;
pub const READY_MARKER: u32 = 473;
pub const SUCCESS_MARKER: u32 = 475;
pub const FAILURE_MARKER: u32 = 595;
pub const PASS: u32 = 1;
const NO_HANDLER: u32 = 2;
const FAIL_PRESTATE: u32 = 0x501;
const FAIL_SETUP: u32 = 0x502;
const FAIL_WRAPPER: u32 = 0x503;
const FAIL_NO_START: u32 = 0x504;
const FAIL_ARM: u32 = 0x505;
const FAIL_START: u32 = 0x506;
const FAIL_SOURCE: u32 = 0x507;
const FAIL_SOURCE_STATE: u32 = 0x508;
const FAIL_ISR: u32 = 0x509;
const FAIL_CLEANUP: u32 = 0x50a;
const FAIL_RX_ONLY: u32 = 0x50b;
const FAIL_STOP_ONLY: u32 = 0x50c;
const FAIL_BYTE: u32 = 0x50d;
const FAIL_FATAL: u32 = 0x50e;
const FAIL_FIFO: u32 = 0x50f;
const RX: u32 = 4;
const STOP: u32 = 1 << 9;
const IRQ: u32 = 1 << 8;
const QUIET_US: u32 = 4_000;
const DEADLINE_US: u32 = 100_000;
const POLL_LIMIT: usize = 2_000_000;
const ACTIVE: u32 = 1;
const CONSUMED: u32 = 2;
const FAILED: u32 = 3;
const TIMEOUT: u32 = 4;
const EXPECTED: u32 = if cfg!(feature = "i2c1-read1-irq-expect-5a") { 0x5a } else { 0xa5 };

fn new_record() -> [u32; WORDS] {
    let mut words = [0; WORDS];
    words[..4].copy_from_slice(&[MAGIC, 1, FAIL_PRESTATE, 6]);
    words[128] = (WORDS * 4) as u32;
    words[129] = EXPECTED;
    words[130] = u32::MAX;
    words[131] = u32::MAX;
    for stage in 0..6 { words[8 + 18 * stage] = stage as u32; }
    words
}

fn identity(s: &[u32; 18], wrapper: u32) -> bool {
    s[1..5] == [wrapper, 0x001f_1fea, 0x3230_322a, 0x4457_0140]
}

fn route(s: &[u32], primask: u32, enabled: u32, active: u32, pending_allowed: u32) -> bool {
    primask <= 1 && s.len() == 8 && s[0] == 0x2000_0000 && s[1] == enabled
        && s[2] == 0 && s[3] & !pending_allowed == 0 && s[4] == 1 << 21
        && s[5] == active && s[6] == 0 && s[7] == primask
}

fn baseline(s: &[u32; 18], levels: [u32; 2], mask: u32, primask: u32) -> bool {
    identity(s, 0) && s[5..10] == [0, 0, mask, 0, 0] && levels == [0, 0]
        && route(&s[10..18], primask, 0, 0, 0)
}

// Snapshot layout: raw,masked,mask,abort,enable,STATUS,TXFLR,RXFLR,RX_TL.
fn fatal(s: &[u32; 9]) -> bool { (s[0] | s[1]) & FATAL != 0 || s[3] != 0 }

fn armed(s: &[u32; 18], read: &[u32; 9], primask: u32) -> bool {
    identity(s, 1) && route(&s[10..18], primask, 0, 0, 0)
        && s[5] & (STOP | RX | FATAL) == 0 && s[6..10] == [0, MASK, 0, 1]
        && !fatal(read) && read[0] & (STOP | RX) == 0
        && read[1..5] == [0, MASK, 0, 1] && read[6..9] == [0, 0, 0]
}

fn source(s: &[u32; 18], read: &[u32; 9], primask: u32) -> bool {
    identity(s, 1) && route(&s[10..18], primask, 0, 0, IRQ) && s[13] == IRQ
        && s[5] & (STOP | RX | FATAL) == STOP | RX && s[6..10] == [RX, MASK, 0, 1]
        && !fatal(read) && read[0] & (STOP | RX) == STOP | RX
        && read[1..5] == [RX, MASK, 0, 1] && read[6..9] == [0, 1, 0]
}

fn source_timeout(raw: u32) -> u32 {
    match raw & (RX | STOP) { RX => FAIL_RX_ONLY, STOP => FAIL_STOP_ONLY,
        0 => FAIL_SOURCE, _ => FAIL_SOURCE_STATE }
}

fn entry_ok(s: &[u32; 9], route_words: &[u32], ipsr: u32, count: u32, state: u32) -> bool {
    count == 0 && state == ACTIVE && ipsr == 24 && route(route_words, 0, 0, IRQ, IRQ)
        && !fatal(s) && s[0] & (RX | STOP) == RX | STOP
        && s[1..5] == [RX, MASK, 0, 1] && s[6..9] == [0, 1, 0]
}

fn postpop_ok(s: &[u32; 9]) -> bool {
    !fatal(s) && s[0] & (RX | STOP) == STOP && s[1..5] == [0, 0, 0, 1]
        && s[6..9] == [0, 0, 0]
}

fn delivery_decision(count: u32, state: u32, pops: u32, actual: u32, elapsed: u32, late: u32) -> u32 {
    if late != 0 { FAIL_ISR }
    else if count == 0 && state == TIMEOUT && elapsed >= DEADLINE_US { NO_HANDLER }
    else if count != 1 || state != CONSUMED || pops != 1 || elapsed >= DEADLINE_US { FAIL_ISR }
    else if actual != EXPECTED { FAIL_BYTE }
    else { PASS }
}

fn final_decision(primary: u32, cleanup_status: u32) -> u32 {
    if cleanup_status == 0x7ff { primary } else { FAIL_CLEANUP }
}

#[cfg(target_arch = "arm")]
mod hardware {
    use super::*;
    use crate::i2c1_wrapper_readonly_proof::capture;
    use core::sync::atomic::{AtomicU32, Ordering};
    use rp1_hal::{gpio::Pin, i2c::{self, I2c1, I2c1Host, I2c1Read1Snapshot}};

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static STATE: AtomicU32 = AtomicU32::new(0);
    static LATE: AtomicU32 = AtomicU32::new(0);
    // Entry9,postpop9,acked1,route8,popped1,popcount1,ISR reason1.
    static EVIDENCE: [AtomicU32; 30] = [const { AtomicU32::new(0) }; 30];

    fn pack(s: I2c1Read1Snapshot) -> [u32; 9] {
        [s.irq.raw_interrupt_status, s.irq.masked_interrupt_status, s.irq.interrupt_mask,
            s.irq.abort_source, s.irq.enable_status, s.status, s.tx_level, s.rx_level, s.rx_threshold]
    }

    fn save(offset: usize, values: &[u32]) {
        for (slot, value) in EVIDENCE[offset..].iter().zip(values) { slot.store(*value, Ordering::Relaxed); }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn I2C1_IRQHandler() {
        unsafe { rp1_rt::mask_i2c1_irq8_one_entry() };
        let entry = i2c::i2c1_read1_snapshot();
        let s = pack(entry);
        let r = rp1_rt::i2c1_irq_route_snapshot();
        let route_words = [r.vtor, r.iser0, r.iser1, r.ispr0, r.ispr1, r.iabr0, r.iabr1, r.primask];
        let ipsr: u32;
        unsafe { core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags)); }
        i2c::i2c1_mask_read1_irq();
        let count = COUNT.load(Ordering::Relaxed);
        let state = STATE.load(Ordering::Acquire);
        if count != 0 || state != ACTIVE {
            LATE.fetch_add(1, Ordering::Relaxed);
            i2c::i2c1_ack_read1_irq(entry);
            STATE.store(FAILED, Ordering::Release);
            COUNT.store(count.wrapping_add(1), Ordering::Release);
            return;
        }
        IPSR.store(ipsr, Ordering::Relaxed);
        save(0, &s);
        save(19, &route_words);
        let mut reason = if fatal(&s) { FAIL_FATAL } else if s[6..8] != [0, 1] { FAIL_FIFO } else { FAIL_ISR };
        let mut acknowledged = entry;
        if entry_ok(&s, &route_words, ipsr, count, state) {
            if let Some(value) = i2c::i2c1_read1_pop_one() {
                save(27, &[value, 1]);
                let post = i2c::i2c1_read1_snapshot();
                let p = pack(post);
                save(9, &p);
                reason = if fatal(&p) { FAIL_FATAL } else if postpop_ok(&p) { PASS } else { FAIL_FIFO };
                acknowledged.irq.raw_interrupt_status |= post.irq.raw_interrupt_status;
                acknowledged.irq.masked_interrupt_status |= post.irq.masked_interrupt_status;
                acknowledged.irq.abort_source |= post.irq.abort_source;
            } else { reason = FAIL_FIFO; }
        }
        save(18, &[i2c::i2c1_ack_read1_irq(acknowledged)]);
        save(29, &[reason]);
        STATE.store(if reason == PASS { CONSUMED } else { FAILED }, Ordering::Release);
        COUNT.store(1, Ordering::Release);
    }

    fn now() -> u32 { unsafe { core::ptr::read_volatile(0x400a_c028 as *const u32) } }

    fn quiet(mut valid: impl FnMut() -> bool) -> (bool, u32) {
        let start = now();
        for _ in 0..POLL_LIMIT {
            let ok = valid();
            let elapsed = now().wrapping_sub(start);
            if !ok || elapsed >= QUIET_US { return (ok, elapsed); }
            core::hint::spin_loop();
        }
        (false, now().wrapping_sub(start))
    }

    fn stage(words: &mut [u32; WORDS], id: usize) -> ([u32; 18], [u32; 9]) {
        let s = capture(id as u32);
        let read = pack(i2c::i2c1_read1_snapshot());
        words[8 + 18 * id..26 + 18 * id].copy_from_slice(&s);
        words[143 + 2 * id..145 + 2 * id].copy_from_slice(&read[6..8]);
        words[4] |= 1 << id;
        (s, read)
    }

    #[inline(never)]
    fn wrapper_or_once() -> bool {
        let address = 0x4007_4108 as *mut u32;
        unsafe {
            let before = core::ptr::read_volatile(address);
            if before != 0 { return false; }
            core::ptr::write_volatile(address, before | 1);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::read_volatile(address) == 1
        }
    }

    fn experiment(host: &mut I2c1Host, words: &mut [u32; WORDS]) -> u32 {
        let primask = words[5];
        let setup = host.snapshot(0, 0);
        words[138] = u32::from(setup.tx_fifo_depth);
        words[139] = u32::from(i2c::i2c1_rx_fifo_depth(setup.component_parameter));
        if words[138..140] != [32, 32] { return FAIL_FIFO; }
        words[127] |= 1 << 2;
        let wrapper_ok = wrapper_or_once();
        let (s, read) = stage(words, 2);
        if !wrapper_ok || !identity(&s, 1) || s[5..10] != [0; 5]
            || !route(&s[10..18], primask, 0, 0, 0) || read[6..8] != [0, 0] {
            return FAIL_WRAPPER;
        }
        let arm_ok = host.arm_read1_irq_preserving_causes(0x2d).is_ok();
        let (s, read) = stage(words, 2);
        let setup = host.snapshot(0, 0);
        words[140..143].copy_from_slice(&[setup.target, setup.control, read[8]]);
        if !arm_ok || !armed(&s, &read, primask) || words[140..143] != [0x2d, 0x63, 0] {
            return FAIL_ARM;
        }
        let (ok, elapsed) = quiet(|| {
            let (s, read) = stage(words, 2);
            armed(&s, &read, primask) && COUNT.load(Ordering::Acquire) == 0
        });
        words[119] = elapsed;
        if !ok || elapsed < QUIET_US { return FAIL_NO_START; }
        words[127] |= 1 << 7;
        STATE.store(ACTIVE, Ordering::Release);
        words[127] |= 1 << 3;
        match host.start_read1() {
            Ok(command) => {
                words[124] = command;
                words[132] = 1;
                words[127] |= 1 << 4;
                if command != I2C1_READ1_COMMAND { return FAIL_START; }
            }
            Err(_) => return FAIL_START,
        }
        let start = now();
        let mut asserted = false;
        for _ in 0..POLL_LIMIT {
            let (s, read) = stage(words, 3);
            words[155..157].copy_from_slice(&[read[5], read[8]]);
            words[120] = now().wrapping_sub(start);
            if fatal(&read) || s[5] & FATAL != 0 || s[8] != 0 { return FAIL_FATAL; }
            if read[6] > 1 || read[7] > 1 { return FAIL_FIFO; }
            if !identity(&s, 1) || !route(&s[10..18], primask, 0, 0, IRQ)
                || s[7] != MASK || s[9] != 1 || read[2] != MASK || read[4] != 1
                || read[8] != 0 || COUNT.load(Ordering::Acquire) != 0 { return FAIL_SOURCE_STATE; }
            if words[120] >= DEADLINE_US { break; }
            if source(&s, &read, primask) { asserted = true; break; }
            core::hint::spin_loop();
        }
        if !asserted { return source_timeout(words[8 + 18 * 3 + 5]); }
        words[127] |= 1 << 5;
        let start = now(); // Include enable/entry execution in the delivery deadline.
        unsafe { rp1_rt::enable_i2c1_irq8_after_source_asserted() };
        words[127] |= 1 << 6;
        for _ in 0..POLL_LIMIT {
            words[121] = now().wrapping_sub(start);
            if COUNT.load(Ordering::Acquire) != 0 || words[121] >= DEADLINE_US { break; }
            core::hint::spin_loop();
        }
        unsafe { rp1_rt::mask_i2c1_irq8_one_entry() };
        if COUNT.load(Ordering::Acquire) == 0 { STATE.store(TIMEOUT, Ordering::Release); }
        let (s, read) = stage(words, 4);
        if fatal(&read) || s[5] & FATAL != 0 || s[8] != 0 { return FAIL_FATAL; }
        let count = COUNT.load(Ordering::Acquire);
        if count != 0 && EVIDENCE[29].load(Ordering::Relaxed) != PASS {
            return EVIDENCE[29].load(Ordering::Relaxed);
        }
        if !identity(&s, 1) || !route(&s[10..18], 0, 0, 0, IRQ)
            || read[4] != 1 || (count != 0 && (read[0] & (STOP | RX) != 0 || read[1..4] != [0, 0, 0] || read[6..9] != [0, 0, 0])) {
            return FAIL_ISR;
        }
        delivery_decision(count, STATE.load(Ordering::Acquire), EVIDENCE[28].load(Ordering::Relaxed),
            EVIDENCE[27].load(Ordering::Relaxed) & 0xff, words[121], LATE.load(Ordering::Relaxed))
    }

    pub fn run(i2c: I2c1, sda: Pin<2>, scl: Pin<3>) -> u32 {
        let mut words = new_record();
        let (before, read) = stage(&mut words, 0);
        words[5] = before[17];
        if !baseline(&before, [read[6], read[7]], 0x48ff, 0) || fatal(&read) || read[0] != 0 {
            stage(&mut words, 5);
            words[125] = FAIL_PRESTATE;
            return publish(words);
        }
        let saved = match unsafe { rp1_rt::prepare_i2c1_irq8_one_entry() } {
            Some(saved) => saved,
            None => {
                stage(&mut words, 5);
                words[125] = FAIL_PRESTATE;
                return publish(words);
            }
        };
        words[127] |= 1;
        let setup = i2c.into_host_100khz(sda, scl);
        let (after, read) = stage(&mut words, 1);
        let mut primary = match setup {
            Ok(mut host) if baseline(&after, [read[6], read[7]], 0, saved.primask) && !fatal(&read) => {
                words[127] |= 1 << 1;
                experiment(&mut host, &mut words)
            }
            _ => FAIL_SETUP,
        };
        unsafe { rp1_rt::mask_i2c1_irq8_one_entry() };
        let count = COUNT.load(Ordering::Acquire);
        if STATE.load(Ordering::Acquire) == ACTIVE { STATE.store(TIMEOUT, Ordering::Release); }
        let (_, pre) = stage(&mut words, 4);
        if fatal(&pre) { primary = FAIL_FATAL; }
        if primary == PASS && (count != 1 || LATE.load(Ordering::Relaxed) != 0) { primary = FAIL_ISR; }
        words[125] = primary;
        words[126] = count;
        i2c::i2c1_mask_read1_irq();
        let disable_start = now();
        let disable_ok = i2c::i2c1_disable_read1_irq().is_ok();
        words[137] = now().wrapping_sub(disable_start);
        words[183] = u32::from(disable_ok);
        let before_ack = i2c::i2c1_read1_snapshot();
        words[173..182].copy_from_slice(&pack(before_ack));
        if fatal(&pack(before_ack)) { primary = FAIL_FATAL; }
        if primary == PASS && (before_ack.tx_level != 0 || before_ack.rx_level != 0) { primary = FAIL_FIFO; }
        if disable_ok && words[139] == 32 {
            words[182] = i2c::i2c1_read1_discard_residual(32);
            if words[182] != 0 && primary == PASS { primary = FAIL_FIFO; }
        }
        i2c::i2c1_ack_read1_irq(before_ack);
        // Any newly observed cause after this recorded acknowledgement stays
        // latched for the quiet/final failure evidence; never silently clear it.
        unsafe { rp1_rt::restore_i2c1_irq8_one_entry(saved) };
        let wrapper = u32::from(words[127] & (1 << 2) != 0);
        let (stable, elapsed) = quiet(|| {
            let (s, read) = stage(&mut words, 5);
            COUNT.load(Ordering::Acquire) == count && identity(&s, wrapper)
                && route(&s[10..18], saved.primask, 0, 0, 0) && s[5] & (STOP | RX | FATAL) == 0
                && s[6..10] == [0; 4] && !fatal(&read) && read[0] & (STOP | RX) == 0
                && read[1..5] == [0; 4] && read[6..8] == [0, 0]
        });
        words[122] = elapsed;
        let (s, read) = stage(&mut words, 5);
        let status = u32::from(disable_ok) | (u32::from(s[9] == 0) << 1)
            | (u32::from(s[7] == 0) << 2) | (u32::from(s[5] & (STOP | RX | FATAL) == 0) << 3)
            | (u32::from(s[6] == 0) << 4) | (u32::from(s[8] == 0) << 5)
            | (u32::from(route(&s[10..18], saved.primask, 0, 0, 0)) << 6)
            | (u32::from(stable && elapsed >= QUIET_US) << 7) | (u32::from(identity(&s, wrapper)) << 8)
            | (u32::from(read[6] == 0) << 9) | (u32::from(read[7] == 0) << 10);
        words[123] = status;
        words[125] = primary;
        words[136] = if status == 0x7ff { 0 } else { FAIL_CLEANUP };
        words[2] = final_decision(primary, status);
        words[6] = COUNT.load(Ordering::Acquire);
        words[7] = IPSR.load(Ordering::Relaxed);
        let evidence: [u32; 30] = core::array::from_fn(|i| EVIDENCE[i].load(Ordering::Relaxed));
        words[116..119].copy_from_slice(&[evidence[0], evidence[1], evidence[3]]);
        words[157..163].copy_from_slice(&[evidence[2], evidence[4], evidence[5], evidence[6], evidence[7], evidence[8]]);
        words[163..172].copy_from_slice(&evidence[9..18]);
        words[172] = evidence[18];
        words[184..192].copy_from_slice(&evidence[19..27]);
        if evidence[28] != 0 { words[130] = evidence[27] & 0xff; words[131] = evidence[27]; }
        words[133] = evidence[28];
        words[134] = STATE.load(Ordering::Acquire);
        words[135] = LATE.load(Ordering::Relaxed);
        publish(words)
    }

    #[inline(never)]
    fn publish(words: [u32; WORDS]) -> u32 {
        const _: () = assert!(WORDS * 4 == 768 && WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
        const _: () = assert!(rp1_hal::debug::MAILBOX_ADDR == 0x2000_fc00);
        let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
        unsafe {
            for (index, word) in words.iter().enumerate().skip(1) { core::ptr::write_volatile(out.add(index), *word); }
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
    fn read1_command_gate_error_precedence_timeouts_and_cleanup() {
        assert_eq!(MAGIC.to_le_bytes(), *b"I1X1");
        assert_eq!(WORDS * 4, 768);
        assert_eq!(new_record()[128..132], [768, EXPECTED, u32::MAX, u32::MAX]);
        assert_eq!(I2C1_READ1_COMMAND, 0x300);
        assert_eq!(MASK, 0x4f);
        let before = [0, 0, 0x001f_1fea, 0x3230_322a, 0x4457_0140, 0, 0, 0x48ff, 0, 0,
            0x2000_0000, 0, 0, 0, 1 << 21, 0, 0, 0];
        assert!(baseline(&before, [0, 0], 0x48ff, 0));
        assert!(!baseline(&before, [0, 32], 0x48ff, 0));
        let mut no_start = before;
        no_start[1] = 1;
        no_start[7] = MASK;
        no_start[9] = 1;
        let quiet_read = [0, 0, MASK, 0, 1, 6, 0, 0, 0];
        assert!(armed(&no_start, &quiet_read, 0));
        for index in [0, 1, 3, 6, 7, 8] {
            let mut dirty = quiet_read;
            dirty[index] = 1;
            assert!(!armed(&no_start, &dirty, 0), "dirty no-start {index}");
        }
        let mut stage = [3, 1, 0x001f_1fea, 0x3230_322a, 0x4457_0140, RX | STOP, RX, MASK, 0, 1,
            0x2000_0000, 0, 0, IRQ, 1 << 21, 0, 0, 0];
        let read = [RX | STOP, RX, MASK, 0, 1, 6, 0, 1, 0];
        assert!(source(&stage, &read, 0));
        for (raw, failure) in [(0, FAIL_SOURCE), (RX, FAIL_RX_ONLY), (STOP, FAIL_STOP_ONLY)] {
            let mut partial = read;
            partial[0] = raw;
            assert!(!source(&stage, &partial, 0));
            assert_eq!(source_timeout(raw), failure);
        }
        // Both arrival orders must join the same exact gate, never terminal early.
        for order in [[RX, STOP], [STOP, RX]] {
            let mut partial = read;
            partial[0] = order[0];
            assert!(!source(&stage, &partial, 0));
            partial[0] |= order[1];
            assert!(source(&stage, &partial, 0));
        }
        for field in [1, 2, 3, 4, 6, 7, 8] {
            let mut bad = read;
            bad[field] ^= 1;
            assert!(!source(&stage, &bad, 0), "read field {field}");
        }
        stage[13] = 0;
        assert!(!source(&stage, &read, 0));
        let route_words = [0x2000_0000, 0, 0, IRQ, 1 << 21, IRQ, 0, 0];
        assert!(entry_ok(&read, &route_words, 24, 0, ACTIVE));
        for (count, state) in [(1, ACTIVE), (0, TIMEOUT), (0, CONSUMED)] {
            assert!(!entry_ok(&read, &route_words, 24, count, state));
        }
        for level in [0, 2, 32, u32::MAX] {
            let mut bad = read;
            bad[7] = level;
            assert!(!entry_ok(&bad, &route_words, 24, 0, ACTIVE));
            assert!(!rp1_hal::i2c::i2c1_read1_pop_allowed(level));
        }
        let post = [STOP, 0, 0, 0, 1, 6, 0, 0, 0];
        assert!(postpop_ok(&post));
        for index in [1, 2, 3, 6, 7, 8] {
            let mut bad = post;
            bad[index] = 1;
            assert!(!postpop_ok(&bad));
        }
        for error in [1, 2, 8, 64] {
            let mut bad = read;
            bad[0] |= error;
            assert!(fatal(&bad));
            assert!(!entry_ok(&bad, &route_words, 24, 0, ACTIVE));
            let mut bad = post;
            bad[0] |= error;
            assert!(!postpop_ok(&bad));
        }
        assert_eq!(delivery_decision(1, CONSUMED, 1, EXPECTED, 1, 0), PASS);
        assert_eq!(delivery_decision(1, FAILED, 1, EXPECTED, 1, 0), FAIL_ISR);
        assert_eq!(delivery_decision(1, CONSUMED, 1, EXPECTED ^ 0xff, 1, 0), FAIL_BYTE);
        assert_eq!(delivery_decision(0, TIMEOUT, 0, 0, DEADLINE_US, 0), NO_HANDLER);
        assert_eq!(delivery_decision(1, TIMEOUT, 1, EXPECTED, DEADLINE_US, 1), FAIL_ISR);
        assert_eq!(delivery_decision(1, CONSUMED, 2, EXPECTED, 1, 0), FAIL_ISR);
        for bit in 0..11 { assert_eq!(final_decision(PASS, 0x7ff ^ (1 << bit)), FAIL_CLEANUP); }
        assert_eq!(final_decision(PASS, 0x7ff), PASS);
        assert_eq!(final_decision(FAIL_FATAL, 0x7ff), FAIL_FATAL);
    }
}

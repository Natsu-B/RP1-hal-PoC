//! One fresh warm-epoch UART0 owner; real USB-UART peer, two bounded exchanges.
//! No preparation, GPIO witness, recovery, or retry policy lives here.
//! Host check: rustc --edition=2024 --test warm_uart.rs -o /tmp/warm-uart-test
//! Telemetry: 124=AU01, 125=phase[7:0]/completed[9:8]/idle checks[31:16].
//! Receipts at 126/143: generation, received, IRQ entries, IPSR, higher-priority
//! wakes, RIS | MIS<<16, OR of all RX errors/overflow/residual, cleanup us,
//! quiet samples, final CR/IMSC/FR, then five big-endian payload/sentinel words.
//! Phase: 1=handoff, 2=owner, 3/5=exchange, 4/6=ACK, 7=ready, ff=failure
//! (failure code replaces idle checks). Completed bits imply checked canaries.

const FIRST: usize = 124;
const RECEIPT_FIRST: usize = FIRST + 2;
const RECEIPT_WORDS: usize = 17;
const LAST: usize = RECEIPT_FIRST + 2 * RECEIPT_WORDS - 1;
const BEFORE: u32 = 0x5aa5_a55a;
const AFTER: u32 = 0xa55a_5aa5;
const PAYLOADS: [&[u8; 19]; 2] = [b"HOST2RP1 IRQ 0001\r\n", b"HOST2RP1 IRQ 0002\r\n"];
const _: () = assert!(FIRST == 124 && LAST == 159);

#[repr(C)]
struct Buffer { before: u32, bytes: [u8; 20], after: u32 }

fn buffer_ok(generation: u32, b: &Buffer) -> bool {
    matches!(generation, 1 | 2) && b.before == BEFORE && b.after == AFTER
        && b.bytes[19] == 0xc3 && &b.bytes[..19] == PAYLOADS[generation as usize - 1]
}

fn receipt_ok(generation: u32, r: &[u32; RECEIPT_WORDS]) -> bool {
    matches!(generation, 1 | 2) && r[0] == generation && r[1] == 19
        && r[2] > 0 && r[2] <= 84 && r[3] == 41 && r[4] > 0 && r[4] <= r[2]
        && r[5] & 0x50 != 0 && (r[5] >> 16) & 0x50 != 0
        && (r[5] >> 16) & !0x50 == 0 && r[6] == 0
        && r[7] >= 8000 && r[8] >= 4
        && r[9] & 0x301 == 0x101 && r[10] == 0 && r[11] & 0x18 == 0x10
}

fn complete(completed: u32, active: u32, irqs: u32, expected_irqs: u32) -> bool {
    completed == 3 && active == 0 && expected_irqs != 0 && irqs == expected_irqs
}

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get, put};
    use core::{ffi::c_void, ptr};
    use rp1_freertos as os;
    use rp1_hal::uart::Uart0Tx;

    static mut HOST: Option<Uart0Tx> = None;
    static mut HOST_STATE: u32 = 0; // 0=never set, 1=published, 2=permanently taken.
    static mut IRQ_ENTRIES: u32 = 0; // Sole writer is the direct local IRQ25 ISR.
    static mut COMPLETED: u32 = 0;
    static mut READY_IRQS: u32 = 0; // Publish last; zero never means ready.

    fn fail(code: u32) -> ! {
        unsafe { ptr::addr_of_mut!(READY_IRQS).write_volatile(0); }
        put(FIRST + 1, (code << 16) | (get(FIRST + 1) & 0x300) | 0xff);
        panic!("warm UART contract failure");
    }
    fn check(ok: bool, code: u32) { if !ok { fail(code); } }
    fn irq_entries() -> u32 { unsafe { ptr::addr_of!(IRQ_ENTRIES).read_volatile() } }
    fn phase(value: u32) {
        put(FIRST + 1, value | (unsafe { ptr::addr_of!(COMPLETED).read_volatile() } << 8));
    }

    /// Called exactly once by proc0 warm preparation, before scheduler start.
    /// The caller has already established exclusive initialized UART0/pins and
    /// disabled IRQ25. The consumed state survives taking HOST until fresh BSS.
    pub fn set_host(host: Uart0Tx) {
        check(unsafe { ptr::addr_of!(HOST_STATE).read_volatile() } == 0, 1);
        unsafe {
            ptr::addr_of_mut!(HOST).write(Some(host));
            ptr::addr_of_mut!(HOST_STATE).write_volatile(1);
        }
        put(FIRST, u32::from_le_bytes(*b"AU01"));
        phase(1);
    }

    /// Proc0 monitor-task query. A later rearm or extra IRQ revokes readiness
    /// immediately, even before the owner's next periodic no-repeat check.
    pub fn ready() -> bool {
        let expected = unsafe { ptr::addr_of!(READY_IRQS).read_volatile() };
        expected != 0 && complete(
            unsafe { ptr::addr_of!(COMPLETED).read_volatile() },
            unsafe { os::uart0::active_generation() }, irq_entries(), expected)
    }

    fn store(generation: u32, r: os::uart0::Receipt, b: &Buffer) -> [u32; RECEIPT_WORDS] {
        let mut words = [r.generation, r.received, r.irq_entries, r.ipsr,
            r.higher_priority_wakes, r.first_ris | (r.first_mis << 16),
            r.rsr_errors | r.first_error_dr | r.overflow_bytes | r.residual_bytes,
            r.cleanup_elapsed_us, r.quiet_samples, r.final_cr, r.final_imsc,
            r.final_fr, 0, 0, 0, 0, 0];
        for (word, bytes) in words[12..].iter_mut().zip(b.bytes.chunks_exact(4)) {
            *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        let base = RECEIPT_FIRST + (generation as usize - 1) * RECEIPT_WORDS;
        for (i, word) in words.iter().enumerate() { put(base + i, *word); }
        words
    }

    /// Slot7, priority5, 512 words. Null argument, one owner, no allocation.
    /// All fallible exchanges/TX are bounded; failure uses the existing no-unwind
    /// panic halt. Notification0 is reserved solely for the existing adapter.
    pub unsafe extern "C" fn worker(arg: *mut c_void) {
        check(arg.is_null(), 2);
        let (ipsr, control, psp): (u32, u32, u32);
        unsafe { core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP",
            out(reg) ipsr, out(reg) control, out(reg) psp, options(nomem, nostack)); }
        check(ipsr == 0 && control & 3 == 2 && psp & 7 == 0, 3);
        check(unsafe { ptr::addr_of!(HOST_STATE).read_volatile() } == 1, 4);
        let Some(host) = (unsafe { ptr::addr_of_mut!(HOST).replace(None) }) else { fail(5); };
        unsafe { ptr::addr_of_mut!(HOST_STATE).write_volatile(2); }
        // run() clears reserved telemetry after pre-scheduler host handoff.
        put(FIRST, u32::from_le_bytes(*b"AU01"));
        phase(2);
        check(irq_entries() == 0 && unsafe { os::uart0::active_generation() } == 0, 6);
        let driver = unsafe { os::uart0::Driver::new(host) };
        let mut buffer = Buffer { before: BEFORE, bytes: [0xc3; 20], after: AFTER };
        let mut expected_irqs = 0;
        for (index, (prompt, ack)) in [
            (&b"RP1U0 RTOSREADY 0001\r\n"[..], &b"RP1U0 RTOSOK 0001\r\n"[..]),
            (&b"RP1U0 RTOSREADY 0002\r\n"[..], &b"RP1U0 RTOSOK 0002\r\n"[..]),
        ].into_iter().enumerate() {
            let generation = index as u32 + 1;
            unsafe { os::delay(100).unwrap(); }
            check(irq_entries() == expected_irqs && unsafe { os::uart0::active_generation() } == 0, 7);
            buffer.bytes.fill(0xc3);
            phase(generation * 2 + 1);
            // RX publication/arming precedes READY. Only real IRQ25 can finish.
            let result = unsafe { driver.exchange(prompt, &mut buffer.bytes[..19], 2000) };
            let snapshot = unsafe { ptr::addr_of!(buffer).read_volatile() };
            let Some(receipt) = driver.last_receipt() else { fail(8); };
            let words = store(generation, receipt, &snapshot);
            check(result.is_ok() && receipt_ok(generation, &words), 9);
            check(buffer_ok(generation, &snapshot), 10);
            expected_irqs += receipt.irq_entries;
            check(irq_entries() == expected_irqs && unsafe { os::uart0::active_generation() } == 0, 11);
            check(!unsafe { os::uart0::cancel(generation) }, 12);
            phase(generation * 2 + 2);
            check(unsafe { driver.write_all(ack, 100) }.is_ok(), 13);
            unsafe { ptr::addr_of_mut!(COMPLETED).write_volatile((1 << generation) - 1); }
        }
        check(complete(unsafe { ptr::addr_of!(COMPLETED).read_volatile() },
            unsafe { os::uart0::active_generation() }, irq_entries(), expected_irqs), 14);
        phase(7);
        unsafe { ptr::addr_of_mut!(READY_IRQS).write_volatile(expected_irqs); }
        let mut idle_checks = 0u32;
        loop {
            unsafe { os::delay(1000).unwrap(); }
            check(ready(), 15);
            check(driver.last_receipt().is_some_and(|r| r.generation == 2), 16);
            check(buffer_ok(2, &unsafe { ptr::addr_of!(buffer).read_volatile() }), 17);
            idle_checks = idle_checks.wrapping_add(1);
            put(FIRST + 1, (idle_checks << 16) | 0x307);
        }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn UART0_IRQHandler() {
        unsafe { ptr::addr_of_mut!(IRQ_ENTRIES).write_volatile(irq_entries().saturating_add(1));
            os::uart0::on_interrupt(); }
    }
}

#[cfg(target_arch = "arm")]
pub use target::{ready, set_host, worker};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_two_real_clean_receipts_fit_the_reserved_window() {
        assert_eq!((FIRST, RECEIPT_FIRST, LAST), (124, 126, 159));
        let good = [1, 19, 5, 41, 5, 0x0050_0050, 0, 9000, 12, 0x101, 0, 0x197,
            0, 0, 0, 0, 0];
        assert!(receipt_ok(1, &good));
        for (index, value) in [(0, 0), (1, 18), (2, 0), (2, 85), (3, 25),
            (4, 0), (4, 6), (5, 0x50), (5, 0x0050_0000), (5, 0x0051_0050),
            (6, 1), (7, 7999), (8, 3), (9, 0x301), (10, 0x50), (11, 0x18)] {
            let mut bad = good; bad[index] = value;
            assert!(!receipt_ok(1, &bad), "field {index}");
        }
        assert!(!receipt_ok(2, &good));
        let mut second = good; second[0] = 2;
        assert!(receipt_ok(2, &second));
        let mut b = Buffer { before: BEFORE, bytes: [0xc3; 20], after: AFTER };
        for generation in 1..=2 {
            b.bytes[..19].copy_from_slice(PAYLOADS[generation - 1]);
            assert!(buffer_ok(generation as u32, &b));
            assert!(!buffer_ok(3 - generation as u32, &b));
        }
        b.bytes[19] = 0; assert!(!buffer_ok(2, &b));
        b.bytes[19] = 0xc3; b.before ^= 1; assert!(!buffer_ok(2, &b));
        b.before = BEFORE; b.after ^= 1; assert!(!buffer_ok(2, &b));
        assert!(complete(3, 0, 10, 10));
        for (done, active, irqs, expected) in [(0, 0, 0, 0), (1, 0, 5, 5),
            (2, 0, 5, 5), (3, 3, 10, 10), (3, 0, 11, 10), (3, 0, 9, 10)] {
            assert!(!complete(done, active, irqs, expected));
        }
    }
}

#!/usr/bin/env python3
"""Compile actual UART ring/state and RTOS deadline code; no MMIO or serial."""
from pathlib import Path
import re
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
uart = (root / 'crates/rp1-freertos/src/uart0.rs').read_text()
state = uart.split('// BEGIN PURE UART RX STATE', 1)[1].split('\n', 1)[1]
state = state.split('// END PURE UART RX STATE', 1)[0]
lib = (root / 'crates/rp1-freertos/src/lib.rs').read_text()
deadline = re.search(r'pub fn deadline_remaining\([^{}]+\{[^{}]+\}', lib).group()
hal = (root / 'crates/rp1-hal/src/uart.rs').read_text()
status = re.search(r'pub struct Uart0RxStatus \{[^{}]+\}', hal).group()
clean = re.search(r'fn clean\(s: Uart0RxStatus\) -> bool \{[^{}]+\}', uart).group()
checks = r'''
#[test] fn ring_wrap_keeps_unread_bytes() {
    let mut ring = Ring::new();
    for n in 0..64 { assert!(ring.push(n)); }
    assert!(!ring.push(255));
    for n in 0..32 { assert_eq!(ring.pop(), Some(n)); }
    for n in 64..96 { assert!(ring.push(n)); }
    assert!(!ring.push(255));
    for n in 32..96 { assert_eq!(ring.pop(), Some(n)); }
    assert_eq!(ring.pop(), None);
}
#[test] fn receive_can_stream_more_than_ring_capacity() {
    let mut s = RxState::new(1, 160);
    for burst in 0..5 {
        assert!(s.begin_irq(1));
        let before = s.received;
        for n in 0..FIFO_BUDGET { assert!(s.accept(1, (burst * FIFO_BUDGET + n) as u32)); }
        s.progress(before);
        for n in 0..FIFO_BUDGET { assert_eq!(s.ring.pop(), Some((burst * FIFO_BUDGET + n) as u8)); }
    }
    assert_eq!(s.received, 160);
    assert_eq!(s.entries, 5);
    assert_eq!(s.outcome(false, Some(1)), Ok(true));
}
#[test] fn overflow_and_late_data_are_not_success() {
    let mut s = RxState::new(7, 128);
    for n in 0..65 { assert!(s.accept(7, n)); }
    assert_eq!(s.error, Some(Error::Overflow));
    assert_eq!(s.overflow_bytes, 1);
    for n in 0..64 { assert_eq!(s.ring.pop(), Some(n)); }
    let mut s = RxState::new(8, 1);
    assert!(s.accept(8, 0x41));
    assert_eq!(s.outcome(false, Some(1)), Ok(true));
    s.record_word(0x42); // Actual checked-cleanup path, after terminal IRQ.
    assert_eq!(s.outcome(false, Some(1)), Err(Error::UnexpectedData));
}
#[test] fn error_word_is_retained_and_beats_cancel_or_completion() {
    let mut s = RxState::new(1, 1);
    assert!(s.accept(1, 0xf41));
    assert_eq!(s.first_error_dr, 0xf41);
    assert_eq!(s.rsr_errors, 0xf);
    assert_eq!(s.ring.pop(), Some(0x41));
    assert_eq!(s.outcome(true, None), Err(Error::DataError));
    let mut late = RxState::new(2, 1);
    late.accept(2, 0x42);
    late.retain_rsr(8); // RSR observed during checked cleanup, before ECR.
    assert_eq!(late.outcome(false, Some(1)), Err(Error::DataError));
}
#[test] fn old_generation_cannot_touch_next_exchange() {
    assert_eq!(next_generation(0), 1);
    assert_eq!(next_generation(u32::MAX - 1), u32::MAX);
    assert!(!generation_matches(0, 0));
    let mut s = RxState::new(42, 1);
    assert!(!s.accept(41, 0xff));
    assert!(!s.begin_irq(41));
    assert!(!s.accept(0, 0xff));
    assert_eq!((s.received, s.entries), (0, 0));
    assert_eq!(s.ring.pop(), None);
    assert!(s.accept(42, 0x41));
    assert_eq!(s.outcome(true, Some(1)), Err(Error::Cancelled));
    assert_eq!(s.outcome(false, None), Err(Error::Timeout));
}
#[test] #[should_panic(expected = "generation exhausted")]
fn exhausted_generation_never_reuses_an_old_cancel_tag() { next_generation(u32::MAX); }
#[test] fn irq_entry_and_no_progress_budgets_are_finite() {
    let mut s = RxState::new(1, 100);
    for _ in 0..4 { assert!(s.begin_irq(1)); s.progress(0); }
    assert_eq!(s.error, Some(Error::IrqBudget));
    assert!(!s.begin_irq(1));
    let mut s = RxState::new(2, 1);
    for _ in 0..12 { assert!(s.begin_irq(2)); }
    assert!(!s.begin_irq(2));
    assert_eq!(s.error, Some(Error::IrqBudget));
}
#[test] fn wrapping_tick_and_raw_timer_math() {
    assert_eq!(deadline_remaining(u32::MAX - 5, 4), Some(10));
    assert_eq!(deadline_remaining(4, 4), None);
    assert_eq!(deadline_remaining(5, 4), None);
    assert_eq!(deadline_remaining(0, 0x8000_0000), None);
    assert_eq!(deadline_remaining(0, 0x7fff_ffff), Some(0x7fff_ffff));
    assert_eq!(4u32.wrapping_sub(u32::MAX - 5), 10);
}
#[test] fn cleanup_requires_rxe_off_empty_rx_no_errors_and_only_tx_enabled() {
    fn good() -> Uart0RxStatus { Uart0RxStatus { fr: 0x90, cr: 0x101, imsc: 0, ris: 0, mis: 0, rsr: 0 } }
    assert!(clean(good()));
    let mut s = good(); s.cr |= 0x200; assert!(!clean(s));
    let mut s = good(); s.cr &= !0x100; assert!(!clean(s));
    let mut s = good(); s.cr &= !1; assert!(!clean(s));
    let mut s = good(); s.fr &= !0x10; assert!(!clean(s));
    let mut s = good(); s.imsc = 0x50; assert!(!clean(s));
    let mut s = good(); s.ris = 0x40; assert!(!clean(s));
    let mut s = good(); s.mis = 1; assert!(!clean(s));
    let mut s = good(); s.rsr = 8; assert!(!clean(s));
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-uart-rtos-test-') as tmp:
    p = Path(tmp)
    (p / 'check.rs').write_text(state + deadline + status + clean + checks)
    subprocess.run(['rustc', '--edition=2021', '--test', str(p / 'check.rs'), '-o', str(p / 'check')], check=True)
    subprocess.run([str(p / 'check')], check=True)
print('STATIC actual UART64 ring, overflow/error, generation, IRQ budgets and wrap-time checks PASS; hardware separate')

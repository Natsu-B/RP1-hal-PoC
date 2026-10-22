//! Proc0 UART0/IRQ25: one owner, a 64-byte RX ring, notification0 per exchange.
//! RX is armed before prompt TX. No clock/reset/VTOR or global IRQ-mask writer.
use crate::{self as os, Task};
use core::{cell::{Cell, UnsafeCell}, ptr};
use rp1_hal::uart::{Uart0RxStatus, Uart0Tx};

const BIT: u32 = 1 << 25;
const ENABLE: *mut u32 = 0xe000_e100 as *mut u32;
const DISABLE: *mut u32 = 0xe000_e180 as *mut u32;
const PENDING: *mut u32 = 0xe000_e200 as *mut u32;
const CLEAR: *mut u32 = 0xe000_e280 as *mut u32;
const ACTIVE_IRQ: *mut u32 = 0xe000_e300 as *mut u32;
const PRIORITY: *mut u8 = 0xe000_e419 as *mut u8;
static mut ACTIVE: *mut Context = ptr::null_mut();
static mut GENERATION: u32 = 0;
static mut WAITER: u32 = 0;
static mut CANCEL: u32 = 0;

fn barrier() { unsafe { core::arch::asm!("dsb sy", "isb", options(nostack)); } }
fn mask() { unsafe { DISABLE.write_volatile(BIT); } barrier(); }
fn raw() -> u32 { unsafe { (0x400a_c028 as *const u32).read_volatile() } }
fn clean(s: Uart0RxStatus) -> bool {
    s.cr & 0x301 == 0x101 && s.imsc == 0 && s.fr & 0x10 != 0
        && s.rsr & 0xf == 0 && (s.ris | s.mis) & 0x50 == 0 && s.mis == 0
}

// BEGIN PURE UART RX STATE (compiled directly by tools/test-uart-rtos.py).
const RING_BYTES: usize = 64;
const FIFO_BUDGET: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { InvalidArgument, Cancelled, Timeout, DataError, Overflow, UnexpectedData, IrqBudget }

struct Ring { bytes: [u8; RING_BYTES], head: usize, len: usize }
impl Ring {
    const fn new() -> Self { Self { bytes: [0; RING_BYTES], head: 0, len: 0 } }
    fn push(&mut self, byte: u8) -> bool {
        if self.len == RING_BYTES { return false; }
        self.bytes[(self.head + self.len) % RING_BYTES] = byte;
        self.len += 1;
        true
    }
    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 { return None; }
        let byte = self.bytes[self.head];
        self.head = (self.head + 1) % RING_BYTES;
        self.len -= 1;
        Some(byte)
    }
}

fn generation_matches(active: u32, observed: u32) -> bool { active != 0 && active == observed }
// Never reuse a cancellation tag, even after the full 32-bit generation range.
fn next_generation(previous: u32) -> u32 {
    previous.checked_add(1).expect("UART generation exhausted; restart required")
}

struct RxState {
    ring: Ring, generation: u32, expected: u32, received: u32,
    entries: u32, no_progress: u32, error: Option<Error>,
    rsr_errors: u32, first_error_dr: u32, overflow_bytes: u32,
}
impl RxState {
    const fn new(generation: u32, expected: u32) -> Self {
        Self { ring: Ring::new(), generation, expected, received: 0, entries: 0,
            no_progress: 0, error: None, rsr_errors: 0, first_error_dr: 0, overflow_bytes: 0 }
    }
    fn fail(&mut self, error: Error) {
        if self.error.is_none() || error == Error::DataError { self.error = Some(error); }
    }
    fn retain_rsr(&mut self, rsr: u32) {
        self.rsr_errors |= rsr & 0xf;
        if rsr & 0xf != 0 { self.fail(Error::DataError); }
    }
    fn record_word(&mut self, word: u32) {
        if word & 0xf00 != 0 {
            if self.first_error_dr == 0 { self.first_error_dr = word; }
            self.retain_rsr(word >> 8);
        }
        self.received = self.received.saturating_add(1);
        if !self.ring.push(word as u8) {
            self.overflow_bytes = self.overflow_bytes.saturating_add(1);
            self.fail(Error::Overflow);
        }
        if self.received > self.expected { self.fail(Error::UnexpectedData); }
    }
    fn accept(&mut self, generation: u32, word: u32) -> bool {
        if !generation_matches(self.generation, generation) { return false; }
        self.record_word(word);
        true
    }
    fn begin_irq(&mut self, generation: u32) -> bool {
        if !generation_matches(self.generation, generation) { return false; }
        self.entries = self.entries.saturating_add(1);
        if self.terminal() || self.entries > self.expected.saturating_mul(4).saturating_add(8) {
            self.fail(Error::IrqBudget);
            return false;
        }
        true
    }
    fn progress(&mut self, before: u32) {
        self.no_progress = if before == self.received { self.no_progress + 1 } else { 0 };
        if self.no_progress >= 4 { self.fail(Error::IrqBudget); }
    }
    fn terminal(&self) -> bool { self.error.is_some() || self.received >= self.expected }
    fn outcome(&self, cancelled: bool, remaining: Option<u32>) -> Result<bool, Error> {
        if let Some(error) = self.error { return Err(error); }
        if cancelled { return Err(Error::Cancelled); }
        if remaining.is_none() { return Err(Error::Timeout); }
        Ok(self.received == self.expected)
    }
}
// END PURE UART RX STATE

#[derive(Clone, Copy, Debug, Default)]
pub struct Receipt {
    pub generation: u32, pub received: u32, pub irq_entries: u32, pub ipsr: u32,
    pub elapsed_us: u32, pub irq_body_max_us: u32, pub irq_end_to_task_us: u32,
    pub higher_priority_wakes: u32, pub first_ris: u32, pub first_mis: u32,
    pub rsr_errors: u32, pub first_error_dr: u32, pub overflow_bytes: u32,
    pub residual_bytes: u32,
    /// Sum of preflight and final checked cleanup; both sample >=4ms quiet.
    pub cleanup_elapsed_us: u32, pub quiet_samples: u32,
    pub final_cr: u32, pub final_imsc: u32, pub final_fr: u32,
}

struct Context {
    host: Uart0Tx, state: RxState, receipt: Receipt, irq_end: u32,
    terminal_generation: u32, wake_recorded: bool,
}
impl Context {
    fn observe_terminal(&mut self) {
        if self.terminal_generation == self.state.generation && !self.wake_recorded {
            self.receipt.irq_end_to_task_us = raw().wrapping_sub(self.irq_end);
            self.wake_recorded = true;
        }
    }
}

/// The ISR points only into UnsafeCell, never at the caller's buffer. All task
/// accesses to Context are IRQ25-masked, and end before unmasking or blocking.
/// Interruptible calls must use shared references, including outer callers:
/// UnsafeCell does not relax the uniqueness of an enclosing &mut Driver.
pub struct Driver { context: UnsafeCell<Context>, last: Cell<Option<Receipt>> }
impl Driver {
    /// # Safety
    /// Proc0, exclusive already-initialized UART0/pins/IRQ25 ownership. Install
    /// on_interrupt directly in vector41; keep IRQ25 disabled until this driver
    /// enables it. FreeRTOS uses three priority bits/MAX_SYSCALL logical5.
    pub unsafe fn new(host: Uart0Tx) -> Self {
        assert_eq!(unsafe { ENABLE.read_volatile() } & BIT, 0);
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        let s = host.rx_status();
        assert_eq!(s.cr & 0x101, 0x101);
        assert_eq!(s.imsc & !0x50, 0, "UART unexpected interrupt owner");
        unsafe { PRIORITY.write_volatile(0xc0); }
        barrier();
        assert_eq!(unsafe { PRIORITY.read_volatile() }, 0xc0);
        Self { context: UnsafeCell::new(Context { host, state: RxState::new(0, 0),
            receipt: Receipt::default(), irq_end: 0, terminal_generation: 0, wake_recorded: false }), last: Cell::new(None) }
    }

    pub fn last_receipt(&self) -> Option<Receipt> { self.last.get() }

    /// RXE off, <=32 pops per sample/<=64 total, retain errors before ECR/ICR,
    /// then >=4ms sampled quiet. An in-flight character can finish after RXE is
    /// cleared: it resets the quiet interval and cannot become silent success.
    unsafe fn cleanup(&self, preflight: bool) {
        mask();
        let started = raw();
        { unsafe { &mut *self.context.get() }.host.stop_rx_irq(); }
        let mut quiet_since = started;
        let mut previous = started;
        let mut drained = 0;
        for _ in 0..32 {
            let now = raw();
            assert!(now.wrapping_sub(started) < 20_000 && now.wrapping_sub(previous) < 10_000,
                "UART cleanup timing failed; recovery required");
            previous = now;
            let done = {
                let c = unsafe { &mut *self.context.get() };
                let before = c.host.rx_status();
                c.state.retain_rsr(before.rsr);
                let mut activity = before.rsr & 0xf != 0;
                for _ in 0..FIFO_BUDGET {
                    let Some(word) = c.host.try_read_word() else { break; };
                    activity = true;
                    drained += 1;
                    assert!(drained <= RING_BYTES, "UART disabled RX keeps filling; recovery required");
                    c.receipt.residual_bytes = c.receipt.residual_bytes.saturating_add(1);
                    c.state.record_word(word);
                    // Before arming, even a correctly sized stale response is not ours.
                    if preflight { c.state.fail(Error::UnexpectedData); }
                }
                let rsr = c.host.rx_status().rsr;
                c.state.retain_rsr(rsr);
                activity |= rsr & 0xf != 0;
                c.host.clear_rx_errors();
                c.host.ack_rx_irq();
                let s = c.host.rx_status();
                assert!(clean(s), "UART RX cleanup failed; recovery required");
                assert_eq!(unsafe { ENABLE.read_volatile() | ACTIVE_IRQ.read_volatile() } & BIT, 0);
                // Source is disabled, drained and acknowledged before NVIC ACK.
                unsafe { CLEAR.write_volatile(BIT); }
                barrier();
                assert_eq!(unsafe { PENDING.read_volatile() } & BIT, 0);
                c.receipt.quiet_samples += 1;
                c.receipt.final_cr = s.cr;
                c.receipt.final_imsc = s.imsc;
                c.receipt.final_fr = s.fr;
                if activity { quiet_since = now; }
                let done = now.wrapping_sub(quiet_since) >= 4000;
                if done { assert_eq!(s.fr & 8, 0, "UART TX still busy after checked cleanup"); }
                done
            };
            if done {
                unsafe { &mut *self.context.get() }.receipt.cleanup_elapsed_us += raw().wrapping_sub(started);
                return;
            }
            unsafe { os::delay(1).unwrap(); }
        }
        panic!("UART cleanup sample budget exhausted; recovery required");
    }

    /// Enable only a live, nonterminal request. No Context reference survives it.
    unsafe fn resume_irq(&self) {
        let enable = {
            let c = unsafe { &*self.context.get() };
            generation_matches(c.state.generation, unsafe { ptr::addr_of!(GENERATION).read_volatile() })
                && !c.state.terminal()
        };
        if enable { unsafe { ENABLE.write_volatile(BIT); } barrier(); }
    }

    /// FIFO-full and serial-BUSY waits yield a tick, never busy-spin. RX can run
    /// while sending a prompt, but its ring remains bounded and overflow is fatal.
    unsafe fn transmit(&self, bytes: &[u8], deadline: u32, generation: u32) -> Result<(), Error> {
        let mut sent = 0;
        loop {
            mask();
            let left = os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline);
            let (done, progressed) = {
                let c = unsafe { &mut *self.context.get() };
                if generation != 0 {
                    c.observe_terminal();
                    c.state.outcome(unsafe { ptr::addr_of!(CANCEL).read_volatile() } == generation, left)?;
                } else if left.is_none() { return Err(Error::Timeout); }
                if sent == bytes.len() { (c.host.rx_status().fr & 8 == 0, false) }
                else if c.host.try_write_byte(bytes[sent]) { sent += 1; (false, true) }
                else { (false, false) }
            };
            unsafe { self.resume_irq(); }
            if done { return Ok(()); }
            if !progressed { unsafe { os::delay(1).unwrap(); } }
        }
    }

    /// # Safety
    /// Running proc0 owner task, notification0 reserved throughout the exchange.
    /// No enclosing exclusive Driver borrow or concurrent/reentrant driver call.
    /// `rx` is never ISR-owned. Prefix copies happen under IRQ25 mask; it cannot
    /// return until checked cleanup. Success also requires TX serial BUSY idle.
    #[inline(never)]
    pub unsafe fn exchange(&self, prompt: &[u8], rx: &mut [u8], timeout_ticks: u32)
        -> Result<Receipt, Error>
    {
        self.last.set(None);
        if rx.is_empty() || rx.len() > u32::MAX as usize || timeout_ticks == 0 || timeout_ticks >= 0x8000_0000 {
            return Err(Error::InvalidArgument);
        }
        mask();
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        let started = raw();
        let deadline = unsafe { os::tick().unwrap() }.wrapping_add(timeout_ticks);
        let waiter = unsafe { os::current_task().unwrap().unwrap() };
        unsafe { os::notification_take(true, 0).unwrap(); }
        let generation = {
            let c = unsafe { &mut *self.context.get() };
            let generation = next_generation(c.state.generation);
            c.state = RxState::new(generation, rx.len() as u32);
            c.receipt = Receipt { generation, ..Receipt::default() };
            c.irq_end = 0; c.terminal_generation = 0; c.wake_recorded = false;
            generation
        };
        let mut copied = 0;
        let mut result = (|| {
            unsafe { self.cleanup(true); }
            {
                let c = unsafe { &mut *self.context.get() };
                if let Some(error) = c.state.error { return Err(error); }
                if os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline).is_none() {
                    return Err(Error::Timeout);
                }
                assert!(clean(c.host.rx_status()));
                assert!(c.host.arm_rx_irq(), "UART RX arm failed; recovery required");
            }
            unsafe {
                ptr::addr_of_mut!(CANCEL).write_volatile(0);
                ptr::addr_of_mut!(WAITER).write_volatile(waiter.id());
                ptr::addr_of_mut!(ACTIVE).write_volatile(self.context.get());
                // Commit last, so cancellation never sees a new generation with
                // the old/null waiter. Source is already armed before prompt TX.
                ptr::addr_of_mut!(GENERATION).write_volatile(generation);
            }
            unsafe { self.resume_irq(); self.transmit(prompt, deadline, generation)?; }
            loop {
                mask();
                let left = os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline);
                let done = {
                    let c = unsafe { &mut *self.context.get() };
                    c.observe_terminal();
                    while copied < rx.len() {
                        let Some(byte) = c.state.ring.pop() else { break; };
                        rx[copied] = byte; copied += 1;
                    }
                    let done = c.state.outcome(unsafe { ptr::addr_of!(CANCEL).read_volatile() } == generation, left)?;
                    if done {
                        assert_eq!(c.terminal_generation, generation, "UART completion without terminal IRQ");
                    }
                    done
                };
                if done { return Ok(()); }
                unsafe { self.resume_irq(); os::notification_take(true, left.unwrap()).unwrap(); }
            }
        })();
        mask();
        unsafe {
            // Withdraw first, before cleanup blocks. Old IRQ/cancel cannot notify
            // this task after the reserved request channel has been released.
            ptr::addr_of_mut!(GENERATION).write_volatile(0);
            ptr::addr_of_mut!(ACTIVE).write_volatile(ptr::null_mut());
            ptr::addr_of_mut!(WAITER).write_volatile(0);
        }
        // The canceller's C critical transaction either committed before the
        // generation withdrawal or observes zero and rejects. Check AFTER that
        // boundary, closing the last-completion-to-withdrawal preemption window.
        if result.is_ok() && unsafe { ptr::addr_of!(CANCEL).read_volatile() } == generation {
            result = Err(Error::Cancelled);
        }
        barrier();
        unsafe { os::notification_take(true, 0).unwrap(); self.cleanup(false); }
        unsafe { os::notification_take(true, 0).unwrap(); }
        let c = unsafe { &mut *self.context.get() };
        // Error evidence observed during cleanup outranks a completed payload or
        // cancellation; ECR/ICR cannot erase it into a successful receipt.
        if let Some(error) = c.state.error { result = Err(error); }
        while copied < rx.len() {
            let Some(byte) = c.state.ring.pop() else { break; };
            rx[copied] = byte; copied += 1;
        }
        c.receipt.received = copied as u32;
        c.receipt.irq_entries = c.state.entries;
        c.receipt.rsr_errors = c.state.rsr_errors;
        c.receipt.first_error_dr = c.state.first_error_dr;
        c.receipt.overflow_bytes = c.state.overflow_bytes;
        c.receipt.elapsed_us = raw().wrapping_sub(started);
        self.last.set(Some(c.receipt));
        result.map(|()| c.receipt)
    }

    /// Bounded task TX, also waiting for serial idle. Does not reserve a new
    /// generation or replace last_receipt. RX stays disabled outside exchange.
    /// # Safety
    /// Running proc0 owner task; no outstanding exchange or other UART writer.
    /// No enclosing exclusive Driver borrow or concurrent/reentrant driver call.
    pub unsafe fn write_all(&self, bytes: &[u8], timeout_ticks: u32) -> Result<(), Error> {
        if timeout_ticks == 0 || timeout_ticks >= 0x8000_0000 { return Err(Error::InvalidArgument); }
        mask();
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        let s = unsafe { &*self.context.get() }.host.rx_status();
        assert!(clean(s), "UART write_all requires a checked exchange cleanup");
        let deadline = unsafe { os::tick().unwrap() }.wrapping_add(timeout_ticks);
        let result = unsafe { self.transmit(bytes, deadline, 0) };
        if result.is_err() { unsafe { self.cleanup(false); } }
        result
    }
}

/// # Safety
/// Running proc0 task only. Zero means no published exchange.
pub unsafe fn active_generation() -> u32 { unsafe { ptr::addr_of!(GENERATION).read_volatile() } }

/// # Safety
/// Proc0 task context; the owner's notification0 is reserved by this adapter.
pub unsafe fn cancel(generation: u32) -> bool {
    os::value(unsafe { os::ffi::rp1_freertos_cancel_notification(generation,
        ptr::addr_of!(GENERATION), ptr::addr_of_mut!(CANCEL), ptr::addr_of!(WAITER)) })
        .expect("UART cancellation transaction failed") == 1
}

/// # Safety
/// Direct proc0 local IRQ25/IPSR41 at logical6 only. No foreground polling call.
pub unsafe fn on_interrupt() {
    let started = raw(); let ipsr: u32;
    unsafe { core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack)); }
    assert_eq!(ipsr, 41);
    let p = unsafe { ptr::addr_of!(ACTIVE).read_volatile() };
    let generation = unsafe { ptr::addr_of!(GENERATION).read_volatile() };
    if p.is_null() || generation == 0 { mask(); return; }
    let c = unsafe { &mut *p };
    if !generation_matches(c.state.generation, generation) { mask(); return; }
    let s = c.host.rx_status();
    if c.state.entries == 0 {
        c.receipt.ipsr = ipsr; c.receipt.first_ris = s.ris; c.receipt.first_mis = s.mis;
    }
    c.state.retain_rsr(s.rsr);
    let before = c.state.received;
    if c.state.begin_irq(generation) {
        // An IRQ with no RX/RT source cannot consume unrelated FIFO traffic.
        if s.mis & 0x50 != 0 {
            for _ in 0..FIFO_BUDGET {
                let Some(word) = c.host.try_read_word() else { break; };
                assert!(c.state.accept(generation, word));
            }
            c.state.retain_rsr(c.host.rx_status().rsr);
            c.host.ack_rx_irq();
        }
        c.state.progress(before);
    }
    let terminal = c.state.terminal();
    if terminal {
        c.host.mask_rx_irq(); mask(); c.terminal_generation = generation;
    }
    // Wake for data as well as terminal state, so the task can drain the real
    // producer/consumer ring before the next burst. ISR never sees caller rx.
    if terminal || c.state.received != before {
        let waiter = unsafe { ptr::addr_of!(WAITER).read_volatile() };
        if waiter != 0 && unsafe { Task(waiter).notification_give_from_isr_woken().unwrap() } {
            c.receipt.higher_priority_wakes += 1;
        }
    }
    c.irq_end = raw();
    c.receipt.irq_body_max_us = c.receipt.irq_body_max_us.max(c.irq_end.wrapping_sub(started));
}

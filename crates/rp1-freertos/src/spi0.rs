//! Proc0 SPI0 IRQ19 adapter. One owner, one FIFO, one outstanding request.
//! Uses the caller task's default notification exclusively while receiving.
//! No clock/reset/VTOR writer, allocation, or proc1 lock. Cancellation uses the
//! kernel's task critical section to serialize its short nonblocking transaction.
use crate::{self as os, Task};
use core::{cell::UnsafeCell, ptr};
use rp1_hal::spi::{Spi0Host, Spi0IrqTransfer, Spi0RxError, Spi0RxState};

const BIT: u32 = 1 << 19;
const ENABLE: *mut u32 = 0xe000_e100 as *mut u32;
const DISABLE: *mut u32 = 0xe000_e180 as *mut u32;
const PENDING: *mut u32 = 0xe000_e280 as *mut u32;
const PRIORITY: *mut u8 = 0xe000_e413 as *mut u8;
static mut ACTIVE: *mut Spi0IrqTransfer<'static> = ptr::null_mut();
static mut WAITER: u32 = 0;
static mut GENERATION: u32 = 0;
static mut CANCEL: u32 = 0;
static mut IRQ_COUNT: u32 = 0;
static mut IRQ_LIMIT: u32 = 0;
static mut IRQ_ERROR: u32 = 0;
static mut IRQ_END: u32 = 0;
static mut IRQ_GENERATION: u32 = 0;
static mut IRQ_MAX_US: u32 = 0;

fn barrier() { unsafe { core::arch::asm!("dsb sy", "isb", options(nostack)); } }
fn mask() { unsafe { DISABLE.write_volatile(BIT); } barrier(); }
fn raw() -> u32 { unsafe { (0x400a_c028 as *const u32).read_volatile() } }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { InvalidDeadline, Cancelled, Timeout, IrqBudget, Receive(Spi0RxError) }

#[derive(Clone, Copy, Debug)]
pub struct Receipt {
    pub generation: u32,
    pub irq_entries: u32,
    pub elapsed_us: u32,
    pub irq_body_max_us: u32,
    /// Zero when no RxComplete-to-task observation was made (timeout/cancel).
    pub irq_end_to_task_us: u32,
}

/// Owns the HAL host/pins; do not create another SPI0 owner or call from proc1.
pub struct Driver { host: Spi0Host, generation: u32, last: Option<Receipt> }

impl Driver {
    /// # Safety
    /// Proc0, exclusive SPI0/IRQ19 ownership. Install `on_interrupt` in vector35,
    /// keep route19 disabled before this call. Only this adapter may enable it.
    /// FreeRTOS priority grouping/three-bit configuration must already be set.
    pub unsafe fn new(host: Spi0Host) -> Self {
        assert_eq!(unsafe { ENABLE.read_volatile() } & BIT, 0);
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        unsafe { PRIORITY.write_volatile(0xc0); PENDING.write_volatile(BIT); }
        barrier();
        assert_eq!(unsafe { PRIORITY.read_volatile() }, 0xc0);
        Self { host, generation: 0, last: None }
    }

    /// Last request's counters, including checked-aborted requests. No MMIO.
    pub fn last_receipt(&self) -> Option<Receipt> { self.last }

    /// Receive with bounded FIFO servicing and task blocking until IRQ/deadline.
    /// Successful return means serial idle AND checked local cleanup, not only
    /// RxComplete. Buffer cannot outlive this borrow until IRQ is masked, active
    /// pointer withdrawn, and peripheral cleanup verified. Cleanup failure halts
    /// through the configured panic hook; it never releases an unsafe buffer.
    /// # Safety
    /// Running proc0 task, no competing use of its notification. Same ownership
    /// contract as `new`; no ISR or other task may access `host` or `rx` directly.
    #[inline(never)]
    pub unsafe fn receive(&mut self, tx: &[u8], rx: &mut [u8], timeout_ticks: u32)
        -> Result<Receipt, Error>
    {
        self.last = None;
        if timeout_ticks == 0 || timeout_ticks >= 0x8000_0000 { return Err(Error::InvalidDeadline); }
        // Never recycle an old cancellation ticket. Reject exhaustion before MMIO.
        let generation = self.generation.checked_add(1).expect("SPI generation exhausted; restart required");
        let deadline = unsafe { os::tick().unwrap() }.wrapping_add(timeout_ticks);
        let started = raw();
        mask();
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        let waiter = unsafe { os::current_task().unwrap().unwrap() };
        unsafe { os::notification_take(true, 0).unwrap(); }
        let transfer = UnsafeCell::new(self.host.prepare_irq_transfer(tx, rx).map_err(|e| {
            // HAL preparation has no checked-abort handle on Err. Only errors
            // known to precede MMIO can safely return the host/buffer here.
            // Post-MMIO setup failures halt with IRQ masked and ACTIVE absent;
            // do not mistake the HAL's best-effort Drop for checked recovery.
            assert!(matches!(e, Spi0RxError::LengthMismatch { .. }
                | Spi0RxError::Setup(rp1_hal::spi::Spi0Error::EmptyPayload
                    | rp1_hal::spi::Spi0Error::PayloadTooLong { .. }
                    | rp1_hal::spi::Spi0Error::FifoDepthUnknown)),
                "SPI preparation failed after possible MMIO; recovery required");
            Error::Receive(e)
        })?);
        self.generation = generation;
        unsafe {
            ptr::addr_of_mut!(WAITER).write_volatile(waiter.0);
            ptr::addr_of_mut!(CANCEL).write_volatile(0);
            ptr::addr_of_mut!(IRQ_COUNT).write_volatile(0);
            ptr::addr_of_mut!(IRQ_ERROR).write_volatile(0);
            ptr::addr_of_mut!(IRQ_END).write_volatile(0);
            ptr::addr_of_mut!(IRQ_GENERATION).write_volatile(0);
            ptr::addr_of_mut!(IRQ_MAX_US).write_volatile(0);
            ptr::addr_of_mut!(IRQ_LIMIT).write_volatile(tx.len() as u32 + 1);
            // Erase only the raw pointer's lifetime, never manufacture a static
            // reference. It is withdrawn under IRQ19 mask before `transfer` dies.
            ptr::addr_of_mut!(ACTIVE).write_volatile(transfer.get().cast());
            ptr::addr_of_mut!(GENERATION).write_volatile(generation);
            PENDING.write_volatile(BIT);
        }
        let mut wake_us = None;
        let mut result = (|| {
            if os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline).is_none() {
                return Err(Error::Timeout);
            }
            {
                let t = unsafe { &mut *transfer.get() };
                unsafe { t.enable_local_irq_route() }.map_err(Error::Receive)?;
                t.start().map_err(Error::Receive)?;
            }
            loop {
                // Capture the shared UnsafeCell, not &mut transfer across IRQs.
                // Each inner borrow ends before IRQ enable/notification wait.
                if unsafe { ptr::addr_of!(CANCEL).read_volatile() } == generation { return Err(Error::Cancelled); }
                if unsafe { ptr::addr_of!(IRQ_ERROR).read_volatile() } != 0 { return Err(Error::IrqBudget); }
                let state = unsafe { &*transfer.get() }.state();
                if let Spi0RxState::Failed(e) = state { return Err(Error::Receive(e)); }
                let remaining = os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline)
                    .ok_or(Error::Timeout)?;
                if state == Spi0RxState::RxComplete {
                    assert_eq!(unsafe { ptr::addr_of!(IRQ_GENERATION).read_volatile() }, generation);
                    if wake_us.is_none() { wake_us = Some(raw()); }
                    if unsafe { &mut *transfer.get() }.try_finish().map_err(Error::Receive)? { return Ok(()); }
                    // No useful RX IRQ remains for the last serial edge. One tick
                    // delay avoids busy polling; keep the source masked throughout.
                    unsafe { os::delay(1).unwrap(); }
                } else {
                    unsafe { ENABLE.write_volatile(BIT); } barrier();
                    unsafe { os::notification_take(true, remaining).unwrap(); }
                    mask();
                }
            }
        })();
        mask();
        if result.is_err() { unsafe { &mut *transfer.get() }.abort().expect("SPI checked abort failed; buffer retained by halt"); }
        assert!(matches!(unsafe { &*transfer.get() }.state(), Spi0RxState::Complete | Spi0RxState::Failed(_)));
        // Opt-in acceptance image only: let a real task cancel after checked
        // completion but before withdrawal. No probe/callback in normal builds.
        #[cfg(feature = "spi0-cancel-window-probe")]
        if result.is_ok() && generation == 3 {
            unsafe extern "C" { fn rp1_spi_cancel_window_probe(generation: u32); }
            unsafe { rp1_spi_cancel_window_probe(generation); }
        }
        let receipt = Receipt {
            generation,
            irq_entries: unsafe { ptr::addr_of!(IRQ_COUNT).read_volatile() },
            elapsed_us: raw().wrapping_sub(started),
            irq_body_max_us: unsafe { ptr::addr_of!(IRQ_MAX_US).read_volatile() },
            irq_end_to_task_us: wake_us.map_or(0, |t| t.wrapping_sub(unsafe { ptr::addr_of!(IRQ_END).read_volatile() })),
        };
        unsafe {
            ptr::addr_of_mut!(GENERATION).write_volatile(0);
            ptr::addr_of_mut!(ACTIVE).write_volatile(ptr::null_mut());
            ptr::addr_of_mut!(WAITER).write_volatile(0);
            PENDING.write_volatile(BIT);
        }
        // Like UART: a task canceller either committed before withdrawal or
        // observes zero. Do not return success for an accepted late cancel.
        if result.is_ok() && unsafe { ptr::addr_of!(CANCEL).read_volatile() } == generation {
            result = Err(Error::Cancelled);
        }
        barrier();
        // No ISR or canceller can publish after withdrawal. Do not let a late
        // completion/cancellation count escape this receive's reserved channel.
        unsafe { os::notification_take(true, 0).unwrap(); }
        self.last = Some(receipt);
        result.map(|()| receipt)
    }
}

/// Current active generation (zero means idle). Cancellation is request-based;
/// only the owner receives completion after masked checked cleanup.
/// # Safety
/// Running proc0 task only, never proc1 or another interrupt namespace.
pub unsafe fn active_generation() -> u32 { unsafe { ptr::addr_of!(GENERATION).read_volatile() } }

/// # Safety
/// Proc0 task context. The owner task notification is reserved to this adapter.
pub unsafe fn cancel(generation: u32) -> bool {
    os::value(unsafe { os::ffi::rp1_freertos_cancel_notification(generation,
        ptr::addr_of!(GENERATION), ptr::addr_of_mut!(CANCEL), ptr::addr_of!(WAITER)) })
        .expect("SPI cancellation transaction failed") == 1
}

/// # Safety
/// Call only directly from proc0 local SPI0 IRQ19/IPSR35 at logical priority6.
/// Only the selected peripheral source is serviced; never call from a poll loop.
pub unsafe fn on_interrupt() {
    let started = raw();
    let ipsr: u32;
    unsafe { core::arch::asm!("mrs {0}, IPSR", out(reg) ipsr, options(nomem, nostack)); }
    assert_eq!(ipsr, 35);
    let transfer = unsafe { ptr::addr_of!(ACTIVE).read_volatile() };
    if transfer.is_null() { mask(); return; }
    let count = unsafe { ptr::addr_of!(IRQ_COUNT).read_volatile() }.saturating_add(1);
    unsafe { ptr::addr_of_mut!(IRQ_COUNT).write_volatile(count); }
    let terminal = if count > unsafe { ptr::addr_of!(IRQ_LIMIT).read_volatile() } {
        mask();
        unsafe { ptr::addr_of_mut!(IRQ_ERROR).write_volatile(1); }
        true
    } else {
        (unsafe { (*transfer).on_interrupt() }) != Spi0RxState::Active
    };
    if terminal {
        mask();
        unsafe { ptr::addr_of_mut!(IRQ_GENERATION).write_volatile(ptr::addr_of!(GENERATION).read_volatile()); }
        let waiter = unsafe { ptr::addr_of!(WAITER).read_volatile() };
        if waiter != 0 {
            unsafe { Task(waiter).notification_give_from_isr().unwrap(); }
        }
    }
    let ended = raw();
    unsafe {
        ptr::addr_of_mut!(IRQ_END).write_volatile(ended);
        let old = ptr::addr_of!(IRQ_MAX_US).read_volatile();
        ptr::addr_of_mut!(IRQ_MAX_US).write_volatile(old.max(ended.wrapping_sub(started)));
    }
}

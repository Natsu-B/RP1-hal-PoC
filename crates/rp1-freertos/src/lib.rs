#![no_std]
//! Proc0-only, static-lifetime FreeRTOS V11.3.1 bridge. No allocator, deletion, or SMP.
//!
//! # Safety contract
//! Every unsafe operation must run on proc0, never proc1. Creation/start are boot-thread
//! operations before scheduling; other operations require a running task unless named
//! `from_isr`. ISR APIs require an external IRQ at logical priority 5..=7 (NVIC 0xa0..=0xe0)
//! and perform the required PendSV yield themselves. Do not call task APIs from telemetry
//! hooks, critical sections, or while the scheduler is suspended. No callback may unwind.
//! Task arguments and anything reachable through them must outlive the scheduler, with
//! cross-task/IRQ mutable access synchronized by the caller. Copy handles never own data.

use core::ffi::{CStr, c_char, c_void};

#[cfg(feature = "critical-timing")]
pub mod critical_timing;

#[cfg(all(feature = "spi0-irq", target_arch = "arm"))]
pub mod spi0;

#[cfg(all(feature = "i2c1-irq", target_arch = "arm"))]
pub mod i2c1;

#[cfg(all(feature = "uart0-irq", target_arch = "arm"))]
pub mod uart0;

pub const TASK_SLOTS: u32 = 8;
pub const MIN_STACK_WORDS: u32 = 128;
pub const MAX_STACK_WORDS: u32 = 512;
/// Fixed backing pool; requested task stacks (rounded to even words) share this
/// budget at boot. Exhaustion returns Unavailable; idle has its separate stack.
pub const TOTAL_TASK_STACK_WORDS: u32 = if cfg!(feature = "task-pool-2304") { 2304 } else { 2560 };
pub const QUEUE_SLOTS: u32 = 4;
pub const MAX_QUEUE_WORDS: u32 = 16;
pub const SEMAPHORE_SLOTS: u32 = 4;
pub const PRIORITIES: u32 = 8;
pub const TICK_HZ: u32 = 1000;
pub const WAIT_FOREVER: u32 = u32::MAX;
pub const IDLE_TASK_ID: u32 = u32::MAX;
pub const KERNEL_COMMIT: &str = "3a22924e0a9ddbbc8b0758881c33b3422a5cc20d";

/// Deliberate halt through the official kernel's configASSERT. Test feature only.
/// # Safety
/// Running proc0 task on a recoverable test board; this never returns.
#[cfg(feature = "assert-probe")]
pub unsafe fn trigger_config_assert() -> ! { unsafe { ffi::rp1_freertos_config_assert_probe() } }

pub type TaskEntry = unsafe extern "C" fn(*mut c_void);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidArgument,
    WrongState,
    SlotInUse,
    Unavailable,
    WrongContext,
}

fn value(status: i32) -> Result<u32, Error> {
    match status {
        -1 => Err(Error::InvalidArgument),
        -2 => Err(Error::WrongState),
        -3 => Err(Error::SlotInUse),
        -5 => Err(Error::WrongContext),
        n if n < 0 => Err(Error::Unavailable),
        n => Ok(n as u32),
    }
}

fn task_parameters(slot: u32, name: &CStr, priority: u32, words: u32) -> Result<(), Error> {
    if slot >= TASK_SLOTS
        || name.to_bytes().is_empty()
        || name.to_bytes().len() >= 16
        || priority >= PRIORITIES
        || !(MIN_STACK_WORDS..=MAX_STACK_WORDS).contains(&words)
    {
        Err(Error::InvalidArgument)
    } else {
        Ok(())
    }
}

/// Opaque permanent task ID; the TCB and stack remain C-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Task(u32);

impl Task {
    /// # Safety
    /// Follow the module contract. `entry` must never return; `argument` must remain valid.
    pub unsafe fn create(
        slot: u32,
        name: &'static CStr,
        entry: TaskEntry,
        argument: *mut c_void,
        priority: u32,
        stack_words: u32,
    ) -> Result<Self, Error> {
        task_parameters(slot, name, priority, stack_words)?;
        value(unsafe {
            ffi::rp1_freertos_task_create(
                slot,
                name.as_ptr(),
                entry,
                argument,
                priority,
                stack_words,
            )
        })
        .map(Self)
    }

    /// Telemetry ID: the configured slot plus one. Not a C pointer.
    pub const fn id(self) -> u32 {
        self.0
    }

    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn notification_give(self) -> Result<(), Error> {
        value(unsafe { ffi::rp1_freertos_notification_give(self.0) }).map(|_| ())
    }

    /// # Safety
    /// Proc0 external IRQ, logical priority 5..=7; see the module contract.
    pub unsafe fn notification_give_from_isr(self) -> Result<(), Error> {
        unsafe { self.notification_give_from_isr_woken() }.map(|_| ())
    }

    /// Same notification/yield, with the kernel's higher-priority-woken result.
    /// `Ok(false)` still means the notification was given: no immediate
    /// higher-priority switch was requested. It is not a delivery failure.
    /// # Safety
    /// Proc0 external IRQ, logical priority 5..=7; see the module contract.
    pub unsafe fn notification_give_from_isr_woken(self) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_notification_give_from_isr(self.0) }).map(|v| v!=0)
    }

    /// Includes temporary priority inheritance.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn priority(self) -> Result<u32, Error> {
        value(unsafe { ffi::rp1_freertos_priority_get(self.0) })
    }

    /// Minimum untouched stack words observed, not bytes.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn stack_high_water(self) -> Result<u32, Error> {
        value(unsafe { ffi::rp1_freertos_stack_high_water(self.0) })
    }
}

/// Start scheduling with a verified CPU clock in Hz (1 MHz..=1 GHz, multiple of 1000).
/// Success never returns; a scheduler return invokes the nonreturning fault hook.
/// # Safety
/// Proc0 boot thread only. Install the official handlers directly in VTOR first, ensure
/// proc1 cannot access this kernel, and verify the supplied clock against hardware.
pub unsafe fn start(cpu_hz: u32) -> Result<(), Error> {
    value(unsafe { ffi::rp1_freertos_start(cpu_hz) }).map(|_| ())
}

/// Delay in 1 ms ticks. Zero requests a yield. `WAIT_FOREVER` here is a finite tick delay.
/// # Safety
/// Running proc0 task context; see the module contract.
pub unsafe fn delay(ticks: u32) -> Result<(), Error> {
    value(unsafe { ffi::rp1_freertos_delay(ticks) }).map(|_| ())
}

/// Official absolute-period wait. Initialize `previous` with `tick()` once.
/// Advances `previous` by `increment` even when already due; returns whether the
/// kernel blocked. A late caller should count/rebase rather than burst-catch-up.
/// The positive increment and elapsed time since the previous call must be less
/// than half the 32-bit tick range. This is tick cadence, not wall-clock accuracy.
/// # Safety
/// Running proc0 task context; see the module contract. `previous` is task-owned.
pub unsafe fn delay_until(previous: &mut u32, increment: u32) -> Result<bool, Error> {
    value(unsafe { ffi::rp1_freertos_delay_until(previous, increment) }).map(|v| v != 0)
}

/// Wrapping 32-bit scheduler tick count, not wall time.
/// # Safety
/// Running proc0 task context; see the module contract.
pub unsafe fn tick() -> Result<u32, Error> {
    let mut ticks = 0;
    value(unsafe { ffi::rp1_freertos_tick(&mut ticks) })?;
    Ok(ticks)
}

/// Remaining units before a wrapping32-bit deadline, or None when due/past.
/// Both arguments must use the same counter/unit; the deadline must lie within
/// half its wrap period. Works across wrap; ambiguous half-range is rejected.
pub fn deadline_remaining(now: u32, deadline: u32) -> Option<u32> {
    let remaining = deadline.wrapping_sub(now);
    (remaining != 0 && remaining < 0x8000_0000).then_some(remaining)
}

/// Returns `None` for idle or an unmanaged kernel task.
/// # Safety
/// Running proc0 task context; see the module contract.
pub unsafe fn current_task() -> Result<Option<Task>, Error> {
    let mut id = 0;
    value(unsafe { ffi::rp1_freertos_current_task(&mut id) })?;
    Ok(if (1..=TASK_SLOTS).contains(&id) {
        Some(Task(id))
    } else {
        None
    })
}

/// Take this task's counting notification: clear all if `clear`, otherwise decrement one.
/// Returns the previous count, or zero on timeout. `WAIT_FOREVER` blocks indefinitely.
/// # Safety
/// Running proc0 task context; see the module contract.
pub unsafe fn notification_take(clear: bool, ticks: u32) -> Result<u32, Error> {
    let mut count = 0;
    value(unsafe { ffi::rp1_freertos_notification_take(u32::from(clear), ticks, &mut count) })?;
    Ok(count)
}

/// Fixed C-owned queue of up to 16 `u32` values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U32Queue(u32);

impl U32Queue {
    /// # Safety
    /// Proc0 before scheduler start; see the module contract.
    pub unsafe fn create(slot: u32, capacity: u32) -> Result<Self, Error> {
        value(unsafe { ffi::rp1_freertos_queue_create(slot, capacity) }).map(Self)
    }

    /// False means full/timeout; zero ticks polls, `WAIT_FOREVER` blocks indefinitely.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn send(self, item: u32, ticks: u32) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_queue_send(self.0, item, ticks) }).map(|v| v != 0)
    }

    /// None means empty/timeout; zero ticks polls, `WAIT_FOREVER` blocks indefinitely.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn receive(self, ticks: u32) -> Result<Option<u32>, Error> {
        let mut item = 0;
        let received = value(unsafe { ffi::rp1_freertos_queue_receive(self.0, ticks, &mut item) })?;
        Ok(if received != 0 { Some(item) } else { None })
    }
}

/// Initially empty, non-owning binary semaphore. Safe to signal from an eligible IRQ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BinarySemaphore(u32);

impl BinarySemaphore {
    /// # Safety
    /// Proc0 before scheduler start; see the module contract.
    pub unsafe fn create(slot: u32) -> Result<Self, Error> {
        value(unsafe { ffi::rp1_freertos_semaphore_create(0, slot) }).map(Self)
    }

    /// False means empty/timeout; `WAIT_FOREVER` blocks indefinitely.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn take(self, ticks: u32) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_semaphore_take(0, self.0, ticks) }).map(|v| v != 0)
    }

    /// False means already full.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn give(self) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_semaphore_give(0, self.0) }).map(|v| v != 0)
    }

    /// False means already full; yields to a newly awakened higher-priority task.
    /// # Safety
    /// Proc0 external IRQ, logical priority 5..=7; see the module contract.
    pub unsafe fn give_from_isr(self) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_binary_give_from_isr(self.0) }).map(|v| v != 0)
    }

    /// Nonblocking: false means empty. Performs the port's ISR yield if needed.
    /// # Safety
    /// Proc0 external IRQ, logical priority 5..=7; see the module contract.
    pub unsafe fn take_from_isr(self) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_binary_take_from_isr(self.0) }).map(|v| v != 0)
    }
}

/// Nonrecursive priority-inheriting mutex. No ISR operations, no implicit drop/unlock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mutex(u32);

impl Mutex {
    /// # Safety
    /// Proc0 before scheduler start; see the module contract.
    pub unsafe fn create(slot: u32) -> Result<Self, Error> {
        value(unsafe { ffi::rp1_freertos_semaphore_create(1, slot) }).map(Self)
    }

    /// False means timeout; taking a mutex already owned by this task returns WrongState.
    /// # Safety
    /// Running proc0 task context; see the module contract. Release it from the same task.
    pub unsafe fn take(self, ticks: u32) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_semaphore_take(1, self.0, ticks) }).map(|v| v != 0)
    }

    /// Nonowners receive WrongState instead of entering the kernel's ownership assertion.
    /// # Safety
    /// Running proc0 task context; see the module contract.
    pub unsafe fn give(self) -> Result<bool, Error> {
        value(unsafe { ffi::rp1_freertos_semaphore_give(1, self.0) }).map(|v| v != 0)
    }
}

mod ffi {
    use super::{TaskEntry, c_char, c_void};
    unsafe extern "C" {
        pub fn rp1_freertos_task_create(
            slot: u32,
            name: *const c_char,
            entry: TaskEntry,
            argument: *mut c_void,
            priority: u32,
            words: u32,
        ) -> i32;
        pub fn rp1_freertos_start(cpu_hz: u32) -> i32;
        #[cfg(feature = "assert-probe")]
        pub fn rp1_freertos_config_assert_probe() -> !;
        pub fn rp1_freertos_delay(ticks: u32) -> i32;
        pub fn rp1_freertos_delay_until(previous: *mut u32, increment: u32) -> i32;
        pub fn rp1_freertos_tick(ticks: *mut u32) -> i32;
        pub fn rp1_freertos_current_task(id: *mut u32) -> i32;
        pub fn rp1_freertos_priority_get(id: u32) -> i32;
        pub fn rp1_freertos_stack_high_water(id: u32) -> i32;
        pub fn rp1_freertos_notification_take(clear: u32, ticks: u32, count: *mut u32) -> i32;
        pub fn rp1_freertos_notification_give(id: u32) -> i32;
        pub fn rp1_freertos_cancel_notification(generation: u32, active: *const u32,
            cancelled: *mut u32, waiter: *const u32) -> i32;
        pub fn rp1_freertos_notification_give_from_isr(id: u32) -> i32;
        pub fn rp1_freertos_queue_create(slot: u32, capacity: u32) -> i32;
        pub fn rp1_freertos_queue_send(id: u32, item: u32, ticks: u32) -> i32;
        pub fn rp1_freertos_queue_receive(id: u32, ticks: u32, item: *mut u32) -> i32;
        pub fn rp1_freertos_semaphore_create(kind: u32, slot: u32) -> i32;
        pub fn rp1_freertos_semaphore_take(kind: u32, id: u32, ticks: u32) -> i32;
        pub fn rp1_freertos_semaphore_give(kind: u32, id: u32) -> i32;
        pub fn rp1_freertos_binary_give_from_isr(id: u32) -> i32;
        pub fn rp1_freertos_binary_take_from_isr(id: u32) -> i32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_deadline() {
        assert_eq!(deadline_remaining(10, 20), Some(10));
        assert_eq!(deadline_remaining(20, 20), None);
        assert_eq!(deadline_remaining(21, 20), None);
        assert_eq!(deadline_remaining(0xffff_fff0, 0x10), Some(32));
        assert_eq!(deadline_remaining(0x10, 0xffff_fff0), None);
        assert_eq!(deadline_remaining(0, 0x8000_0000), None);
        assert_eq!(deadline_remaining(0, 0x7fff_ffff), Some(0x7fff_ffff));
    }

    #[test]
    fn boundary_contract() {
        assert_eq!(TOTAL_TASK_STACK_WORDS,
            if cfg!(feature = "task-pool-2304") {2304} else {2560});
        assert_eq!([512u32,128,128,256,256,256,256,512].iter().sum::<u32>(),2304);
        assert!(TOTAL_TASK_STACK_WORDS >= 2304);
        assert_eq!(task_parameters(0, c"worker", 1, 128), Ok(()));
        assert_eq!(task_parameters(7, c"123456789012345", 7, 512), Ok(()));
        for (slot, name, priority, words) in [
            (8, c"x", 1, 128),
            (0, c"", 1, 128),
            (0, c"1234567890123456", 1, 128),
            (0, c"x", 8, 128),
            (0, c"x", 1, 127),
            (0, c"x", 1, 513),
        ] {
            assert_eq!(
                task_parameters(slot, name, priority, words),
                Err(Error::InvalidArgument)
            );
        }
        assert_eq!(value(-1), Err(Error::InvalidArgument));
        assert_eq!(value(-2), Err(Error::WrongState));
        assert_eq!(value(-3), Err(Error::SlotInUse));
        assert_eq!(value(-4), Err(Error::Unavailable));
        assert_eq!(value(-5), Err(Error::WrongContext));
        assert_eq!(value(0), Ok(0));
        assert_eq!(value(512), Ok(512));
        assert_eq!(core::mem::size_of::<Task>(), 4);
        assert_eq!(core::mem::size_of::<U32Queue>(), 4);
    }
}

//! Opt-in task-side outer critical BODY elapsed time, not IRQ-mask WCET.
//! Link both official critical functions with GNU `--wrap`; never wrap handlers.
//! Raw timer units are the selected RP1 microsecond reference. One interval must
//! be shorter than its 32-bit wrap (~71 minutes). Higher-priority IRQ time can be
//! included; real enter/exit, exit bookkeeping, inline ISR/PendSV masks are not.
use crate::Error;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub count: u32,
    pub min_us: u32,
    pub max_us: u32,
    pub last_us: u32,
    pub max_nesting: u32,
    pub saturated: u32,
}
const _: () = assert!(core::mem::size_of::<Sample>() == 24);

unsafe extern "C" {
    fn rp1_freertos_critical_timing_start() -> i32;
    fn rp1_freertos_critical_timing_snapshot(out: *mut Sample) -> i32;
}

/// # Safety
/// First privileged proc0 PSP task, scheduler running, no scheduler suspension,
/// PRIMASK/BASEPRI clear, no enclosing critical section. Proc1 never accesses it.
/// This cannot be restarted or used to measure pre-scheduler critical sections.
pub unsafe fn start() -> Result<(), Error> {
    crate::value(unsafe { rp1_freertos_critical_timing_start() }).map(|_| ())
}

/// # Safety
/// Same running-task/context contract as `start`. The snapshot's critical-body
/// cost appears in the next sample. Zero count leaves `min_us == u32::MAX`.
pub unsafe fn snapshot() -> Result<Sample, Error> {
    let mut sample = Sample::default();
    crate::value(unsafe { rp1_freertos_critical_timing_snapshot(&mut sample) })?;
    Ok(sample)
}

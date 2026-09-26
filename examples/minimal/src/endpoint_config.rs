//! Compile-only config transaction foundation. No observer or startup calls this.
//! Host callback errors model returned failures, NOT catchable target MMIO faults.
#![allow(dead_code)]

const SELECTOR: usize = 0x4010_8000;
const DBI: usize = 0x4010_9000;
const RO: usize = DBI + 0x8bc;
const MONITOR2: usize = 0x4010_81a4;
const CTRL0: usize = 0x4001_4000;
const DONE0: usize = 0x4001_4018;
const READY: u32 = 3 << 16;
const LEVELS: u32 = 0x1f << 16;
const PROC1: u32 = 1 << 31;
const NORMAL: [usize; 8] = [DBI, DBI + 4, DBI + 8, DBI + 0x10,
    DBI + 0x14, DBI + 0x18, SELECTOR, RO];

/// NOT constructed from WINDOW_CANDIDATE_ONLY. No safe constructor is provided.
pub(crate) struct ReviewedWindow { low_before_us: u32, budget_us: u32 }

impl ReviewedWindow {
    /// # Safety
    /// A separate deployment review must establish the fresh low timestamp,
    /// finite host-access exclusion margin, proc0 accessor/exception scope and
    /// external known-good recovery. Diagnostic 5000/2500us bounds do not do so.
    pub(crate) unsafe fn new(low_before_us: u32, budget_us: u32) -> Self {
        Self { low_before_us, budget_us }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status { Disabled, AlreadyAttempted, InvalidWindow, Deadline,
    NotReady, Selector, Proc1Active, Tuple, RoEnabled, LevelDrift,
    CallbackFailure, ReadbackMismatch, Complete }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cleanup { NotNeeded, Restored, DeferredUnsafe, Failed }

#[derive(Debug)]
pub(crate) struct Receipt {
    pub status: Status,
    pub cleanup: Cleanup,
    pub mask_before: Option<u32>,
    pub mask_restored: bool,
    pub first_us: Option<u32>,
    pub last_us: Option<u32>,
    /// Initial protected CTRL0/DONE0 reads, not a history of later guard reads.
    pub proc1: [Option<u32>; 2],
    /// id, cmdstat, classrev, BAR0..2, selector, RO. None means NOT read.
    pub before: [Option<u32>; 8],
    pub after: [Option<u32>; 8],
    /// Recipe-order bits: RO-enable, class, sel1, masks0..2, sel0,
    /// normal BAR0..2, RO-restore. Returned writes are not acceptance proof.
    pub writes_attempted: u16,
    pub writes_returned: u16,
    /// Cleanup-only bits: selector0, saved RO. Not configuration rollback.
    pub cleanup_attempted: u8,
    pub cleanup_returned: u8,
}

impl Receipt {
    fn new(status: Status) -> Self {
        Self { status, cleanup: Cleanup::NotNeeded, mask_before: None,
            mask_restored: false, first_us: None, last_us: None, proc1: [None; 2],
            before: [None; 8], after: [None; 8], writes_attempted: 0,
            writes_returned: 0, cleanup_attempted: 0, cleanup_returned: 0 }
    }

    /// A partial transaction is never claimed rolled back, even if RO/selector
    /// cleanup returned successfully. Use independent known-good recovery.
    pub(crate) fn needs_external_recovery(&self) -> bool {
        (self.mask_before.is_some() && !self.mask_restored)
            || (self.writes_attempted != 0 && self.status != Status::Complete)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op { Mask, Restore(u32), Clock, Read(usize), Write(usize, u32) }

// One callback keeps the actual algorithm host-testable, including mask order.
// There is no allocation, RTOS/UART call, wait, retry, or panic assertion here.
fn call(io: &mut impl FnMut(Op) -> Result<u32, ()>, op: Op) -> Result<u32, Status> {
    io(op).map_err(|_| Status::CallbackFailure)
}

fn live(io: &mut impl FnMut(Op) -> Result<u32, ()>) -> Result<u32, Status> {
    let mon = call(io, Op::Read(MONITOR2))?;
    if mon & READY != READY { return Err(Status::NotReady); }
    if call(io, Op::Read(CTRL0))? & PROC1 == 0
        || call(io, Op::Read(DONE0))? & PROC1 != 0 {
        return Err(Status::Proc1Active);
    }
    Ok(mon)
}

fn time(io: &mut impl FnMut(Op) -> Result<u32, ()>, w: &ReviewedWindow,
    r: &mut Receipt) -> Result<(), Status> {
    let now = call(io, Op::Clock)?;
    if r.first_us.is_none() { r.first_us = Some(now); }
    r.last_us = Some(now);
    if now.wrapping_sub(w.low_before_us) >= w.budget_us { Err(Status::Deadline) }
    else { Ok(()) }
}

fn guard(io: &mut impl FnMut(Op) -> Result<u32, ()>, selector: u32,
    w: &ReviewedWindow, r: &mut Receipt) -> Result<(), Status> {
    live(io)?;
    if call(io, Op::Read(SELECTOR))? != selector { return Err(Status::Selector); }
    time(io, w, r)
}

pub(crate) struct Transaction { attempted: bool }

impl Transaction {
    pub(crate) const fn new() -> Self { Self { attempted: false } }

    fn run(&mut self, window: Option<ReviewedWindow>, mut io: impl FnMut(Op) -> Result<u32, ()>) -> Receipt {
        if self.attempted { return Receipt::new(Status::AlreadyAttempted); }
        self.attempted = true; // Every outcome, including rejection, is terminal.
        let Some(w) = window else { return Receipt::new(Status::Disabled); };
        let mut r = Receipt::new(Status::InvalidWindow);
        if w.budget_us == 0 || w.budget_us >= 0x8000_0000 { return r; }
        let mask = match call(&mut io, Op::Mask) {
            Ok(mask) => mask,
            Err(status) => { r.status = status; return r; }
        };
        r.mask_before = Some(mask);
        r.status = match self.protected(&w, &mut io, &mut r) {
            Ok(()) => Status::Complete, Err(status) => status,
        };
        if r.writes_attempted != 0 {
            r.cleanup = if r.status == Status::Complete { Cleanup::Restored }
                else { cleanup(&w, &mut io, &mut r) };
        }
        r.mask_restored = call(&mut io, Op::Restore(mask)) == Ok(mask);
        if !r.mask_restored { r.status = Status::CallbackFailure; }
        r
    }

    fn protected(&self, w: &ReviewedWindow, io: &mut impl FnMut(Op) -> Result<u32, ()>,
        r: &mut Receipt) -> Result<(), Status> {
        time(io, w, r)?;
        let mon = call(io, Op::Read(MONITOR2))?;
        if mon & READY != READY { return Err(Status::NotReady); }
        // Record actual PROC1 state, never infer it from an earlier boot log.
        r.proc1[0] = Some(call(io, Op::Read(CTRL0))?);
        r.proc1[1] = Some(call(io, Op::Read(DONE0))?);
        if r.proc1[0].is_none_or(|v| v & PROC1 == 0)
            || r.proc1[1].is_none_or(|v| v & PROC1 != 0) { return Err(Status::Proc1Active); }
        for (index, address) in NORMAL.into_iter().enumerate() {
            guard(io, 0, w, r)?;
            let value = call(io, Op::Read(address))?;
            // Fallible indexing also avoids a compiled panic edge at Oz.
            *r.before.get_mut(index).ok_or(Status::Tuple)? = Some(value);
        }
        if (live(io)? ^ mon) & LEVELS != 0 { return Err(Status::LevelDrift); }
        if r.before[0] != Some(0x0001_1de4) || r.before[1].is_none_or(|v| v & 6 != 0)
            || r.before[2] != Some(2) || r.before[3..6] != [Some(0); 3] {
            return Err(Status::Tuple);
        }
        let Some(ro) = r.before[7] else { return Err(Status::Tuple); };
        if ro & 1 != 0 { return Err(Status::RoEnabled); }
        let recipe = [(RO, ro | 1, 0), (DBI + 8, 0x0200_0000, 0),
            (SELECTOR, 1, 0), (DBI + 0x10, 0x3fff, 1),
            (DBI + 0x14, 0x3f_ffff, 1), (DBI + 0x18, 0xffff, 1),
            (SELECTOR, 0, 1), (DBI + 0x10, 0xffff_fff0, 0),
            (DBI + 0x14, 0xffff_fff0, 0), (DBI + 0x18, 0xffff_fff0, 0), (RO, ro, 0)];
        for (index, (address, value, selector)) in recipe.into_iter().enumerate() {
            guard(io, selector, w, r)?;
            r.writes_attempted |= 1 << index;
            call(io, Op::Write(address, value))?;
            r.writes_returned |= 1 << index;
        }
        for (index, address) in NORMAL.into_iter().enumerate() {
            guard(io, 0, w, r)?;
            let value = call(io, Op::Read(address))?;
            *r.after.get_mut(index).ok_or(Status::ReadbackMismatch)? = Some(value);
        }
        guard(io, 0, w, r)?;
        if r.after[0] != r.before[0] || r.after[1].is_none_or(|v| v & 6 != 0)
            || r.after[2..] != [Some(0x0200_0000), Some(0xffff_c000),
                Some(0xffc0_0000), Some(0xffff_0000), Some(0), Some(ro)] {
            return Err(Status::ReadbackMismatch);
        }
        Ok(())
    }

    /// # Safety
    /// Requires ReviewedWindow's separately reviewed admission, one persistent
    /// Transaction per boot epoch, and proc0 task context. Not called by firmware.
    /// PRIMASK excludes local configurable IRQs only, not NMI/HardFault, proc1,
    /// host config cycles or reset hardware. No MMIO-stall recovery is provided.
    #[cfg(target_arch = "arm")]
    pub(crate) unsafe fn attempt_mmio(&mut self, window: Option<ReviewedWindow>) -> Receipt {
        self.run(window, |op| Ok(match op {
            Op::Mask => {
                let mask: u32;
                unsafe { core::arch::asm!("mrs {0}, PRIMASK", "cpsid i", "isb",
                    out(reg) mask, options(nostack)); }
                mask
            }
            Op::Restore(mask) => {
                let restored: u32;
                unsafe { core::arch::asm!("dsb sy", "msr PRIMASK, {0}", "isb",
                    in(reg) mask, options(nostack)); }
                unsafe { core::arch::asm!("mrs {0}, PRIMASK", out(reg) restored,
                    options(nomem, nostack, preserves_flags)); }
                restored
            }
            Op::Clock => unsafe { (0x400a_c028 as *const u32).read_volatile() },
            Op::Read(address) => unsafe { (address as *const u32).read_volatile() },
            Op::Write(address, value) => {
                unsafe { (address as *mut u32).write_volatile(value); }
                0
            }
        }))
    }
}

fn cleanup(w: &ReviewedWindow, io: &mut impl FnMut(Op) -> Result<u32, ()>, r: &mut Receipt) -> Cleanup {
    // Do not restore unobserved DBI2 zeros, class, BARs, reset or IRQ state.
    // No DBI access when current readiness/PROC1 exclusion cannot be checked.
    if live(io).is_err() || time(io, w, r).is_err() { return Cleanup::DeferredUnsafe; }
    r.cleanup_attempted |= 1;
    if call(io, Op::Write(SELECTOR, 0)).is_err() { return Cleanup::Failed; }
    r.cleanup_returned |= 1;
    if call(io, Op::Read(SELECTOR)) != Ok(0) { return Cleanup::Failed; }
    if live(io).is_err() || time(io, w, r).is_err() { return Cleanup::DeferredUnsafe; }
    let Some(ro) = r.before[7] else { return Cleanup::Failed; };
    r.cleanup_attempted |= 2;
    if call(io, Op::Write(RO, ro)).is_err() { return Cleanup::Failed; }
    r.cleanup_returned |= 2;
    if live(io).is_err() || time(io, w, r).is_err() { return Cleanup::DeferredUnsafe; }
    if call(io, Op::Read(SELECTOR)) != Ok(0) || call(io, Op::Read(RO)) != Ok(ro) {
        Cleanup::Failed
    } else { Cleanup::Restored }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Bus {
        regs: BTreeMap<usize, u32>, shadow: [u32; 3], trace: Vec<Op>, mask: u32,
        now: u32, writes: usize, fail_writes: Vec<usize>, apply_failed: bool,
        reset_after: Option<usize>, timeout_after: Option<usize>,
        ignore_write: Option<usize>, fail_read: Option<usize>, drift_at: Option<usize>,
    }

    impl Bus {
        fn new(mask: u32) -> Self {
            Self { regs: [(SELECTOR, 0), (MONITOR2, 0x0007_0020),
                (CTRL0, 0xda31_efff), (DONE0, 0x25ce_1000), (RO, 0x000b_ff40),
                (DBI, 0x0001_1de4), (DBI + 4, 0), (DBI + 8, 2),
                (DBI + 0x10, 0), (DBI + 0x14, 0), (DBI + 0x18, 0)].into(),
                shadow: [0; 3], trace: Vec::new(), mask, now: 1100, writes: 0,
                fail_writes: Vec::new(), apply_failed: true, reset_after: None,
                timeout_after: None, ignore_write: None, fail_read: None, drift_at: None }
        }

        fn io(&mut self, op: Op) -> Result<u32, ()> {
            self.trace.push(op);
            if let Op::Mask = op { let old = self.mask; self.mask = 1; return Ok(old); }
            assert_eq!(self.mask, 1, "every MMIO/clock/cleanup operation is protected");
            if self.drift_at == Some(self.trace.len()) {
                *self.regs.get_mut(&MONITOR2).unwrap() ^= 1 << 20;
            }
            match op {
                Op::Mask => unreachable!(),
                Op::Restore(saved) => { self.mask = saved; Ok(saved) }
                Op::Clock => Ok(self.now),
                Op::Read(address) => {
                    if (DBI..DBI + 0x1000).contains(&address) {
                        assert_eq!(self.regs[&MONITOR2] & READY, READY, "no reset-low DBI read");
                        assert_eq!(self.regs[&SELECTOR], 0, "no DBI2/shadow read oracle");
                    }
                    if self.fail_read == Some(address) { return Err(()); }
                    Ok(self.regs[&address])
                }
                Op::Write(address, value) => {
                    assert!([SELECTOR, RO, DBI + 8, DBI + 0x10, DBI + 0x14, DBI + 0x18]
                        .contains(&address), "forbidden write address");
                    assert_eq!(self.regs[&MONITOR2] & READY, READY, "no reset-low writes");
                    self.writes += 1;
                    let fail = self.fail_writes.contains(&self.writes);
                    if (!fail || self.apply_failed) && self.ignore_write != Some(self.writes) {
                        if (DBI + 0x10..=DBI + 0x18).contains(&address) {
                            let index = (address - DBI - 0x10) / 4;
                            if self.regs[&SELECTOR] == 1 { self.shadow[index] = value; }
                            else { self.regs.insert(address, value & !self.shadow[index]); }
                        } else if address != DBI + 8 || self.regs[&RO] & 1 != 0 {
                            self.regs.insert(address, value);
                        }
                    }
                    if self.reset_after == Some(self.writes) {
                        self.regs.insert(MONITOR2, 0x0004_0020);
                    }
                    if self.timeout_after == Some(self.writes) { self.now = 5100; }
                    if fail { Err(()) } else { Ok(0) }
                }
            }
        }
    }

    fn window() -> Option<ReviewedWindow> { Some(unsafe { ReviewedWindow::new(100, 5000) }) }
    fn run(bus: &mut Bus) -> Receipt { Transaction::new().run(window(), |op| bus.io(op)) }

    fn guard_trace(trace: &mut Vec<Op>) {
        trace.extend([Op::Read(MONITOR2), Op::Read(CTRL0), Op::Read(DONE0),
            Op::Read(SELECTOR), Op::Clock]);
    }

    #[test]
    fn exact_recipe_reads_allowlist_mask_and_oracle() {
        for saved_mask in [0, 1] {
            let mut bus = Bus::new(saved_mask);
            let r = run(&mut bus);
            assert_eq!((r.status, r.cleanup), (Status::Complete, Cleanup::Restored));
            assert_eq!((r.mask_before, r.mask_restored, bus.mask), (Some(saved_mask), true, saved_mask));
            assert_eq!((r.first_us, r.last_us), (Some(1100), Some(1100)));
            assert_eq!((r.writes_attempted, r.writes_returned), (0x7ff, 0x7ff));
            assert_eq!((r.cleanup_attempted, r.cleanup_returned), (0, 0));
            assert_eq!(r.proc1, [Some(0xda31_efff), Some(0x25ce_1000)]);
            assert_eq!(r.before, [Some(0x11de4), Some(0), Some(2), Some(0),
                Some(0), Some(0), Some(0), Some(0xbff40)]);
            assert_eq!(r.after, [Some(0x11de4), Some(0), Some(0x0200_0000),
                Some(0xffff_c000), Some(0xffc0_0000), Some(0xffff_0000), Some(0), Some(0xbff40)]);
            assert!(!r.needs_external_recovery());
            let mut expected = vec![Op::Mask, Op::Clock, Op::Read(MONITOR2),
                Op::Read(CTRL0), Op::Read(DONE0)];
            for address in NORMAL {
                guard_trace(&mut expected); expected.push(Op::Read(address));
            }
            expected.extend([Op::Read(MONITOR2), Op::Read(CTRL0), Op::Read(DONE0)]);
            for (address, value) in [(RO, 0xbff41), (DBI + 8, 0x0200_0000),
                (SELECTOR, 1), (DBI + 0x10, 0x3fff), (DBI + 0x14, 0x3f_ffff),
                (DBI + 0x18, 0xffff), (SELECTOR, 0), (DBI + 0x10, 0xffff_fff0),
                (DBI + 0x14, 0xffff_fff0), (DBI + 0x18, 0xffff_fff0), (RO, 0xbff40)] {
                guard_trace(&mut expected); expected.push(Op::Write(address, value));
            }
            for address in NORMAL {
                guard_trace(&mut expected); expected.push(Op::Read(address));
            }
            guard_trace(&mut expected); expected.push(Op::Restore(saved_mask));
            assert_eq!(bus.trace, expected);
        }
    }

    #[test]
    fn every_rejected_precondition_has_zero_writes() {
        for (address, value, status) in [(MONITOR2, 1 << 16, Status::NotReady),
            (MONITOR2, 1 << 17, Status::NotReady), (MONITOR2, 0, Status::NotReady),
            (CTRL0, 0x5a31_efff, Status::Proc1Active),
            (DONE0, 0xa5ce_1000, Status::Proc1Active), (SELECTOR, 1, Status::Selector),
            (SELECTOR, 0x23, Status::Selector), (DBI, 0, Status::Tuple),
            (DBI + 4, 2, Status::Tuple), (DBI + 4, 4, Status::Tuple),
            (DBI + 8, 0x0200_0002, Status::Tuple), (DBI + 0x10, 1, Status::Tuple),
            (DBI + 0x14, 1, Status::Tuple), (DBI + 0x18, 1, Status::Tuple),
            (RO, 0xbff41, Status::RoEnabled)] {
            let mut bus = Bus::new(0); bus.regs.insert(address, value);
            let r = run(&mut bus);
            assert_eq!(r.status, status, "address={address:x}");
            assert_eq!((bus.writes, r.writes_attempted, r.cleanup), (0, 0, Cleanup::NotNeeded));
            assert_eq!((bus.mask, r.mask_restored), (0, true));
            assert_eq!(r.after, [None; 8]);
        }
        let mut bus = Bus::new(1); bus.drift_at = Some(54); // Final initial MONITOR2.
        let r = run(&mut bus);
        assert_eq!((r.status, bus.writes, bus.mask), (Status::LevelDrift, 0, 1));
        // Errors are returned only by the HOST callback, never a target fault handler.
        for address in NORMAL.into_iter().chain([MONITOR2, CTRL0, DONE0]) {
            let mut bus = Bus::new(0); bus.fail_read = Some(address);
            let r = run(&mut bus);
            assert_eq!((r.status, bus.writes, bus.mask), (Status::CallbackFailure, 0, 0));
        }
    }

    #[test]
    fn mutation_failures_cleanup_without_claiming_rollback() {
        for failed in 1..=11 {
            for apply in [false, true] {
                let mut bus = Bus::new(0); bus.fail_writes = vec![failed]; bus.apply_failed = apply;
                let r = run(&mut bus);
                assert_eq!((r.status, r.cleanup), (Status::CallbackFailure, Cleanup::Restored));
                assert_eq!(r.writes_attempted, (1 << failed) - 1);
                assert_eq!(r.writes_returned, (1 << (failed - 1)) - 1);
                assert_eq!((r.cleanup_attempted, r.cleanup_returned), (3, 3));
                assert_eq!((bus.regs[&SELECTOR], bus.regs[&RO], bus.mask), (0, 0xbff40, 0));
                assert!(r.needs_external_recovery());
                assert_eq!(r.after, [None; 8]);
            }
            let mut bus = Bus::new(1); bus.ignore_write = Some(failed);
            let r = run(&mut bus);
            assert_ne!(r.status, Status::Complete, "callback success is not write acceptance");
            assert!(r.needs_external_recovery());
            assert!(r.mask_restored);
        }
    }

    #[test]
    fn reset_loss_and_deadline_after_every_mutation_are_terminal() {
        for changed in 1..=11 {
            for reset in [false, true] {
                let mut bus = Bus::new(0);
                if reset { bus.reset_after = Some(changed); } else { bus.timeout_after = Some(changed); }
                let mut txn = Transaction::new();
                let r = txn.run(window(), |op| bus.io(op));
                assert_eq!(r.status, if reset { Status::NotReady } else { Status::Deadline });
                assert_eq!(r.cleanup, Cleanup::DeferredUnsafe);
                assert_eq!(r.writes_attempted, (1 << changed) - 1);
                assert_eq!(r.writes_returned, r.writes_attempted);
                assert_eq!(r.cleanup_attempted, 0);
                assert_eq!(bus.mask, 0);
                assert!(r.needs_external_recovery());
                let trace_len = bus.trace.len(); bus.now = 1100;
                assert_eq!(txn.run(window(), |op| bus.io(op)).status, Status::AlreadyAttempted);
                assert_eq!(bus.trace.len(), trace_len);
            }
        }
    }

    #[test]
    fn disabled_invalid_expiry_wrap_and_no_rearm() {
        for (w, now, expected) in [(None, 1100, Status::Disabled),
            (Some(unsafe { ReviewedWindow::new(100, 0) }), 1100, Status::InvalidWindow),
            (Some(unsafe { ReviewedWindow::new(100, 0x8000_0000) }), 1100, Status::InvalidWindow),
            (window(), 5100, Status::Deadline), (window(), 5099, Status::Complete),
            (Some(unsafe { ReviewedWindow::new(u32::MAX - 100, 5000) }), 50, Status::Complete)] {
            let mut bus = Bus::new(1); bus.now = now;
            let mut txn = Transaction::new(); let r = txn.run(w, |op| bus.io(op));
            assert_eq!(r.status, expected);
            assert_eq!(bus.mask, 1);
            if expected != Status::Complete { assert_eq!(bus.writes, 0); }
            let count = bus.trace.len(); bus.now = 0;
            assert_eq!(txn.run(window(), |op| bus.io(op)).status, Status::AlreadyAttempted);
            assert_eq!(bus.trace.len(), count);
        }
    }

    #[test]
    fn cleanup_failures_and_unexecuted_readback_are_truthful() {
        for (second_fail, attempted, returned) in [(2, 1, 0), (3, 3, 1)] {
            let mut bus = Bus::new(0); bus.fail_writes = vec![1, second_fail];
            let r = run(&mut bus);
            assert_eq!(r.cleanup, Cleanup::Failed);
            assert_eq!((r.cleanup_attempted, r.cleanup_returned), (attempted, returned));
            assert!(r.needs_external_recovery()); assert!(r.mask_restored);
        }
        let mut bus = Bus::new(0);
        let r = Transaction::new().run(window(), |op| {
            if bus.writes == 11 && op == Op::Read(DBI + 8) { return Err(()); }
            bus.io(op)
        });
        assert_eq!(r.status, Status::CallbackFailure);
        assert_eq!(&r.after[..2], &[Some(0x11de4), Some(0)]);
        assert_eq!(&r.after[2..], &[None; 6]);
        assert_eq!(r.cleanup, Cleanup::Restored);
        let mut bus = Bus::new(0);
        let r = Transaction::new().run(window(), |op| {
            let result = bus.io(op);
            if let Op::Restore(_) = op { Ok(1) } else { result }
        });
        assert_eq!((r.status, r.mask_restored), (Status::CallbackFailure, false));
        assert!(r.needs_external_recovery());
        let mut bus = Bus::new(0); bus.now = 5100;
        let r = Transaction::new().run(window(), |op| {
            let result = bus.io(op);
            if let Op::Restore(_) = op { Err(()) } else { result }
        });
        assert_eq!(r.writes_attempted, 0);
        assert!(!r.mask_restored);
        assert!(r.needs_external_recovery());
    }

    #[test]
    fn cleanup_never_exceeds_margin_or_reuses_lost_proc1_exclusion() {
        for reset in [false, true] {
            let mut bus = Bus::new(0); bus.fail_writes = vec![1];
            // Initial failure, then only cleanup's selector write returns.
            if reset { bus.reset_after = Some(2); } else { bus.timeout_after = Some(2); }
            let r = run(&mut bus);
            assert_eq!((r.status, r.cleanup), (Status::CallbackFailure, Cleanup::DeferredUnsafe));
            assert_eq!((r.cleanup_attempted, r.cleanup_returned, bus.writes), (1, 1, 2));
            assert!(r.needs_external_recovery()); assert!(r.mask_restored);
        }
        for changed in 1..=11 {
            let mut bus = Bus::new(1);
            let r = Transaction::new().run(window(), |op| {
                let result = bus.io(op);
                if bus.writes == changed { bus.regs.insert(CTRL0, 0x5a31_efff); }
                result
            });
            assert_eq!((r.status, r.cleanup), (Status::Proc1Active, Cleanup::DeferredUnsafe));
            assert_eq!((r.cleanup_attempted, bus.writes), (0, changed));
            assert!(r.needs_external_recovery()); assert!(r.mask_restored);
        }
        let mut bus = Bus::new(0);
        let r = Transaction::new().run(window(), |op| {
            if bus.trace.len() > 10 && op == Op::Clock { bus.now = 5100; }
            bus.io(op)
        });
        assert_eq!((r.status, r.writes_attempted, bus.writes), (Status::Deadline, 0, 0));
        assert_eq!(r.cleanup, Cleanup::NotNeeded);
        assert!(r.mask_restored);
    }
}

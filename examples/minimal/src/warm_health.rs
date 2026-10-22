//! Proc0-only, allocation-free shadow checkpoint; no watchdog/MMIO access.
//! Standalone check: rustc --edition=2024 --test warm_health.rs -o <tmpfs>/test
//!
//! The caller supplies one fresh, barrier-bracketed owner/R1 observation at a
//! completed quiet boundary. Booleans below are verified facts, not word161.
//! Retain one Cursor for the entire warm epoch; never reset it to retry a refusal.

const GENERATIONS: u32 = 8;
const PROGRESS_MAX_AGE: u32 = 35_000;
// The current R1 monitor period is 1000 ticks; this is a ceiling, not a timer.
const R1_MAX_AGE: u32 = 1000;

pub(crate) struct Sample {
    pub core: u32,
    pub warm_epoch: u32,
    pub now: u32,
    pub sequence_before: u32,
    pub sequence_after: u32,
    pub generation: u32,
    pub operation: u32,
    pub deadline: u32,
    pub checked_count: u32,
    pub last_checked_tick: u32,
    /// SPI/UART/I2C generations from the fully checked rolling receipts.
    pub checked_generation: [u32; 3],
    /// Existing exact receipts, payload/canaries, and error latches all pass.
    pub receipts_ok: bool,
    /// Owner published completion AFTER finish/abort cleanup and cancel refusal.
    pub cleanup_complete: bool,
    pub buffers_returned: bool,
    pub late_buffers_unchanged: bool,
    pub active_generation: [u32; 3],
    /// All three sources and their NVIC enable/pending/active bits were quiet
    /// at both ends of the observation, not just the nonselected sources.
    pub quiet: bool,
    pub checked_irqs: [u32; 3],
    pub irqs_before: [u32; 3],
    pub irqs_after: [u32; 3],
    /// Fresh full R1 pass: spinner/queue/mutex progress, context/error latches,
    /// all seven task + owner HWM, MSP and thread-mode checks, without weakening.
    pub r1_ok: bool,
    pub r1_checked_tick: u32,
    /// R1 words64/80/49/50, in that order; tick/idle are NOT progress inputs.
    pub r1_progress: [u32; 4],
}

// No Clone/Copy: one proc0 consumer owns this cursor, not an exported grant.
pub(crate) struct Cursor {
    warm_epoch: u32,
    generation: u32,
    r1_progress: [u32; 4],
}

impl Cursor {
    /// Call once at the admitted warm epoch's start, with its R1 baseline.
    pub(crate) const fn new(warm_epoch: u32, r1_baseline: [u32; 4]) -> Self {
        Self { warm_epoch, generation: 0, r1_progress: r1_baseline }
    }

    /// True consumes new checked progress NOW, for shadow telemetry only.
    /// False leaves the cursor untouched. Neither result authorizes a reload,
    /// arm, expiry, reset, delayed action, or reuse of a stored true value.
    // Keep one shared decision body: inlining both the admission and replay
    // call duplicated code in the measured Oz target build.
    #[inline(never)]
    pub(crate) fn consume(&mut self, s: &Sample) -> bool {
        let budget = match s.operation {
            0 => 1000,
            7 if s.generation == 2 => 20_000,
            _ => return false, // Includes UART ACK/cleanup op6 and terminal op8.
        };
        if s.core != 0 || self.warm_epoch == 0 || s.warm_epoch != self.warm_epoch
            || s.sequence_before & 1 != 0 || s.sequence_before != s.sequence_after
            || !(1..=GENERATIONS).contains(&s.generation)
            || s.generation <= self.generation
            || s.checked_count != 3 * s.generation
            || s.checked_generation != [s.generation; 3]
            || !s.receipts_ok || !s.cleanup_complete || !s.buffers_returned
            || !s.late_buffers_unchanged || s.active_generation != [0; 3] || !s.quiet
            || s.irqs_before != s.checked_irqs || s.irqs_after != s.checked_irqs
            || s.deadline.wrapping_sub(s.now).wrapping_sub(1) >= budget
            || s.now.wrapping_sub(s.last_checked_tick) > PROGRESS_MAX_AGE
            || !s.r1_ok || s.now.wrapping_sub(s.r1_checked_tick) > R1_MAX_AGE
            || s.r1_progress.iter().zip(self.r1_progress).any(|(&now, before)| now == before)
        {
            return false;
        }
        self.generation = s.generation;
        self.r1_progress = s.r1_progress;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Sample {
        Sample {
            core: 0, warm_epoch: 1, now: 100, sequence_before: 12, sequence_after: 12,
            generation: 1, operation: 0, deadline: 1100, checked_count: 3,
            last_checked_tick: 90, checked_generation: [1; 3], receipts_ok: true,
            cleanup_complete: true, buffers_returned: true, late_buffers_unchanged: true,
            active_generation: [0; 3], quiet: true, checked_irqs: [4, 19, 2],
            irqs_before: [4, 19, 2], irqs_after: [4, 19, 2], r1_ok: true,
            r1_checked_tick: 99, r1_progress: [10, 20, 30, 40],
        }
    }

    fn cursor() -> Cursor { Cursor::new(1, [0; 4]) }

    #[test]
    fn progress_is_consumed_once_and_refusal_does_not_advance() {
        let mut c = cursor();
        let mut s = sample();
        for bus in 0..3 {
            s.checked_generation[bus] = 0;
            assert!(!c.consume(&s));
            s.checked_generation[bus] = 1;
        }
        assert!(c.consume(&s));
        assert!(!c.consume(&s));
        // Tick, owner sequence, IRQ noise, and R1 progress cannot renew a bus generation.
        s.now += 1; s.sequence_before += 2; s.sequence_after += 2;
        s.r1_progress = [11, 21, 31, 41];
        assert!(!c.consume(&s));
        s.irqs_after[0] += 1;
        assert!(!c.consume(&s));
        s.irqs_after = s.checked_irqs;
        s.generation = 2; s.checked_count = 6; s.checked_generation = [2; 3];
        assert!(c.consume(&s));
        assert!(!c.consume(&s));
        s.generation = 1; s.checked_count = 3; s.checked_generation = [1; 3];
        assert!(!c.consume(&s)); // Older replay, not just equal replay.
    }

    #[test]
    fn incoherence_cleanup_ownership_irq_and_r1_fail_closed() {
        let mutations: &[fn(&mut Sample)] = &[
            |s| s.core = 1, |s| s.warm_epoch = 2,
            |s| { s.sequence_before = 13; s.sequence_after = 13; },
            |s| s.sequence_after += 2, |s| s.checked_count -= 1,
            |s| s.receipts_ok = false, |s| s.cleanup_complete = false,
            |s| s.buffers_returned = false, |s| s.late_buffers_unchanged = false,
            |s| s.quiet = false, |s| s.r1_ok = false,
        ];
        for change in mutations {
            let mut c = cursor(); let mut s = sample(); change(&mut s);
            assert!(!c.consume(&s));
            assert!(c.consume(&sample()));
        }
        for bus in 0..3 {
            let mut s = sample(); s.active_generation[bus] = 1;
            assert!(!cursor().consume(&s));
            s = sample(); s.irqs_before[bus] += 1;
            assert!(!cursor().consume(&s));
            s = sample(); s.irqs_after[bus] += 1;
            assert!(!cursor().consume(&s));
        }
        for task in 0..4 {
            let mut s = sample(); s.r1_progress[task] = 0;
            assert!(!cursor().consume(&s));
        }
        assert!(!Cursor::new(0, [0; 4]).consume(&sample()));
        let mut c = cursor(); let mut s = sample(); assert!(c.consume(&s));
        s.generation = 2; s.checked_count = 6; s.checked_generation = [2; 3];
        assert!(!c.consume(&s)); // The same R1 evidence cannot serve a second generation.
        s.r1_progress = [11, 21, 31, 41]; assert!(c.consume(&s));
    }

    #[test]
    fn wrapped_ticks_and_exact_bounds() {
        let mut s = sample();
        s.now = 5; s.last_checked_tick = u32::MAX - 4; s.r1_checked_tick = u32::MAX;
        for remaining in [1, 1000] {
            s.deadline = s.now.wrapping_add(remaining); assert!(cursor().consume(&s));
        }
        for remaining in [0, 1001, 0x8000_0000, u32::MAX] {
            s.deadline = s.now.wrapping_add(remaining); assert!(!cursor().consume(&s));
        }
        s.deadline = 6;
        s.last_checked_tick = s.now.wrapping_sub(PROGRESS_MAX_AGE);
        s.r1_checked_tick = s.now.wrapping_sub(R1_MAX_AGE);
        assert!(cursor().consume(&s));
        s.last_checked_tick = s.last_checked_tick.wrapping_sub(1);
        assert!(!cursor().consume(&s));
        s.last_checked_tick = s.now;
        s.r1_checked_tick = s.r1_checked_tick.wrapping_sub(1);
        assert!(!cursor().consume(&s));
        s.r1_checked_tick = s.now.wrapping_add(1); assert!(!cursor().consume(&s));
        s.r1_checked_tick = s.now;
        s.last_checked_tick = s.now.wrapping_add(1); assert!(!cursor().consume(&s));
    }

    #[test]
    fn bounded_handoff_only_and_terminal_is_never_progress() {
        let mut s = sample();
        for op in [1, 2, 3, 4, 5, 6, 7, 8, 9, u32::MAX] {
            s.operation = op; assert!(!cursor().consume(&s));
        }
        s.operation = 7; s.generation = 2; s.checked_count = 6; s.checked_generation = [2; 3];
        for remaining in [1, 20_000] {
            s.deadline = s.now + remaining; assert!(cursor().consume(&s));
        }
        for remaining in [0, 20_001] {
            s.deadline = s.now + remaining; assert!(!cursor().consume(&s));
        }
        s.operation = 8; s.generation = 8; s.checked_count = 24; s.checked_generation = [8; 3];
        s.deadline = s.now + 1;
        assert!(!cursor().consume(&s)); // All healthy evidence still cannot feed at terminal op8.
        s.operation = 0;
        for g in [0, 9, u32::MAX] {
            s.generation = g; assert!(!cursor().consume(&s));
        }
    }
}

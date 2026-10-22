//! Opt-in C1 period-loop profiler (`freertos-amp-proc1-profile`), record v3/v6.
//!
//! The RP1 Cortex-M3 has no DWT cycle counter (DWT NOCYCCNT=1), so C1's own
//! SysTick is used as a free-running core-clock down counter with TICKINT=0.
//! It is only taken when it reads idle (CSR/RVR zero); otherwise every hook is
//! a no-op and STATE records the refusal.
//!
//! v2 keeps the per-iteration cost low: every iteration records only its total
//! (begin..end), lateness and a histogram; the per-section breakdown is taken
//! on one iteration in SAMPLE_EVERY. On other iterations `mark()` is a flag
//! test, whose call cost is probed before READY so totals can be corrected.
//! Results: SRAM 0x2000f700 (64 words) and 0x2000f600 (32 words), outside every
//! linked section, between the profile code and the RTOS record (0x2000f800).
const BASE: usize = 0x2000_f700; // 64 published words, Linux BAR2 + 0xf700
const LOCAL: usize = 0x2000_f600; // 32 words: C1 scratch 0..15, published sums 16..31
const SYST_CSR: usize = 0xe000_e010;
const SYST_RVR: usize = 0xe000_e014;
const SYST_CVR: usize = 0xe000_e018;
const RAW_LOW: usize = 0x400a_c028;
const ISRAM: usize = 0x1000_0000; // C1's own proc-local ISRAM instance (unused by this firmware)
const DSRAM: usize = 0x1000_2000; // C1's own proc-local DSRAM instance (read only here)
const SNIPPET: usize = 0x2000_f680; // free shared SRAM after LOCAL, before BASE
const NOP2: u32 = 0xbf00_bf00; // two Thumb NOPs
const BX_LR2: u32 = 0x4770_4770; // bx lr
const MASK: u32 = 0x00ff_ffff;
const SAMPLE_EVERY: u32 = 64;
pub(super) const SEGMENTS: usize = 11;

// Published words at BASE (u32). Probes are (min, max) over PROBE_N samples.
const MAGIC: usize = 0; // "PRF1" written last
const VERSION: usize = 1; // 3, or 6 for the no-hooks EXAC probe
const STATE: usize = 2; // 1 probing, 2 loop live, 0xbad0 SysTick not idle
const CSR_BEFORE: usize = 3;
const RVR_BEFORE: usize = 4;
const CSR_AFTER: usize = 5;
const CYCLES_10MS: usize = 7;
const CYCLES_PER_US: usize = 8;
const PROBES: usize = 9; // 8 probes x 2 words = 9..24
const PROBE_N: u32 = 64;
const ITERATIONS: usize = 25; // every iteration
const TOTAL_MAX: usize = 26; // every iteration, begin..end
const SLACK_MIN: usize = 27;
const OVERRUNS: usize = 28; // total >= 200 us
const LATE_MAX_US: usize = 29;
const LATE_SUM_US: usize = 30;
const WORST_CYCLE: usize = 31;
const WORST_LATE_US: usize = 32;
const LATE_COUNT: usize = 33; // deadline detected >= 1 us late
const SEG_MAX: usize = 34; // 34..44, sampled iterations only
const WORST: usize = 45; // 45..55, slowest sampled iteration
const HIST: usize = 56; // 56..63 totals <10,<20,<40,<80,<120,<160,<200,>=200 us
const WORDS: usize = 64;
// LOCAL words: 0..15 C1 scratch, 16..31 published sums.
const PREV: usize = 0;
const END: usize = 1;
const LATE_NOW: usize = 2;
const START: usize = 3;
const SCRATCH: usize = 4; // 4..14
const SAMPLE: usize = 15; // nonzero while this iteration is sampled
const SAMPLES: usize = 16;
const SAMPLED_MAX: usize = 17; // total of the WORST iteration
const TOTAL_SUM: usize = 18; // every iteration, wrapping
const SAMPLED_SUM: usize = 19;
const SEG_SUM: usize = 20; // 20..30
const LOCAL_WORDS: usize = 32;

#[inline(always)]
fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
#[inline(always)]
fn write(address: usize, value: u32) { unsafe { (address as *mut u32).write_volatile(value) } }
#[inline(always)]
fn get(i: usize) -> u32 { read(BASE + 4 * i) }
#[inline(always)]
fn put(i: usize, value: u32) { write(BASE + 4 * i, value) }
#[inline(always)]
fn lget(i: usize) -> u32 { read(LOCAL + 4 * i) }
#[inline(always)]
fn lput(i: usize, value: u32) { write(LOCAL + 4 * i, value) }
#[inline(always)]
fn ladd(i: usize, value: u32) { lput(i, lget(i).wrapping_add(value)) }
#[inline(always)]
fn add(i: usize, value: u32) { put(i, get(i).wrapping_add(value)) }
#[inline(always)]
fn raise(i: usize, value: u32) { if value > get(i) { put(i, value); } }
#[inline(always)]
fn cycles() -> u32 { read(SYST_CVR) }
/// SysTick counts down: `earlier - later`, modulo 24 bits.
#[inline(always)]
fn elapsed(earlier: u32, later: u32) -> u32 { earlier.wrapping_sub(later) & MASK }
#[inline(always)]
fn live() -> bool { get(STATE) == 2 }

#[inline(always)]
fn measure(slot: usize, f: impl Fn()) {
    let (mut min, mut max) = (u32::MAX, 0u32);
    for _ in 0..PROBE_N {
        let a = cycles();
        f();
        let d = elapsed(a, cycles());
        min = min.min(d);
        max = max.max(d);
    }
    put(slot, min);
    put(slot + 1, max);
}

/// Runs once on C1 before READY. Bounded: ~10 ms rate check + 6x64 samples.
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn probe() -> bool {
    for i in 0..WORDS { put(i, 0); }
    for i in 0..LOCAL_WORDS { lput(i, 0); }
    put(VERSION, if cfg!(feature = "freertos-amp-proc1-no-hooks") { 6 } else { 3 });
    put(STATE, 1);
    let (csr, rvr) = (read(SYST_CSR), read(SYST_RVR));
    put(CSR_BEFORE, csr);
    put(RVR_BEFORE, rvr);
    if csr & 1 != 0 || rvr != 0 {
        // Not an idle private SysTick (e.g. shared with C0 FreeRTOS): never touch it.
        put(STATE, 0xbad0);
        put(MAGIC, u32::from_le_bytes(*b"PRF1"));
        return false;
    }
    write(SYST_RVR, MASK);
    write(SYST_CVR, 0);
    write(SYST_CSR, 0b101); // ENABLE | CLKSOURCE=processor, TICKINT=0
    unsafe { core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags)); }
    put(CSR_AFTER, read(SYST_CSR));
    // CALIB (word 6) is 0x1387 in v1/v2; not re-read in v3 (space).
    let t0 = read(RAW_LOW);
    let c0 = cycles();
    let mut guard = 0u32;
    while read(RAW_LOW).wrapping_sub(t0) < 10_000 {
        guard = guard.wrapping_add(1);
        if guard > 50_000_000 { break; }
    }
    let c10 = elapsed(c0, cycles());
    put(CYCLES_10MS, c10);
    put(CYCLES_PER_US, (c10 / 10_000).max(1));
    // v3 memory probes. The same 16-NOP + `bx lr` snippet is written to C1's
    // proc-local ISRAM (Code region, 0x10000000) and to free shared SRAM
    // (0x2000f680, System region) and timed through the same indirect call.
    #[cfg(not(feature = "freertos-amp-proc1-no-hooks"))]
    {
    for i in 0..9 {
        let op = if i < 8 { NOP2 } else { BX_LR2 };
        write(ISRAM + 4 * i, op);
        write(SNIPPET + 4 * i, op);
    }
    unsafe { core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags)); }
    let isram: extern "C" fn() = unsafe { core::mem::transmute(ISRAM | 1) };
    let shared: extern "C" fn() = unsafe { core::mem::transmute(SNIPPET | 1) };
    measure(PROBES, || {});
    measure(PROBES + 2, || isram());
    measure(PROBES + 4, || shared());
    measure(PROBES + 6, || { let _ = read(DSRAM); });
    // Shared SRAM read (slot 4) was 34 cycles in v1 and v2; dropped for space.
    // Unsampled hook (SAMPLE=0): call + flag test, as paid 10x per iteration.
    measure(PROBES + 14, || mark(0));
    }
    #[cfg(feature = "freertos-amp-proc1-no-hooks")]
    lib_probes();
    put(SLACK_MIN, u32::MAX);
    lput(END, cycles());
    put(STATE, 2);
    put(MAGIC, u32::from_le_bytes(*b"PRF1"));
    true
}

/// Called when a deadline is detected; `late_us` = detection time - deadline.
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn begin(late_us: u32) {
    if !live() { return; }
    let now = cycles();
    let n = get(ITERATIONS);
    if n != 0 {
        let slack = elapsed(lget(END), now);
        if slack < get(SLACK_MIN) { put(SLACK_MIN, slack); }
    }
    lput(SAMPLE, u32::from(n % SAMPLE_EVERY == 0));
    lput(START, now);
    lput(PREV, now);
    lput(LATE_NOW, late_us);
}

#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn mark(segment: usize) {
    if lget(SAMPLE) == 0 || segment >= SEGMENTS { return; }
    let now = cycles();
    lput(SCRATCH + segment, elapsed(lget(PREV), now));
    lput(PREV, now);
}

/// Closes the iteration: total, lateness, histogram; sampled breakdown.
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn end(cycle: u32) {
    if !live() { return; }
    let now = cycles();
    let total = elapsed(lget(START), now);
    let late = lget(LATE_NOW);
    if lget(SAMPLE) != 0 {
        lput(SCRATCH + SEGMENTS - 1, elapsed(lget(PREV), now));
        for s in 0..SEGMENTS {
            let d = lget(SCRATCH + s);
            raise(SEG_MAX + s, d);
            ladd(SEG_SUM + s, d);
        }
        if total > lget(SAMPLED_MAX) {
            lput(SAMPLED_MAX, total);
            for s in 0..SEGMENTS { put(WORST + s, lget(SCRATCH + s)); }
        }
        ladd(SAMPLES, 1);
        ladd(SAMPLED_SUM, total);
    }
    if total > get(TOTAL_MAX) {
        put(TOTAL_MAX, total);
        put(WORST_CYCLE, cycle);
        put(WORST_LATE_US, late);
    }
    ladd(TOTAL_SUM, total);
    let us = total / get(CYCLES_PER_US).max(1);
    let bucket = [10, 20, 40, 80, 120, 160, 200].iter().position(|b| us < *b).unwrap_or(7);
    add(HIST + bucket, 1);
    if us >= 200 { add(OVERRUNS, 1); }
    if late != 0 { add(LATE_COUNT, 1); }
    raise(LATE_MAX_US, late);
    add(LATE_SUM_US, late);
    add(ITERATIONS, 1);
    lput(END, cycles());
}

// No-hooks v6 reuses the unused snippet area for a single-writer mailbox/results.
#[cfg(feature = "freertos-amp-proc1-no-hooks")]
const EXAC: usize = 0x4040_0380; // word 7; no other user in this replacement firmware

#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(always)]
fn exclusive_read() -> u32 {
    let value;
    unsafe { core::arch::asm!("ldrex {v}, [{p}]", "clrex", p = in(reg) EXAC,
        v = out(reg) value, options(nostack, preserves_flags)); }
    value
}

#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(always)]
fn attempt(owner: u32) -> (u32, u32) {
    let (value, status);
    unsafe { core::arch::asm!("ldrex {v}, [{p}]", "strex {s}, {owner}, [{p}]", "dmb sy",
        p = in(reg) EXAC, owner = in(reg) owner, v = out(reg) value,
        s = out(reg) status, options(nostack, preserves_flags)); }
    (value, status)
}

#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn lib_probes_init() {
    for i in 0..13 { write(SNIPPET + i * 4, 0); }
    barrier();
}

/// Called once per C0 launch poll, only before C1 READY. No loop or waiting.
/// C0 clears the mailbox while C1 is held; thereafter C1 owns command (word 0)
/// and C0 owns acknowledgement (word 1) and results 8..12. Command is published
/// only after C1 has observed DONE bit6 and acquired the otherwise unused word.
#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn lib_probes_c0() {
    if read(SNIPPET) != 1 || read(SNIPPET + 4) != 0 { return; }
    unsafe { core::arch::asm!("dmb sy", options(nostack, preserves_flags)); }
    let start = read(RAW_LOW);
    let (value, status, mask): (u32, u32, u32);
    unsafe { core::arch::asm!("mrs {mask}, PRIMASK", "cpsid i",
        "ldrex {v}, [{p}]", "strex {s}, {owner}, [{p}]", "dmb sy", "msr PRIMASK, {mask}",
        p = in(reg) EXAC, owner = in(reg) 0u32, v = out(reg) value,
        s = out(reg) status, mask = out(reg) mask, options(nostack, preserves_flags)); }
    let normal = read(EXAC);
    let after = exclusive_read();
    // If the challenger stole the word, release it even if C1 has timed out.
    if after == 0 { write(EXAC, 15); }
    write(SNIPPET + 32, value);
    write(SNIPPET + 36, status);
    write(SNIPPET + 40, normal);
    write(SNIPPET + 44, after);
    write(SNIPPET + 48, read(RAW_LOW).wrapping_sub(start));
    barrier();
    write(SNIPPET + 4, 1);
}

/// v6: DONE-gated single-core semantics, then C1-owner/C0-challenger trial.
/// C1 waits at most 10ms/100000 polls; C0 performs one attempt and never waits.
#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
fn lib_probes() {
    const DONE: usize = 0x4001_4018;
    lput(16, read(DONE));
    lput(18, read(0x4001_4000));
    if lget(16) & 64 == 0 {
        write(0x4001_7000, 64); // only hypothesized EXAC reset bit6, CLR alias
        barrier();
    }
    for _ in 0..1024 {
        if read(DONE) & 64 != 0 { break; }
    }
    lput(17, read(DONE));
    lput(31, 1); // DONE failure: no EXAC access
    if lget(17) & 64 != 0 {
        lput(19, read(EXAC));
        lput(20, exclusive_read());
        let original = lget(20);
        lput(31, 2); // unexpected owner encoding: no EXAC writes
        if original <= 15 {
            // Word7 has no firmware user; reset need not initialize it to free.
            write(EXAC, 15);
            barrier();
            let (_, status) = attempt(1);
            lput(21, status);
            lput(22, read(EXAC));
            lput(23, exclusive_read());
            let status;
            unsafe { core::arch::asm!("clrex", "strex {s}, {owner}, [{p}]", "dmb sy",
                p = in(reg) EXAC, owner = in(reg) 0u32, s = out(reg) status,
                options(nostack, preserves_flags)); }
            lput(24, status);
            lput(25, read(EXAC));
            lput(26, exclusive_read());
            write(EXAC, 15);
            barrier();
            lput(27, read(EXAC));
            lput(28, exclusive_read());
            let (value, status) = attempt(1);
            write(SNIPPET + 8, value);
            write(SNIPPET + 12, status);
            write(SNIPPET + 16, read(EXAC));
            let held = exclusive_read();
            write(SNIPPET + 20, held);
            lput(31, 3); // could not establish C1 ownership
            if held == 1 {
                barrier();
                write(SNIPPET, 1);
                let start = read(RAW_LOW);
                for _ in 0..100_000 {
                    if read(SNIPPET + 4) == 1 { break; }
                    if read(RAW_LOW).wrapping_sub(start) >= 10_000 { break; }
                }
                unsafe { core::arch::asm!("dmb sy", options(nostack, preserves_flags)); }
                lput(31, if read(SNIPPET + 4) == 1 { 0 } else { 4 });
                write(SNIPPET, 0); // withdraw even on timeout; C0 cleans a late steal
                write(SNIPPET + 24, read(EXAC));
                write(SNIPPET + 28, exclusive_read());
            }
            write(EXAC, original);
            barrier();
            lput(29, exclusive_read());
        }
    }
    barrier();
    lput(30, u32::from_le_bytes(*b"EXA6"));
}

#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(always)]
fn barrier() {
    unsafe { core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags)); }
}

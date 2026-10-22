//! Opt-in C1 period-loop profiler (`freertos-amp-proc1-profile`).
//!
//! The RP1 Cortex-M3 has no DWT cycle counter (DWT NOCYCCNT=1), so C1's own
//! SysTick is used as a free-running core-clock down counter with TICKINT=0.
//! It is only taken when it reads idle (CSR/RVR zero); otherwise every hook is
//! a no-op and STATE records the refusal. Results live in raw SRAM
//! 0x2000f600..0x2000f800 (Linux: BAR2 + 0xf600), outside every linked
//! section, between the MSP top (0x2000f000) and the RTOS record (0x2000f800).
const BASE: usize = 0x2000_f700; // 64 published words, Linux BAR2 + 0xf700
const LOCAL: usize = 0x2000_f600; // C1-private scratch between profile code and results
const SYST_CSR: usize = 0xe000_e010;
const SYST_RVR: usize = 0xe000_e014;
const SYST_CVR: usize = 0xe000_e018;
const SYST_CALIB: usize = 0xe000_e01c;
const RAW_HIGH: usize = 0x400a_c024;
const RAW_LOW: usize = 0x400a_c028;
const GPIO_STATUS: usize = 0x400e_0000;
const SHARED_SRAM: usize = 0x2000_ba00; // RP1_AMP_PROC1_STATUS word 0 (read only here)
const PPB_ISER0: usize = 0xe000_e100;
const MASK: u32 = 0x00ff_ffff;
pub(super) const SEGMENTS: usize = 11;

// Published words (u32). Probes are (min, max) over PROBE_N samples.
const MAGIC: usize = 0; // "PRF1" written last
const VERSION: usize = 1;
const STATE: usize = 2; // 1 probing, 2 loop live, 0xbad0 SysTick not idle
const CSR_BEFORE: usize = 3;
const RVR_BEFORE: usize = 4;
const CSR_AFTER: usize = 5;
const CALIB: usize = 6;
const CYCLES_10MS: usize = 7;
const CYCLES_PER_US: usize = 8;
const PROBES: usize = 9; // 8 probes x 2 words = 9..24
const PROBE_N: u32 = 64;
const ITERATIONS: usize = 25;
const TOTAL_MAX: usize = 26;
const SLACK_MIN: usize = 27;
const OVERRUNS: usize = 28; // iteration work >= 200 us
const LATE_MAX_US: usize = 29;
const LATE_SUM_US: usize = 30;
const WORST_CYCLE: usize = 31;
const WORST_LATE_US: usize = 32;
const LATE_COUNT: usize = 33; // deadline detected >= 1 us late
const SEG_MAX: usize = 34; // 34..44
const WORST: usize = 45; // 45..55 breakdown of the slowest iteration
const HIST: usize = 56; // 56..63 work buckets <10,<20,<40,<80,<120,<160,<200,>=200 us
const WORDS: usize = 64;
// C1-private scratch words.
const PREV: usize = 0;
const END: usize = 1;
const LATE_NOW: usize = 2;
const SINK: usize = 3;
const SCRATCH: usize = 4; // 4..14

#[inline(always)]
fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
#[inline(always)]
fn write(address: usize, value: u32) { unsafe { (address as *mut u32).write_volatile(value) } }
#[inline(always)]
fn get(i: usize) -> u32 { read(BASE + 4 * i) }
#[inline(always)]
fn lget(i: usize) -> u32 { read(LOCAL + 4 * i) }
#[inline(always)]
fn lput(i: usize, value: u32) { write(LOCAL + 4 * i, value) }
#[inline(always)]
fn put(i: usize, value: u32) { write(BASE + 4 * i, value) }
#[inline(always)]
fn cycles() -> u32 { read(SYST_CVR) }
/// SysTick counts down: `earlier - later`, modulo 24 bits.
#[inline(always)]
fn elapsed(earlier: u32, later: u32) -> u32 { earlier.wrapping_sub(later) & MASK }
#[inline(always)]
fn live() -> bool { get(STATE) == 2 }

#[inline(always)]
fn measure(slot: usize, f: impl Fn()) {
    let (mut min, mut max, mut sum) = (u32::MAX, 0u32, 0u32);
    for _ in 0..PROBE_N {
        let a = cycles();
        f();
        let d = elapsed(a, cycles());
        min = min.min(d);
        max = max.max(d);
        sum = sum.wrapping_add(d);
    }
    let _ = sum;
    put(slot, min);
    put(slot + 1, max);
}

/// Runs once on C1 before READY. Bounded: ~10 ms rate check + 8x64 samples.
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn probe() -> bool {
    for i in 0..WORDS { put(i, 0); }
    for i in 0..16 { lput(i, 0); }
    put(VERSION, 1);
    put(STATE, 1);
    let (csr, rvr) = (read(SYST_CSR), read(SYST_RVR));
    put(CSR_BEFORE, csr);
    put(RVR_BEFORE, rvr);
    if csr != 0 || rvr != 0 {
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
    put(CALIB, read(SYST_CALIB));
    let t0 = read(RAW_LOW);
    let c0 = cycles();
    let mut guard = 0u32;
    while read(RAW_LOW).wrapping_sub(t0) < 10_000 {
        guard = guard.wrapping_add(1);
        if guard > 50_000_000 { break; }
    }
    let c10 = elapsed(c0, cycles());
    put(CYCLES_10MS, c10);
    let per_us = (c10 / 10_000).max(1);
    put(CYCLES_PER_US, per_us);
    measure(PROBES, || {});
    measure(PROBES + 2, || { let _ = read(RAW_LOW); });
    measure(PROBES + 4, || {
        for _ in 0..4 {
            let hi = read(RAW_HIGH);
            let _ = read(RAW_LOW);
            if hi == read(RAW_HIGH) { break; }
        }
    });
    measure(PROBES + 6, || { let _ = read(GPIO_STATUS); });
    measure(PROBES + 8, || { let _ = read(SHARED_SRAM); });
    measure(PROBES + 10, || unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); });
    measure(PROBES + 12, || {
        lput(SINK, 1);
        unsafe { core::arch::asm!("dmb sy", options(nostack, preserves_flags)); }
    });
    measure(PROBES + 14, || { let _ = read(PPB_ISER0); });
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
    if get(ITERATIONS) != 0 {
        let slack = elapsed(lget(END), now);
        if slack < get(SLACK_MIN) { put(SLACK_MIN, slack); }
    }
    lput(PREV, now);
    lput(LATE_NOW, late_us);
}

#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn mark(segment: usize) {
    if !live() || segment >= SEGMENTS { return; }
    let now = cycles();
    lput(SCRATCH + segment, elapsed(lget(PREV), now));
    lput(PREV, now);
}

/// Closes the iteration: last segment, maxima, worst breakdown, histograms.
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
pub(super) fn end(cycle: u32) {
    if !live() { return; }
    mark(SEGMENTS - 1);
    let mut total = 0u32;
    for s in 0..SEGMENTS {
        let d = lget(SCRATCH + s);
        total = total.wrapping_add(d);
        if d > get(SEG_MAX + s) { put(SEG_MAX + s, d); }
    }
    let late = lget(LATE_NOW);
    if total > get(TOTAL_MAX) {
        put(TOTAL_MAX, total);
        for s in 0..SEGMENTS { put(WORST + s, lget(SCRATCH + s)); }
        put(WORST_CYCLE, cycle);
        put(WORST_LATE_US, late);
    }
    let per_us = get(CYCLES_PER_US).max(1);
    let us = total / per_us;
    let bucket = [10, 20, 40, 80, 120, 160, 200].iter().position(|b| us < *b).unwrap_or(7);
    put(HIST + bucket, get(HIST + bucket).wrapping_add(1));
    if us >= 200 { put(OVERRUNS, get(OVERRUNS).wrapping_add(1)); }
    if late != 0 { put(LATE_COUNT, get(LATE_COUNT).wrapping_add(1)); }
    if late > get(LATE_MAX_US) { put(LATE_MAX_US, late); }
    put(LATE_SUM_US, get(LATE_SUM_US).wrapping_add(late));
    put(ITERATIONS, get(ITERATIONS).wrapping_add(1));
    lput(END, cycles());
}

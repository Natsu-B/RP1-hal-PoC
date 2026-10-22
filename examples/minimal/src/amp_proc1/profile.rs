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
const VERSION: usize = 1; // 3, or 6 for the no-hooks PIO probe
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
    // v6 uses the fixed 0x600-byte code budget for PIO instead of memory timing.
    #[cfg(not(feature = "freertos-amp-proc1-no-hooks"))]
    {
    // v3 memory probes. The same 16-NOP + `bx lr` snippet is written to C1's
    // proc-local ISRAM (Code region, 0x10000000) and to free shared SRAM
    // (0x2000f680, System region) and timed through the same indirect call.
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

/// v6, no-hooks only: G9 direct PIO, SM0, no GPIO instructions or pinmux/pad writes.
/// Only take an initially held block: instruction memory is write-only, so an
/// already released owner's program cannot be saved. Linux PIO must be disabled.
/// LOCAL 16..31; PIO1 magic is published after cleanup, status is word 31.
#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(never)]
#[unsafe(link_section = ".amp_proc1.profile_text")]
fn lib_probes() {
    const RESET: usize = 0x4001_4000;
    const BIT: u32 = 1 << 27; // PIO_APBS; relation to local PIO is still a hypothesis.
    const PIO: usize = 0xf000_0000;
    const SM: usize = PIO + 0xcc; // RP1 Version-1 layout, NOT RP2040's +0xc8.
    const LIMIT: u32 = 100_000;
    let reset = read(RESET);
    lput(16, reset);
    lput(17, read(RESET + 0x18));
    lput(26, u32::MAX); // Never confuse an unread FIFO with the expected 21.
    let mut status = 1; // Refused: reset was already released.
    if reset & BIT != 0 {
        write(RESET + 0x3000, BIT);
        barrier();
        status = 2; // Reset DONE timeout: no PIO access on this path.
        for _ in 0..LIMIT {
            if read(RESET + 0x18) & BIT != 0 { status = 3; break; }
        }
        lput(18, read(RESET + 0x18));
        if status == 3 {
            let ctrl = read(PIO);
            let fstat = read(PIO + 4);
            let level = read(PIO + 0xc);
            lput(19, ctrl);
            lput(20, fstat);
            lput(21, level);
            // Refuse any running/restarting SM or queued data, including other SMs.
            if ctrl & 0xfff == 0 && level == 0 && fstat == 0x0f00_0f00 {
                let div = read(SM);
                let exec = read(SM + 4);
                let shift = read(SM + 8);
                let addr = read(SM + 0xc);
                let pins = read(SM + 0x14);
                write(SM + 0x14, 0); // SIDESET_COUNT=0; program has no OUT/SET pins.
                write(SM, 1 << 16); // INT=1, FRAC=0: SM clock = clk_sys (official header).
                write(SM + 4, 3 << 12); // wrap 0..3; last instruction is a self-jump.
                write(SM + 8, 0); // No autopush/autopull or FIFO joining.
                write(PIO + 0x4c, 0xe035); // set x,21
                write(PIO + 0x50, 0xa0c1); // mov isr,x
                write(PIO + 0x54, 0x8020); // push block
                write(PIO + 0x58, 0x0003); // jmp 3: only one FIFO result
                write(PIO, 0x110); // Restart SM0 and its divider, keep all SMs disabled.
                write(SM + 0x10, 0); // Inject jmp 0 while disabled.
                barrier();
                lput(22, read(SM));
                status = 4; // Configuration readback mismatch; never enable.
                if read(SM) == 1 << 16 && read(SM + 4) == 3 << 12
                    && read(SM + 8) == 0 && read(SM + 0x14) == 0 && read(SM + 0xc) == 0 {
                    write(PIO, 1); // SM0 only; no processor IRQ enable is touched.
                    barrier();
                    status = 5; // RX FIFO timeout.
                    for _ in 0..LIMIT {
                        if read(PIO + 4) & (1 << 8) == 0 { status = 6; break; }
                    }
                    write(PIO, 0);
                    barrier();
                    lput(23, read(PIO + 4));
                    lput(24, read(PIO + 0xc));
                    lput(25, read(SM + 0xc));
                    if status == 6 {
                        let value = read(PIO + 0x24); // Pop only after RXEMPTY cleared.
                        lput(26, value);
                        lput(27, read(PIO + 4));
                        if value == 21 && lget(24) == 0x10 && lget(25) == 3 { status = 0; }
                    }
                }
                write(PIO, 0);
                write(SM + 0x10, 0xe020); // Restore X=0 in the initially reset SM.
                write(PIO, 0x110); // Clear ISR/counters/stall and divider phase.
                write(SM + 8, 1 << 31); // Flush only SM0's FIFOs, including a timed-out run.
                write(SM + 8, shift);
                write(SM, div);
                write(SM + 4, exec);
                write(SM + 0x10, addr); // jmp saved PC (0..31), no pin effects.
                write(SM + 0x14, pins);
                for i in 0..4 { write(PIO + 0x4c + i * 4, 0); }
                barrier();
                if read(PIO) != ctrl || read(SM) != div || read(SM + 4) != exec
                    || read(SM + 8) != shift || read(SM + 0xc) != addr
                    || read(SM + 0x14) != pins || read(PIO + 0xc) != 0 {
                    status |= 0x100; // Cleanup mismatch preserves the original failure code.
                }
            }
        }
        write(RESET + 0x2000, BIT); // Restore the original held bit only.
        barrier();
    }
    lput(28, read(RESET));
    lput(29, read(RESET + 0x18));
    if (lget(28) ^ reset) & BIT != 0 { status |= 0x200; }
    lput(31, status);
    barrier();
    lput(30, u32::from_le_bytes(*b"PIO1"));
}

#[cfg(feature = "freertos-amp-proc1-no-hooks")]
#[inline(always)]
fn barrier() {
    unsafe { core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags)); }
}

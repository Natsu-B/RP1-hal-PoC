#![cfg_attr(not(target_arch = "arm"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "arm")]
use rp1_hal::prelude::*;

const WINDOW_US: u64 = 120_000_000;
const PERIOD_US: u64 = 100_000;
const GPIO_PERIOD_TICKS: u32 = 100;
#[cfg(target_arch = "arm")]
const GPIO_PULSE_US: u64 = 1_000;
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const DBI_MONITOR_WINDOW_US: u64 = 30_000_000;
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const DBI_POLL_US: u64 = 1_000;
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const DBI_RECORD_LIMIT: u8 = 32;

#[cfg(target_arch = "arm")]
const CLK_UART_CTRL: usize = 0x4001_8054;
#[cfg(target_arch = "arm")]
const CLK_UART_DIV_INT: usize = 0x4001_8058;
#[cfg(target_arch = "arm")]
const CLK_UART_SEL: usize = 0x4001_8060;
#[cfg(target_arch = "arm")]
const RESET_DONE1: usize = 0x4001_401c;
#[cfg(target_arch = "arm")]
const PLL_SYS_CS: usize = 0x4002_0000;
#[cfg(target_arch = "arm")]
const PLL_SYS_PRIM: usize = 0x4002_0010;
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const PCIE_VIEWPORT_SELECTOR: usize = 0x4010_8000;
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const PCIE_DBI_WINDOW: usize = 0x4010_9000;
// Plain reads in rp1-regdb pcie-apbs.toml: MONITOR2/INTR/INTE/INTS.
// INTR acknowledgement is a separate W1C write; this observer never performs it.
// Do not add the LTSSM FIFO at +0x124: reading it consumes entries.
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const PCIE_APBS_READS: [usize; 4] = [0x4010_81a4, 0x4010_81a8, 0x4010_81ac, 0x4010_81b4];

#[cfg(target_arch = "arm")]
const CLK_UART_ENABLE: u32 = 1 << 11;
#[cfg(target_arch = "arm")]
const CLK_UART_RELEVANT: u32 = 0x0000_0fe0;
#[cfg(target_arch = "arm")]
const CLK_UART_XOSC_ENABLED: u32 = 0x0000_0840;
#[cfg(target_arch = "arm")]
const UART0_RESET_DONE: u32 = 1 << 26;
#[cfg(target_arch = "arm")]
const PLL_SYS_LOCKED: u32 = 0x8000_0001;
#[cfg(target_arch = "arm")]
const PLL_SYS_PRI_PH_ENABLED: u32 = 1 << 4;

const HEX: &[u8; 16] = b"0123456789abcdef";
const HEARTBEAT_TEMPLATE: [u8; 54] = *b"RP1CLK seq=0x00000000 ctrl=0x00000000 off=0x00000000\r\n";
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const DBI_LINE_TEMPLATE: [u8; 270] = *b"RP1DBI event=INIT elapsed_us=0x0000000000000000 valid=0 sel0=0x00000000 sel1=0x00000000 id=0x00000000 cmdstat=0x00000000 classrev=0x00000000 bhlc=0x00000000 bar0=0x00000000 bar1=0x00000000 bar2=0x00000000 mon2=0x00000000 intr=0x00000000 inte=0x00000000 ints=0x00000000\r\n";
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
const DBI_DWORD_OFFSETS: [usize; 7] = [93, 112, 132, 148, 164, 180, 196];

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DbiSample {
    valid: bool,
    sel0: u32,
    sel1: u32,
    dwords: [u32; 7],
    apbs: [u32; 4],
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
impl DbiSample {
    const fn invalid(sel0: u32, sel1: u32) -> Self {
        Self {
            valid: false,
            sel0,
            sel1,
            dwords: [0; 7],
            apbs: [0; 4],
        }
    }
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DbiEvent {
    Initial,
    Change,
    Cap,
    End,
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
impl DbiEvent {
    const fn token(self) -> [u8; 4] {
        match self {
            Self::Initial => *b"INIT",
            Self::Change => *b"CHG ",
            Self::Cap => *b"CAP ",
            Self::End => *b"END ",
        }
    }
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
pub(crate) struct DbiMonitor {
    last: Option<DbiSample>,
    records: u8,
    capped: bool,
    ended: bool,
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
impl DbiMonitor {
    pub(crate) const fn new() -> Self {
        Self {
            last: None,
            records: 0,
            capped: false,
            ended: false,
        }
    }

    fn observe(&mut self, sample: DbiSample) -> Option<DbiEvent> {
        // Traffic/debug signals are still reported, but must not exhaust the
        // bounded record budget before a reset-level transition is observed.
        if self.last == Some(sample) || (cfg!(feature = "freertos-endpoint-uart")
            && self.last.is_some_and(|last| last.valid == sample.valid
            && last.sel0 == sample.sel0 && last.sel1 == sample.sel1
            && last.dwords == sample.dwords && last.apbs[1..] == sample.apbs[1..]
            && (last.apbs[0] ^ sample.apbs[0]) & 0x001f_0000 == 0)) {
            return None;
        }

        let event = if self.last.is_none() {
            DbiEvent::Initial
        } else {
            DbiEvent::Change
        };
        self.last = Some(sample);

        if self.records < DBI_RECORD_LIMIT {
            self.records += 1;
            Some(event)
        } else if !self.capped {
            self.capped = true;
            Some(DbiEvent::Cap)
        } else {
            None
        }
    }

    fn finish(&mut self) -> Option<DbiEvent> {
        if self.ended {
            None
        } else {
            self.ended = true;
            Some(DbiEvent::End)
        }
    }
}

#[cfg(feature = "freertos-endpoint-uart")]
pub(crate) mod endpoint_gate {
    use super::*;

    const WINDOW: u32 = 60_000_000;
    const BUDGET: u32 = 5_000;
    const MAX_GAP: u32 = 2_500;
    const PERSTN: u32 = 1 << 17;
    const CORE_ALIVE: u32 = 1 << 16;
    const LEVELS: u32 = 0x001f_0000;
    const RO_WR: usize = PCIE_DBI_WINDOW + 0x8bc;

    #[derive(Clone, Copy)]
    struct Sample {
        dbi: DbiSample,
        before: u32,
        after: u32,
        selector: u32,
        levels: u32,
        ro: u32,
        reads: u32,
    }

    fn read(mut read: impl FnMut(usize) -> u32, mut clock: impl FnMut() -> u32) -> Sample {
        // Before the first MONITOR2 read, not after detecting a low level.
        // This conservatively also covers the final MONITOR2 read if both are low.
        let before = clock();
        let dbi = read_dbi_sample(&mut read);
        let mut reads = if dbi.sel0 == 0 { 0x1fff } else { 0x101f };
        let ro = if dbi.valid && dbi.apbs[0] & (PERSTN | CORE_ALIVE) == PERSTN | CORE_ALIVE {
            reads |= 1 << 13; read(RO_WR)
        } else { 0 };
        let selector = read(PCIE_VIEWPORT_SELECTOR);
        let levels = read(PCIE_APBS_READS[0]);
        reads |= 3 << 14;
        let after = clock(); // After the last high-level read, never before it.
        Sample { dbi, before, after, selector, levels, ro, reads }
    }

    // Numeric wire codes; only Candidate is positive, and it grants NO writes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u32)]
    enum Result { Candidate = 1, Expired, Gap, Deadline, Reset, Selector, Levels, Tuple, RoEnabled }

    pub(crate) struct Gate {
        epoch: u32,
        low: Option<u32>,
        high: Option<u32>,
        done: bool,
    }

    impl Gate {
        pub(crate) const fn new(epoch: u32) -> Self {
            Self { epoch, low: None, high: None, done: false }
        }

        fn observe(&mut self, s: Sample) -> Option<Result> {
            if self.done { return None; }
            let result = if s.after.wrapping_sub(self.epoch) >= WINDOW {
                Result::Expired
            } else if self.low.is_none() && s.dbi.apbs[0] & s.levels & PERSTN != 0 {
                return None; // Initial configured/high boot is not the reset of interest.
            } else if self.high.is_some() && s.dbi.apbs[0] & s.levels & PERSTN == 0 {
                Result::Reset
            } else if !s.dbi.valid || s.selector != 0 {
                Result::Selector
            } else if (s.dbi.apbs[0] ^ s.levels) & LEVELS != 0 {
                Result::Levels
            } else if s.levels & PERSTN == 0 {
                self.low = Some(s.before);
                return None;
            } else {
                let low = self.low.unwrap(); // A stable low is required above.
                let high = *self.high.get_or_insert(s.after);
                if high.wrapping_sub(low) > MAX_GAP {
                    Result::Gap
                } else if s.after.wrapping_sub(low) >= BUDGET {
                    Result::Deadline
                } else if s.levels & CORE_ALIVE == 0 {
                    return None; // Keep the SAME low origin while awaiting CORE_ALIVE.
                } else if s.dbi.dwords[0] != 0x0001_1de4 || s.dbi.dwords[1] & 6 != 0
                    || s.dbi.dwords[2] != 2 || s.dbi.dwords[4..] != [0, 0, 0] {
                    Result::Tuple
                } else if s.reads != 0xffff || s.ro & 1 != 0 {
                    Result::RoEnabled
                } else {
                    Result::Candidate
                }
            };
            self.done = true; // One terminal result; elapsed-counter wrap cannot rearm it.
            Some(result)
        }
    }

    const LINE: &[u8] = b"RP1GATE code=0x00000000 before=0x00000000 after=0x00000000 low=0x00000000 high=0x00000000 gap=0x00000000 reads=0x00000000 ro=0x00000000 sel=0x00000000 mon0=0x00000000 mon1=0x00000000 budget=0x00001388 maxgap=0x000009c4\r\n";

    /// Actual endpoint path: classify the measured sample BEFORE any UART callback.
    /// Legacy RP1DBI sample/format/trigger semantics remain independent of the gate.
    pub(crate) fn sample(monitor: &mut DbiMonitor, gate: &mut Gate, elapsed_us: u64,
        read32: impl FnMut(usize) -> u32, clock: impl FnMut() -> u32, mut emit: impl FnMut(&[u8])) {
        if gate.done {
            let s = read_dbi_sample(read32); // No extra gate reads after its one-shot terminal result.
            if let Some(event) = monitor.observe(s) { emit(&dbi_line(event, elapsed_us, s)); }
            return;
        }
        let s = read(read32, clock);
        if let Some(result) = gate.observe(s) {
            let mut line = [0; LINE.len()];
            line.copy_from_slice(LINE);
            let low = gate.low.unwrap_or(0);
            let high = gate.high.unwrap_or(0);
            let gap = if gate.high.is_some() { high.wrapping_sub(low) } else { 0 };
            for (offset, value) in [15, 33, 50, 65, 81, 96, 113, 127, 142, 158, 174]
                .into_iter().zip([result as u32, s.before, s.after, low, high, gap,
                    s.reads, s.ro, s.selector, s.dbi.apbs[0], s.levels]) {
                encode_hex_u32(&mut line, offset, value);
            }
            emit(&line);
        }
        if let Some(event) = monitor.observe(s.dbi) {
            emit(&dbi_line(event, elapsed_us, s.dbi));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn sample_at(before: u32, after: u32, levels: u32) -> Sample {
            Sample { dbi: DbiSample { valid: true, sel0: 0, sel1: 0,
                dwords: [0x0001_1de4, 0, 2, 0, 0, 0, 0], apbs: [levels, 0, 0, 0] },
                before, after, selector: 0, levels, ro: 0, reads: 0xffff }
        }

        #[test]
        fn actual_gate_wrap_deadlines_and_rejections() {
            let low = sample_at(100, 200, 0);
            let high = sample_at(1000, 1100, PERSTN | CORE_ALIVE | (1 << 20));
            let fresh = || { let mut g = Gate::new(0); assert_eq!(g.observe(low), None); g };
            let mut g = Gate::new(0);
            assert_eq!(g.observe(high), None); // Configured initial high doesn't consume it.
            assert_eq!(g.observe(low), None);
            assert_eq!(g.observe(high), Some(Result::Candidate)); // Early link-up is allowed.
            assert_eq!(g.observe(low), None); // No second reset, or timer wrap, can rearm.
            assert_eq!(g.observe(high), None);

            let mut g = Gate::new(u32::MAX - 1000);
            assert_eq!(g.observe(sample_at(u32::MAX - 100, u32::MAX - 50, 0)), None);
            assert_eq!(g.observe(sample_at(50, 100, PERSTN | CORE_ALIVE)), Some(Result::Candidate));
            assert_eq!(g.high.unwrap().wrapping_sub(g.low.unwrap()), 201);
            assert_eq!(fresh().observe(sample_at(2599, 2600, high.levels)), Some(Result::Candidate));
            assert_eq!(fresh().observe(sample_at(2599, 2601, high.levels)), Some(Result::Gap));
            // Using after-low or before-high would incorrectly admit this stale low.
            let mut g = Gate::new(0);
            assert_eq!(g.observe(sample_at(100, 2400, 0)), None);
            assert_eq!(g.observe(sample_at(2500, 2700, high.levels)), Some(Result::Gap));
            for (at, result) in [(5099, Result::Candidate), (5100, Result::Deadline)] {
                let mut g = fresh();
                assert_eq!(g.observe(sample_at(1000, 1100, PERSTN)), None);
                assert_eq!(g.observe(sample_at(at - 1, at, high.levels)), Some(result));
                assert_eq!((g.low, g.high), (Some(100), Some(1100)));
            }
            let mut g = fresh();
            assert_eq!(g.observe(sample_at(1000, 1100, PERSTN)), None);
            assert_eq!(g.observe(low), Some(Result::Reset));
            assert_eq!(g.observe(high), None);
            let mut g = fresh();
            assert_eq!(g.observe(sample_at(WINDOW - 1, WINDOW, 0)), Some(Result::Expired));
            assert_eq!(g.observe(low), None);
            assert_eq!(g.observe(high), None);
            let mut wrong = high;
            wrong.selector = 1;
            assert_eq!(fresh().observe(wrong), Some(Result::Selector));
            wrong = high; wrong.dbi.valid = false;
            assert_eq!(fresh().observe(wrong), Some(Result::Selector));
            wrong = high; wrong.levels ^= 1 << 20;
            assert_eq!(fresh().observe(wrong), Some(Result::Levels));
            for (index, value) in [(0, 0), (1, 2), (1, 4), (2, 0x0200_0002), (4, 1), (5, 1), (6, 1)] {
                wrong = high; wrong.dbi.dwords[index] = value;
                assert_eq!(fresh().observe(wrong), Some(Result::Tuple));
            }
            wrong = high; wrong.ro = 1;
            assert_eq!(fresh().observe(wrong), Some(Result::RoEnabled));
        }

        #[test]
        fn actual_sample_read_order_mask_and_uart_boundary() {
            use core::cell::Cell;
            let addresses = core::cell::RefCell::new(Vec::new());
            let time = Cell::new(100);
            let s = read(|addr| {
                addresses.borrow_mut().push(addr);
                if addr == PCIE_APBS_READS[0] { PERSTN | CORE_ALIVE } else { 0 }
            }, || {
                let n = addresses.borrow().len();
                assert!(n == 0 || n == 16); // Clock brackets ALL actual MMIO reads.
                let now = time.get(); time.set(now + 100); now
            });
            assert_eq!((s.before, s.after, s.reads), (100, 200, 0xffff));
            assert_eq!(&addresses.borrow()[12..], &[PCIE_VIEWPORT_SELECTOR, RO_WR,
                PCIE_VIEWPORT_SELECTOR, PCIE_APBS_READS[0]]);
            let s = read(|addr| if addr == PCIE_VIEWPORT_SELECTOR { 1 } else { 0 }, || 0);
            assert_eq!(s.reads, 0xd01f); // No DBI/RO reads while selector is nonzero.
            for levels in [0, PERSTN, CORE_ALIVE] {
                let s = read(|addr| {
                    assert_ne!(addr, RO_WR); // New register not touched before CORE_ALIVE+PERSTN.
                    if addr == PCIE_APBS_READS[0] { levels } else { 0 }
                }, || 0);
                assert_eq!(s.reads, 0xdfff);
            }
            let mut g = Gate::new(0);
            let mut m = DbiMonitor::new();
            let mut lines = Vec::new();
            // Actual low sample is consumed before the UART callback delays the next sample.
            sample(&mut m, &mut g, 0, |_| 0, || time.get(), |line| {
                lines.push(line.to_vec()); time.set(10_000);
            });
            assert_eq!(g.low, Some(300));
            sample(&mut m, &mut g, 10_000, |addr| match addr {
                a if a == PCIE_APBS_READS[0] => PERSTN | CORE_ALIVE,
                a if a == PCIE_DBI_WINDOW => 0x0001_1de4,
                a if a == PCIE_DBI_WINDOW + 8 => 2,
                _ => 0,
            }, || time.get(), |line| lines.push(line.to_vec()));
            assert!(g.done);
            let line = &lines[1];
            assert_eq!(line, b"RP1GATE code=0x00000003 before=0x00002710 after=0x00002710 low=0x0000012c high=0x00002710 gap=0x000025e4 reads=0x0000ffff ro=0x00000000 sel=0x00000000 mon0=0x00030000 mon1=0x00030000 budget=0x00001388 maxgap=0x000009c4\r\n");
            let mut reads = Vec::new();
            sample(&mut m, &mut g, 20_000, |addr| { reads.push(addr); 0 },
                || panic!("terminal gate must not sample time or rearm"), |_| ());
            assert_eq!(reads.len(), 13); // Original observer only after terminal output.
            assert!(!reads.contains(&RO_WR));
        }
    }
}

/// One monitor period, using wrap-safe RTOS tick subtraction (period << 2^31).
#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
pub(crate) fn endpoint_wait_remaining(start: u32, now: u32) -> u32 {
    1000u32.saturating_sub(now.wrapping_sub(start))
}

/// Actual monitor scheduling body, host-testable without pretending to run an RTOS.
#[cfg(feature = "freertos-endpoint-uart")]
pub(crate) fn endpoint_wait_period(fast: &mut bool, epoch: u32,
    mut tick: impl FnMut() -> u32, mut sleep: impl FnMut(u32), mut sample: impl FnMut()) {
    let start = tick();
    sample();
    for _ in 0..1000 {
        let now = tick();
        let remaining = endpoint_wait_remaining(start, now);
        if remaining == 0 { break; }
        *fast &= now.wrapping_sub(epoch) < 60_000; // one-way expiry, includes bookkeeping
        sleep(if *fast { 1 } else { remaining });
        if endpoint_wait_remaining(start, tick()) == 0 { break; }
        sample();
    }
}

fn encode_hex_u32(out: &mut [u8], offset: usize, value: u32) {
    for index in 0..8 {
        let shift = 28 - index * 4;
        out[offset + index] = HEX[((value >> shift) & 0xf) as usize];
    }
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
fn encode_hex_u64(out: &mut [u8], offset: usize, value: u64) {
    for index in 0..16 {
        let shift = 60 - index * 4;
        out[offset + index] = HEX[((value >> shift) & 0xf) as usize];
    }
}

fn heartbeat_line(sequence: u32, ctrl: u32, off_periods: u32) -> [u8; HEARTBEAT_TEMPLATE.len()] {
    let mut line = HEARTBEAT_TEMPLATE;
    encode_hex_u32(&mut line, 13, sequence);
    encode_hex_u32(&mut line, 29, ctrl);
    encode_hex_u32(&mut line, 44, off_periods);
    line
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
fn dbi_line(event: DbiEvent, elapsed_us: u64, sample: DbiSample) -> [u8; DBI_LINE_TEMPLATE.len()] {
    let mut line = DBI_LINE_TEMPLATE;
    line[13..17].copy_from_slice(&event.token());
    encode_hex_u64(&mut line, 31, elapsed_us);
    line[54] = if sample.valid { b'1' } else { b'0' };
    encode_hex_u32(&mut line, 63, sample.sel0);
    encode_hex_u32(&mut line, 79, sample.sel1);
    for (offset, value) in DBI_DWORD_OFFSETS.into_iter().zip(sample.dwords) {
        encode_hex_u32(&mut line, offset, value);
    }
    for (offset, value) in [212, 228, 244, 260].into_iter().zip(sample.apbs) {
        encode_hex_u32(&mut line, offset, value);
    }
    line
}

const fn tick_due(elapsed_us: u64, next_us: u64) -> bool {
    elapsed_us < WINDOW_US && elapsed_us >= next_us
}

const fn next_deadline(elapsed_us: u64) -> u64 {
    (elapsed_us / PERIOD_US + 1) * PERIOD_US
}

const fn gpio_marker_due(sequence: u32) -> bool {
    sequence % GPIO_PERIOD_TICKS == 0
}

#[cfg(target_arch = "arm")]
#[inline(always)]
fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

#[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
fn read_dbi_sample(mut read32: impl FnMut(usize) -> u32) -> DbiSample {
    // Selector-independent observations remain useful when the DBI bank is
    // ambiguous. Sequential reads are not an atomic event/level snapshot.
    let apbs = PCIE_APBS_READS.map(&mut read32);
    let sel0 = read32(PCIE_VIEWPORT_SELECTOR);
    if sel0 != 0 {
        // A non-zero selector makes the DBI window ambiguous; do not touch it.
        let sel1 = read32(PCIE_VIEWPORT_SELECTOR);
        return DbiSample { apbs, ..DbiSample::invalid(sel0, sel1) };
    }

    let dwords = [
        read32(PCIE_DBI_WINDOW),
        read32(PCIE_DBI_WINDOW + 0x04),
        read32(PCIE_DBI_WINDOW + 0x08),
        read32(PCIE_DBI_WINDOW + 0x0c),
        read32(PCIE_DBI_WINDOW + 0x10),
        read32(PCIE_DBI_WINDOW + 0x14),
        read32(PCIE_DBI_WINDOW + 0x18),
    ];
    let sel1 = read32(PCIE_VIEWPORT_SELECTOR);
    DbiSample {
        valid: sel0 == 0 && sel1 == 0,
        sel0,
        sel1,
        dwords,
        apbs,
    }
}

#[cfg(target_arch = "arm")]
fn contract_ready() -> bool {
    read32(CLK_UART_CTRL) & CLK_UART_RELEVANT == CLK_UART_XOSC_ENABLED
        && read32(CLK_UART_DIV_INT) == 1
        && read32(CLK_UART_SEL) == 1
        && read32(RESET_DONE1) & UART0_RESET_DONE != 0
        && read32(PLL_SYS_CS) == PLL_SYS_LOCKED
        && read32(PLL_SYS_PRIM) & PLL_SYS_PRI_PH_ENABLED != 0
}

#[cfg(target_arch = "arm")]
pub fn run(gpio22: &mut ConfiguredPin<22, Output>, timer: &RawTimer, uart0: Uart0) -> ! {
    if !contract_ready() {
        gpio22.set_high();
        timer.delay_us(20_000);
        gpio22.set_low();
        stop();
    }

    let mut uart = uart0.init_tx_115200_clock_ready();
    let start = timer.now();
    let mut next_us = 0;
    let mut off_periods = 0u32;
    #[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
    let mut dbi_monitor = DbiMonitor::new();
    #[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
    let mut dbi_next_us = 0;
    // 1 ms is a best-effort target; synchronous UART records/heartbeats create gaps.

    // From here to completion, clock/reset/UART configuration is read-only.
    // write_bytes() only performs bounded UART data-register writes.
    loop {
        let elapsed_us = timer.elapsed_since(start);
        if elapsed_us >= WINDOW_US {
            gpio22.set_low();
            stop();
        }
        #[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
        {
            if elapsed_us >= DBI_MONITOR_WINDOW_US {
                if let Some(event) = dbi_monitor.finish() {
                    let sample = dbi_monitor
                        .last
                        .unwrap_or(DbiSample::invalid(u32::MAX, u32::MAX));
                    let line = dbi_line(event, elapsed_us, sample);
                    if uart.write_bytes(&line) != line.len() {
                        stop();
                    }
                }
            } else if elapsed_us >= dbi_next_us {
                let sample = read_dbi_sample(read32);
                if let Some(event) = dbi_monitor.observe(sample) {
                    let line = dbi_line(event, elapsed_us, sample);
                    if uart.write_bytes(&line) != line.len() {
                        stop();
                    }
                }
                dbi_next_us = (elapsed_us / DBI_POLL_US + 1) * DBI_POLL_US;
            }
        }
        if tick_due(elapsed_us, next_us) {
            let sequence = (elapsed_us / PERIOD_US) as u32;
            if gpio_marker_due(sequence) {
                gpio22.set_high();
                timer.delay_us(GPIO_PULSE_US);
                gpio22.set_low();
            }

            let ctrl = read32(CLK_UART_CTRL);
            if ctrl & CLK_UART_ENABLE != 0 {
                let line = heartbeat_line(sequence, ctrl, off_periods);
                let _ = uart.write_bytes(&line);
            } else {
                off_periods = off_periods.wrapping_add(1);
            }
            next_us = next_deadline(elapsed_us);
        }
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "arm")]
fn stop() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
    #[test]
    fn dbi_reads_only_allowlist_and_rejects_selector_drift() {
        let mut addresses = Vec::new();
        let sample = read_dbi_sample(|address| {
            addresses.push(address);
            if address == PCIE_VIEWPORT_SELECTOR { 0 } else { address as u32 }
        });
        assert!(sample.valid);
        assert_eq!(sample.apbs, PCIE_APBS_READS.map(|address| address as u32));
        let mut expected = PCIE_APBS_READS.to_vec();
        expected.push(PCIE_VIEWPORT_SELECTOR);
        expected.extend((0..7).map(|n| PCIE_DBI_WINDOW + n*4));
        expected.push(PCIE_VIEWPORT_SELECTOR);
        assert_eq!(addresses, expected);
        addresses.clear();
        let sample = read_dbi_sample(|address| { addresses.push(address); 1 });
        assert!(!sample.valid);
        let mut expected = PCIE_APBS_READS.to_vec();
        expected.extend([PCIE_VIEWPORT_SELECTOR; 2]);
        assert_eq!(addresses, expected);
        assert_eq!(sample.apbs, [1; 4]);
        let mut selectors = 0;
        let sample = read_dbi_sample(|address| {
            if address == PCIE_VIEWPORT_SELECTOR { selectors += 1; selectors - 1 } else { 0 }
        });
        assert!(!sample.valid); // A selector ABA still cannot be detected by two reads.
    }

    #[test]
    fn heartbeat_encoding_and_schedule_are_fixed() {
        assert_eq!(
            heartbeat_line(0x0123_abcd, 0x1000_0840, 0x55),
            *b"RP1CLK seq=0x0123abcd ctrl=0x10000840 off=0x00000055\r\n"
        );
        assert!(tick_due(0, 0));
        assert!(!tick_due(99_999, 100_000));
        assert_eq!(next_deadline(350_000), 400_000);
        assert!(gpio_marker_due(0));
        assert!(!gpio_marker_due(99));
        assert!(gpio_marker_due(100));
        assert!(!tick_due(WINDOW_US, WINDOW_US));
    }

    #[cfg(any(feature = "rp1-linux-pcie-dbi-transition-monitor", feature = "freertos-endpoint-uart"))]
    #[test]
    fn dbi_transition_monitor_is_change_only_bounded_and_fixed() {
        let mut monitor = DbiMonitor::new();
        let mut sample = DbiSample {
            valid: true,
            sel0: 0,
            sel1: 0,
            dwords: [0x2000_1927, 0, 0x0200_0000, 0, 0x0080_0000, 0, 0x0040_0000],
            apbs: [0; 4],
        };

        assert_eq!(monitor.observe(sample), Some(DbiEvent::Initial));
        assert_eq!(monitor.observe(sample), None);
        sample.dwords[1] = 1;
        assert_eq!(monitor.observe(sample), Some(DbiEvent::Change));
        let mut mismatch = DbiSample::invalid(0, 1);
        mismatch.dwords = sample.dwords;
        assert!(!mismatch.valid);
        assert_eq!(monitor.observe(mismatch), Some(DbiEvent::Change));
        for selector in 1..=29 {
            assert_eq!(
                monitor.observe(DbiSample::invalid(selector, selector)),
                Some(DbiEvent::Change)
            );
        }
        assert_eq!(
            monitor.observe(DbiSample::invalid(30, 30)),
            Some(DbiEvent::Cap)
        );
        assert_eq!(monitor.observe(DbiSample::invalid(30, 30)), None);
        assert_eq!(monitor.observe(DbiSample::invalid(31, 31)), None);
        assert_eq!(monitor.finish(), Some(DbiEvent::End));
        assert_eq!(monitor.finish(), None);

        let mut monitor = DbiMonitor::new();
        assert_eq!(monitor.observe(sample), Some(DbiEvent::Initial));
        sample.apbs[1] = 2;
        assert_eq!(monitor.observe(sample), Some(DbiEvent::Change));
        assert_eq!(monitor.observe(sample), None);

        #[cfg(feature = "freertos-endpoint-uart")]
        { sample.apbs[0] = 0x00e0_ffff; } // debug/traffic only
        assert_eq!(monitor.observe(sample), None);
        for bit in 16..=20 {
            sample.apbs[0] ^= 1 << bit;
            assert_eq!(monitor.observe(sample), Some(DbiEvent::Change));
        }
        assert_eq!(endpoint_wait_remaining(7, 7), 1000);
        assert_eq!(endpoint_wait_remaining(7, 1006), 1);
        assert_eq!(endpoint_wait_remaining(7, 1007), 0);
        assert_eq!(endpoint_wait_remaining(u32::MAX - 9, 990), 0);
        assert_eq!(endpoint_wait_remaining(u32::MAX - 9, 0), 990);
        assert_eq!(endpoint_wait_remaining(7, 1057), 0); // bounded TX overshoot

        let mut encoded = DbiSample::invalid(0x1111_2222, 0x3333_4444);
        encoded.dwords = [
            0xdead_beef,
            0x0001_0002,
            0x0200_0000,
            0x0001_0000,
            0x0080_0000,
            0,
            0x0040_0000,
        ];
        assert!(!encoded.valid);
        encoded.apbs = [0x0013_0000, 2, 3, 2];
        assert_eq!(
            dbi_line(DbiEvent::Cap, 0x0123_4567_89ab_cdef, encoded),
            *b"RP1DBI event=CAP  elapsed_us=0x0123456789abcdef valid=0 sel0=0x11112222 sel1=0x33334444 id=0xdeadbeef cmdstat=0x00010002 classrev=0x02000000 bhlc=0x00010000 bar0=0x00800000 bar1=0x00000000 bar2=0x00400000 mon2=0x00130000 intr=0x00000002 inte=0x00000003 ints=0x00000002\r\n"
        );
    }

    #[cfg(feature = "freertos-endpoint-uart")]
    #[test]
    fn endpoint_schedule_wrap_expiry_and_slow_periods() {
        use core::cell::Cell;
        let epoch = u32::MAX - 20;
        let tick = Cell::new(epoch);
        let mut fast = true;
        let mut samples = Vec::new();
        endpoint_wait_period(&mut fast, epoch, || tick.get(),
            |n| tick.set(tick.get().wrapping_add(n)), || samples.push(tick.get()));
        assert_eq!(samples.len(), 1000);
        assert!(samples.windows(2).all(|s| s[1].wrapping_sub(s[0]) == 1));
        assert_eq!(tick.get().wrapping_sub(epoch), 1000);
        tick.set(epoch.wrapping_add(59_999)); // includes time outside sampling windows
        samples.clear();
        endpoint_wait_period(&mut fast, epoch, || tick.get(),
            |n| tick.set(tick.get().wrapping_add(n)), || samples.push(tick.get()));
        assert!(!fast);
        assert_eq!(samples, [epoch.wrapping_add(59_999), epoch.wrapping_add(60_000)]);
        for _ in 0..2 {
            let start = tick.get(); samples.clear();
            endpoint_wait_period(&mut fast, epoch, || tick.get(),
                |n| tick.set(tick.get().wrapping_add(n)), || samples.push(tick.get()));
            assert_eq!(samples, [start]);
            assert_eq!(tick.get().wrapping_sub(start), 1000);
        }
        tick.set(epoch); // even a whole counter wrap must not rearm the expired mode
        samples.clear();
        endpoint_wait_period(&mut fast, epoch, || tick.get(),
            |n| tick.set(tick.get().wrapping_add(n)), || samples.push(tick.get()));
        assert!(!fast); assert_eq!(samples.len(), 1);
    }
}

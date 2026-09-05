#![cfg_attr(not(target_arch = "arm"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "arm")]
use rp1_hal::prelude::*;

const WINDOW_US: u64 = 120_000_000;
const PERIOD_US: u64 = 100_000;
const GPIO_PERIOD_TICKS: u32 = 100;
#[cfg(target_arch = "arm")]
const GPIO_PULSE_US: u64 = 1_000;
#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
const DBI_MONITOR_WINDOW_US: u64 = 30_000_000;
#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
const DBI_POLL_US: u64 = 1_000;
#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
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
#[cfg(all(target_arch = "arm", feature = "rp1-linux-pcie-dbi-transition-monitor"))]
const PCIE_VIEWPORT_SELECTOR: usize = 0x4010_8000;
#[cfg(all(target_arch = "arm", feature = "rp1-linux-pcie-dbi-transition-monitor"))]
const PCIE_DBI_WINDOW: usize = 0x4010_9000;

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
#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
const DBI_LINE_TEMPLATE: [u8; 206] = *b"RP1DBI event=INIT elapsed_us=0x0000000000000000 valid=0 sel0=0x00000000 sel1=0x00000000 id=0x00000000 cmdstat=0x00000000 classrev=0x00000000 bhlc=0x00000000 bar0=0x00000000 bar1=0x00000000 bar2=0x00000000\r\n";
#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
const DBI_DWORD_OFFSETS: [usize; 7] = [93, 112, 132, 148, 164, 180, 196];

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DbiSample {
    valid: bool,
    sel0: u32,
    sel1: u32,
    dwords: [u32; 7],
}

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
impl DbiSample {
    const fn invalid(sel0: u32, sel1: u32) -> Self {
        Self {
            valid: false,
            sel0,
            sel1,
            dwords: [0; 7],
        }
    }
}

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DbiEvent {
    Initial,
    Change,
    Cap,
    End,
}

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
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

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
struct DbiMonitor {
    last: Option<DbiSample>,
    records: u8,
    capped: bool,
    ended: bool,
}

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
impl DbiMonitor {
    const fn new() -> Self {
        Self {
            last: None,
            records: 0,
            capped: false,
            ended: false,
        }
    }

    fn observe(&mut self, sample: DbiSample) -> Option<DbiEvent> {
        if self.last == Some(sample) {
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

fn encode_hex_u32(out: &mut [u8], offset: usize, value: u32) {
    for index in 0..8 {
        let shift = 28 - index * 4;
        out[offset + index] = HEX[((value >> shift) & 0xf) as usize];
    }
}

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
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

#[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
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

#[cfg(all(target_arch = "arm", feature = "rp1-linux-pcie-dbi-transition-monitor"))]
fn read_dbi_sample() -> DbiSample {
    let sel0 = read32(PCIE_VIEWPORT_SELECTOR);
    if sel0 != 0 {
        // A non-zero selector makes the DBI window ambiguous; do not touch it.
        let sel1 = read32(PCIE_VIEWPORT_SELECTOR);
        return DbiSample::invalid(sel0, sel1);
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
    #[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
    let mut dbi_monitor = DbiMonitor::new();
    #[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
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
        #[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
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
                let sample = read_dbi_sample();
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

    #[cfg(feature = "rp1-linux-pcie-dbi-transition-monitor")]
    #[test]
    fn dbi_transition_monitor_is_change_only_bounded_and_fixed() {
        let mut monitor = DbiMonitor::new();
        let mut sample = DbiSample {
            valid: true,
            sel0: 0,
            sel1: 0,
            dwords: [0x2000_1927, 0, 0x0200_0000, 0, 0x0080_0000, 0, 0x0040_0000],
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
        assert_eq!(
            dbi_line(DbiEvent::Cap, 0x0123_4567_89ab_cdef, encoded),
            *b"RP1DBI event=CAP  elapsed_us=0x0123456789abcdef valid=0 sel0=0x11112222 sel1=0x33334444 id=0xdeadbeef cmdstat=0x00010002 classrev=0x02000000 bhlc=0x00010000 bar0=0x00800000 bar1=0x00000000 bar2=0x00400000\r\n"
        );
    }
}

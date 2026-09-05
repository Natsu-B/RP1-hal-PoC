#![cfg_attr(not(target_arch = "arm"), allow(dead_code, unused_imports))]

use rp1_hal::prelude::*;
use rp1_hal::spi::{Spi0, Spi0Snapshot};

pub const RECORD_OFFSET: usize = 0x180;
pub const RECORD_WORDS: usize = 160;
pub const MAGIC: u32 = u32::from_le_bytes(*b"C1RP");
pub const VERSION: u32 = 1;

pub const WORD_MAGIC: usize = 0;
pub const WORD_VERSION: usize = 1;
pub const WORD_SIZE: usize = 2;
pub const WORD_SEQUENCE: usize = 3;
pub const WORD_ACK: usize = 4;
pub const WORD_GO: usize = 5;
pub const WORD_PHASE: usize = 6;
pub const WORD_COMPLETION: usize = 7;
pub const WORD_FLAGS: usize = 8;
pub const WORD_ERROR: usize = 9;
pub const WORD_START_US: usize = 10;
pub const WORD_END_US: usize = 12;
pub const WORD_ELAPSED_US: usize = 14;
pub const WORD_PLL_SYS_PRIM_BEFORE: usize = 16;
pub const WORD_PLL_SYS_PRIM_AFTER: usize = 17;
pub const WORD_UART_BYTES: usize = 18;
pub const WORD_UART_TICKS: usize = 19;
pub const WORD_SPI_BYTES: usize = 20;
pub const WORD_PWM_LOW_STATUS: usize = 21;
pub const WORD_PWM_HIGH_STATUS: usize = 22;
pub const WORD_SPI_STATUS: usize = 23;
pub const WORD_ENTRY_SNAPSHOTS: usize = 24;
pub const WORD_COMPLETION_SNAPSHOTS: usize = 72;
pub const WORD_PWM_LOW_SNAPSHOT: usize = 120;
pub const WORD_PWM_HIGH_SNAPSHOT: usize = 132;
pub const WORD_SPI_SNAPSHOT: usize = 144;
pub const WORD_UART_CLOCK: usize = 154;
pub const WORD_AUTONOMOUS_TARGET_US: usize = 158;
pub const WORD_CHECKSUM: usize = 159;

pub const PHASE_ENTRY: u32 = 1;
pub const PHASE_WAIT_GO: u32 = 2;
pub const PHASE_GO_RECEIVED: u32 = 3;
pub const PHASE_AUTONOMOUS: u32 = 4;
pub const PHASE_DONE: u32 = 5;
pub const PHASE_ERROR: u32 = 0x8000_0000;

pub const COMPLETION_DONE: u32 = u32::from_le_bytes(*b"DONE");
pub const COMPLETION_GO_TIMEOUT: u32 = u32::from_le_bytes(*b"GOUT");
pub const COMPLETION_FAILED: u32 = u32::from_le_bytes(*b"FAIL");

const FLAG_INITIALIZED: u32 = 1 << 0;
const FLAG_LINK_ACKED: u32 = 1 << 1;
const FLAG_GO_RECEIVED: u32 = 1 << 2;
const FLAG_PLL_SYS_LOCKED: u32 = 1 << 3;
const FLAG_PLL_SYS_PRIM_ENABLED: u32 = 1 << 4;
const FLAG_UART_CLOCK_READY: u32 = 1 << 5;
const FLAG_UART_MARKER_WRITTEN: u32 = 1 << 6;
const FLAG_PWM_LOW_STARTED: u32 = 1 << 7;
const FLAG_PWM_HIGH_APPLIED: u32 = 1 << 8;
const FLAG_SPI_RESET_RELEASED: u32 = 1 << 9;
const FLAG_SPI_PACKET_WRITTEN: u32 = 1 << 10;
const FLAG_AUTONOMOUS_WINDOW_DONE: u32 = 1 << 11;
const FLAG_GPIO_PROGRESS: u32 = 1 << 12;
const FLAG_COMPLETION_SNAPSHOT: u32 = 1 << 13;
const FLAG_PWM_STOPPED: u32 = 1 << 14;
const FLAG_REBOOT_RECOVERY_REQUIRED: u32 = 1 << 15;

const ERROR_GO_TIMEOUT: u32 = 1;
const ERROR_CLOCK_CONTRACT: u32 = 2;
const ERROR_SPI_RESET_BASE: u32 = 0x100;
const ERROR_UART_WRITE: u32 = 0x200;
const ERROR_PWM_LOW_BASE: u32 = 0x300;
const ERROR_PWM_HIGH_BASE: u32 = 0x400;
const ERROR_SPI_INIT: u32 = 0x500;
const ERROR_SPI_WRITE: u32 = 0x501;
const ERROR_PWM_STOP: u32 = 0x600;

const GO_TIMEOUT_US: u64 = 10_000_000;
const AUTONOMOUS_US: u64 = 2_200_000;
const PWM_PHASE_US: u64 = 1_100_000;
const UART_TICK_US: u64 = 200_000;
const CHECKSUM_SEED: u32 = 0x811c_9dc5;
const CHECKSUM_MUL: u32 = 0x9e37_79b1;
const ABSENT: usize = usize::MAX;
const PLL_SYS_PRIM: usize = 0x4002_0010;

const CLOCK_REGISTERS: [[usize; 4]; 12] = [
    [0x4002_0000, 0x4002_0004, 0x4002_0008, 0x4002_000c],
    [0x4002_4000, 0x4002_4004, 0x4002_4008, 0x4002_400c],
    [0x4002_8000, 0x4002_8004, 0x4002_8008, 0x4002_800c],
    [0x4001_8014, 0x4001_8018, ABSENT, 0x4001_8020],
    [0x4001_8024, 0x4001_8028, ABSENT, 0x4001_8030],
    [0x4001_8044, 0x4001_8048, ABSENT, 0x4001_8050],
    [0x4001_8054, 0x4001_8058, ABSENT, 0x4001_8060],
    [0x4001_8064, 0x4001_8068, ABSENT, 0x4001_8070],
    [0x4001_8074, 0x4001_8078, 0x4001_807c, 0x4001_8080],
    [0x4001_8084, 0x4001_8088, 0x4001_808c, 0x4001_8090],
    [0x4001_80b4, 0x4001_80b8, ABSENT, 0x4001_80c0],
    [0x4001_80e4, 0x4001_80e8, 0x4001_80ec, 0x4001_80f0],
];

#[repr(C)]
struct ClockIndependenceRecord([u32; RECORD_WORDS]);

const _: () = assert!(core::mem::size_of::<rp1_hal::debug::DebugMailbox>() <= RECORD_OFFSET);
const _: () = assert!(core::mem::size_of::<ClockIndependenceRecord>() <= 0x280);
const _: () = assert!(
    RECORD_OFFSET + core::mem::size_of::<ClockIndependenceRecord>() <= rp1_hal::debug::MAILBOX_SIZE
);

#[cfg(target_arch = "arm")]
#[inline(always)]
fn record() -> *mut u32 {
    (rp1_hal::debug::MAILBOX_ADDR as usize + RECORD_OFFSET) as *mut u32
}

#[cfg(target_arch = "arm")]
#[inline(always)]
unsafe fn read_word(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(record().add(index)) }
}

#[cfg(target_arch = "arm")]
#[inline(always)]
unsafe fn write_word(index: usize, value: u32) {
    unsafe { core::ptr::write_volatile(record().add(index), value) };
}

#[inline(always)]
const fn checksum_update(checksum: u32, word: u32) -> u32 {
    (checksum ^ word).rotate_left(5).wrapping_mul(CHECKSUM_MUL)
}

#[cfg(test)]
fn checksum_words(words: &[u32; RECORD_WORDS]) -> u32 {
    let mut checksum = CHECKSUM_SEED;
    let mut index = 0;
    while index < WORD_CHECKSUM {
        if index != WORD_COMPLETION {
            checksum = checksum_update(checksum, words[index]);
        }
        index += 1;
    }
    checksum
}

#[cfg(target_arch = "arm")]
unsafe fn record_checksum() -> u32 {
    let mut checksum = CHECKSUM_SEED;
    let mut index = 0;
    while index < WORD_CHECKSUM {
        if index != WORD_COMPLETION {
            checksum = checksum_update(checksum, unsafe { read_word(index) });
        }
        index += 1;
    }
    checksum
}

#[cfg(target_arch = "arm")]
#[inline(always)]
fn barrier() {
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)) };
}

#[cfg(target_arch = "arm")]
unsafe fn set_flags(flags: u32) {
    let value = unsafe { read_word(WORD_FLAGS) } | flags;
    unsafe { write_word(WORD_FLAGS, value) };
}

#[cfg(target_arch = "arm")]
unsafe fn set_error(error: u32) {
    if unsafe { read_word(WORD_ERROR) } == 0 {
        unsafe { write_word(WORD_ERROR, error) };
    }
}

#[cfg(target_arch = "arm")]
unsafe fn write_u64(index: usize, value: u64) {
    unsafe {
        write_word(index, value as u32);
        write_word(index + 1, (value >> 32) as u32);
    }
}

#[cfg(target_arch = "arm")]
unsafe fn snapshot_clocks(base: usize) {
    for (clock, registers) in CLOCK_REGISTERS.iter().enumerate() {
        for (field, address) in registers.iter().enumerate() {
            let value = if *address == ABSENT {
                u32::MAX
            } else {
                unsafe { core::ptr::read_volatile(*address as *const u32) }
            };
            unsafe { write_word(base + clock * 4 + field, value) };
        }
    }
}

#[cfg(target_arch = "arm")]
pub fn initialize() {
    unsafe {
        let previous_sequence = if read_word(WORD_MAGIC) == MAGIC {
            read_word(WORD_SEQUENCE)
        } else {
            0
        };
        for index in 0..RECORD_WORDS {
            write_word(index, 0);
        }
        let sequence = previous_sequence.wrapping_add(1).max(1);
        write_word(WORD_VERSION, VERSION);
        write_word(WORD_SIZE, RECORD_WORDS as u32);
        write_word(WORD_SEQUENCE, sequence);
        write_word(WORD_PHASE, PHASE_ENTRY);
        write_word(WORD_AUTONOMOUS_TARGET_US, AUTONOMOUS_US as u32);
        write_word(
            WORD_PLL_SYS_PRIM_BEFORE,
            core::ptr::read_volatile(PLL_SYS_PRIM as *const u32),
        );
        snapshot_clocks(WORD_ENTRY_SNAPSHOTS);
        write_word(WORD_FLAGS, FLAG_INITIALIZED);
        barrier();
        write_word(WORD_MAGIC, MAGIC);
        barrier();
    }
}

#[cfg(target_arch = "arm")]
pub fn acknowledge_and_wait_for_go(timer: &RawTimer) -> bool {
    unsafe {
        let sequence = read_word(WORD_SEQUENCE);
        write_word(WORD_PHASE, PHASE_WAIT_GO);
        set_flags(FLAG_LINK_ACKED);
        barrier();
        write_word(WORD_ACK, sequence);
        barrier();

        let start = timer.now();
        write_u64(WORD_START_US, start);
        while read_word(WORD_GO) != sequence {
            if timer.elapsed_since(start) >= GO_TIMEOUT_US {
                finish(timer, COMPLETION_GO_TIMEOUT, ERROR_GO_TIMEOUT);
                return false;
            }
            core::hint::spin_loop();
        }

        write_word(WORD_PHASE, PHASE_GO_RECEIVED);
        write_u64(WORD_START_US, timer.now());
        set_flags(FLAG_GO_RECEIVED);
        barrier();
        true
    }
}

#[cfg(target_arch = "arm")]
pub fn record_spi_reset(result: Result<(), super::Spi0ResetError>, timer: &RawTimer) -> bool {
    unsafe {
        match result {
            Ok(()) => {
                set_flags(FLAG_SPI_RESET_RELEASED);
                true
            }
            Err(error) => {
                finish(
                    timer,
                    COMPLETION_FAILED,
                    ERROR_SPI_RESET_BASE | error as u32,
                );
                false
            }
        }
    }
}

#[cfg(target_arch = "arm")]
fn clock_contract_ready() -> bool {
    unsafe {
        let pll_cs = core::ptr::read_volatile(0x4002_0000 as *const u32);
        let pll_pwr = core::ptr::read_volatile(0x4002_0004 as *const u32);
        let pll_fbdiv = core::ptr::read_volatile(0x4002_0008 as *const u32);
        let pll_frac = core::ptr::read_volatile(0x4002_000c as *const u32);
        let pll_prim = core::ptr::read_volatile(PLL_SYS_PRIM as *const u32);
        let uart_ctrl = core::ptr::read_volatile(0x4001_8054 as *const u32);
        let uart_div = core::ptr::read_volatile(0x4001_8058 as *const u32);
        let uart_sel = core::ptr::read_volatile(0x4001_8060 as *const u32);
        let reset_done1 = core::ptr::read_volatile(0x4001_401c as *const u32);

        write_word(WORD_PLL_SYS_PRIM_AFTER, pll_prim);
        write_word(WORD_UART_CLOCK, uart_ctrl);
        write_word(WORD_UART_CLOCK + 1, uart_div);
        write_word(WORD_UART_CLOCK + 2, u32::MAX);
        write_word(WORD_UART_CLOCK + 3, uart_sel);

        let pll_ok =
            pll_cs == 0x8000_0001 && pll_pwr == 0x0000_0004 && pll_fbdiv == 20 && pll_frac == 0;
        let prim_ok = pll_prim == 0x0007_7010;
        let uart_ok = uart_ctrl & 0x0000_0fe0 == 0x0000_0840
            && uart_div == 1
            && uart_sel & 1 != 0
            && reset_done1 & (1 << 26) != 0;
        if pll_ok {
            set_flags(FLAG_PLL_SYS_LOCKED);
        }
        if prim_ok {
            set_flags(FLAG_PLL_SYS_PRIM_ENABLED);
        }
        if uart_ok {
            set_flags(FLAG_UART_CLOCK_READY);
        }
        pll_ok && prim_ok && uart_ok
    }
}

#[cfg(target_arch = "arm")]
fn progress(pin: &mut ConfiguredPin<22, Output>, timer: &RawTimer) {
    pin.set_high();
    timer.delay_us(1_000);
    pin.set_low();
    unsafe { set_flags(FLAG_GPIO_PROGRESS) };
}

#[cfg(target_arch = "arm")]
fn write_pwm_snapshot(base: usize, snapshot: Pwm0Snapshot) {
    let words = [
        snapshot.clock_ctrl,
        snapshot.clock_div_int,
        snapshot.clock_div_frac,
        snapshot.clock_sel,
        snapshot.reset_ctrl1,
        snapshot.reset_done1,
        snapshot.gpio12_ctrl,
        snapshot.gpio12_pad,
        snapshot.global_ctrl,
        snapshot.channel_ctrl,
        snapshot.range,
        snapshot.duty,
    ];
    unsafe {
        for (index, word) in words.into_iter().enumerate() {
            write_word(base + index, word);
        }
    }
}

#[cfg(target_arch = "arm")]
fn write_spi_snapshot(snapshot: Spi0Snapshot) {
    let words = [
        snapshot.version,
        snapshot.control,
        snapshot.enable,
        snapshot.baud_divisor,
        snapshot.status,
        snapshot.tx_fifo_level,
        snapshot.rx_fifo_level,
        snapshot.rx_sample_delay,
        snapshot.cs_override,
        u32::from(snapshot.fifo_depth) << 16 | u32::from(snapshot.bytes_queued),
    ];
    unsafe {
        for (index, word) in words.into_iter().enumerate() {
            write_word(WORD_SPI_SNAPSHOT + index, word);
        }
    }
}

#[cfg(target_arch = "arm")]
fn uart_ticks(uart: &mut Uart0Tx, timer: &RawTimer, start: u64, end_us: u64, next: &mut u64) {
    const TICK: &[u8] = b"RP1CLK tick\r\n";
    loop {
        let elapsed = timer.elapsed_since(start);
        if elapsed >= end_us {
            break;
        }
        if tick_due(elapsed, end_us, *next) {
            let written = uart.write_bytes(TICK);
            unsafe {
                if written == TICK.len() {
                    write_word(WORD_UART_TICKS, read_word(WORD_UART_TICKS) + 1);
                } else {
                    set_error(ERROR_UART_WRITE);
                }
            }
            *next += UART_TICK_US;
        }
        core::hint::spin_loop();
    }
}

const fn tick_due(elapsed: u64, end_us: u64, next: u64) -> bool {
    elapsed < end_us && elapsed >= next
}

#[cfg(target_arch = "arm")]
pub fn run_autonomous(
    gpio22: &mut ConfiguredPin<22, Output>,
    gpio: &mut Gpio,
    timer: &RawTimer,
    uart0: Uart0,
    pwm0: &mut Pwm0,
    spi0: Spi0,
) {
    const UART_MARKER: &[u8] = b"\r\nRP1 CLOCK XOSC UART0 AUTONOMOUS\r\n";
    const PWM_LOW: Pwm0Config = Pwm0Config::new(5_000_000, 1_250_000);
    const PWM_HIGH: Pwm0Config = Pwm0Config::new(50_000, 37_500);
    const SPI_PACKET: [u8; 20] = [
        0x44, 0x31, 0x53, 0x50, 0x01, 0x53, 0x02, 0x09, 0xdf, 0x9b, 0x57, 0x13, 0xe0, 0xac, 0x68,
        0x24, 0x53, 0x50, 0x49, 0x30,
    ];

    if !clock_contract_ready() {
        unsafe { finish(timer, COMPLETION_FAILED, ERROR_CLOCK_CONTRACT) };
        return;
    }

    unsafe {
        write_word(WORD_PHASE, PHASE_AUTONOMOUS);
        write_u64(WORD_START_US, timer.now());
        set_flags(FLAG_REBOOT_RECOVERY_REQUIRED);
        barrier();
    }
    let start = unsafe {
        u64::from(read_word(WORD_START_US)) | (u64::from(read_word(WORD_START_US + 1)) << 32)
    };
    let mut uart = uart0.init_tx_115200_clock_ready();
    let marker_written = uart.write_bytes(UART_MARKER);
    unsafe {
        write_word(WORD_UART_BYTES, marker_written as u32);
        if marker_written == UART_MARKER.len() {
            set_flags(FLAG_UART_MARKER_WRITTEN);
        } else {
            set_error(ERROR_UART_WRITE);
        }
    }
    progress(gpio22, timer);

    let mut pwm_channel = match pwm0.start_gpio12(PWM_LOW) {
        Ok(channel) => {
            let snapshot = channel.snapshot();
            write_pwm_snapshot(WORD_PWM_LOW_SNAPSHOT, snapshot);
            unsafe {
                write_word(WORD_PWM_LOW_STATUS, 1);
                set_flags(FLAG_PWM_LOW_STARTED);
            }
            Some(channel)
        }
        Err(error) => {
            let snapshot = pwm0.snapshot();
            write_pwm_snapshot(WORD_PWM_LOW_SNAPSHOT, snapshot);
            unsafe {
                write_word(WORD_PWM_LOW_STATUS, error as u32);
                set_error(ERROR_PWM_LOW_BASE | error as u32);
            }
            None
        }
    };
    progress(gpio22, timer);

    let mut next_tick = UART_TICK_US;
    uart_ticks(&mut uart, timer, start, PWM_PHASE_US, &mut next_tick);

    if let Some(channel) = pwm_channel.as_mut() {
        match channel.apply(PWM_HIGH) {
            Ok(()) => unsafe {
                write_word(WORD_PWM_HIGH_STATUS, 1);
                set_flags(FLAG_PWM_HIGH_APPLIED);
            },
            Err(error) => unsafe {
                write_word(WORD_PWM_HIGH_STATUS, error as u32);
                set_error(ERROR_PWM_HIGH_BASE | error as u32);
            },
        }
        write_pwm_snapshot(WORD_PWM_HIGH_SNAPSHOT, channel.snapshot());
    } else {
        write_pwm_snapshot(WORD_PWM_HIGH_SNAPSHOT, pwm0.snapshot());
    }
    progress(gpio22, timer);

    let cs0 = gpio.pin::<8>();
    let miso = gpio.pin::<9>();
    let mosi = gpio.pin::<10>();
    let sclk = gpio.pin::<11>();
    match spi0.into_host_mode0_100khz(cs0, miso, mosi, sclk) {
        Ok(mut spi) => match spi.write(&SPI_PACKET) {
            Ok(snapshot) => {
                write_spi_snapshot(snapshot);
                unsafe {
                    write_word(WORD_SPI_STATUS, 1);
                    write_word(WORD_SPI_BYTES, SPI_PACKET.len() as u32);
                    set_flags(FLAG_SPI_PACKET_WRITTEN);
                }
            }
            Err(_) => unsafe {
                write_word(WORD_SPI_STATUS, 2);
                set_error(ERROR_SPI_WRITE);
            },
        },
        Err(_) => unsafe {
            write_word(WORD_SPI_STATUS, 3);
            set_error(ERROR_SPI_INIT);
        },
    }
    progress(gpio22, timer);

    uart_ticks(&mut uart, timer, start, AUTONOMOUS_US, &mut next_tick);
    if let Some(channel) = pwm_channel.as_mut() {
        match channel
            .apply(Pwm0Config::new(PWM_HIGH.range, 0))
            .and_then(|()| channel.stop())
        {
            Ok(()) => unsafe { set_flags(FLAG_PWM_STOPPED) },
            Err(error) => unsafe { set_error(ERROR_PWM_STOP | error as u32) },
        }
    }
    progress(gpio22, timer);

    unsafe {
        set_flags(FLAG_AUTONOMOUS_WINDOW_DONE);
        let completion = if read_word(WORD_ERROR) == 0 {
            COMPLETION_DONE
        } else {
            COMPLETION_FAILED
        };
        finish(timer, completion, 0);
    }
}

#[cfg(target_arch = "arm")]
unsafe fn finish(timer: &RawTimer, completion: u32, error: u32) {
    if error != 0 {
        unsafe { set_error(error) };
    }
    unsafe {
        snapshot_clocks(WORD_COMPLETION_SNAPSHOTS);
        set_flags(FLAG_COMPLETION_SNAPSHOT);
        let end = timer.now();
        let start =
            u64::from(read_word(WORD_START_US)) | (u64::from(read_word(WORD_START_US + 1)) << 32);
        write_u64(WORD_END_US, end);
        write_u64(WORD_ELAPSED_US, end.wrapping_sub(start));
        write_word(
            WORD_PHASE,
            if completion == COMPLETION_DONE {
                PHASE_DONE
            } else {
                PHASE_ERROR
            },
        );
        write_word(WORD_CHECKSUM, record_checksum());
        barrier();
        write_word(WORD_COMPLETION, completion);
        barrier();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_layout_and_checksum_contract_are_fixed() {
        assert_eq!(RECORD_OFFSET / 4, 96);
        assert_eq!(WORD_COMPLETION_SNAPSHOTS - WORD_ENTRY_SNAPSHOTS, 48);
        assert_eq!(WORD_CHECKSUM + 1, RECORD_WORDS);
        let mut words = [0u32; RECORD_WORDS];
        for (index, word) in words.iter_mut().enumerate() {
            *word = index as u32;
        }
        assert_eq!(checksum_words(&words), 0xdcd7_9d40);
        words[42] ^= 1;
        assert_ne!(checksum_words(&words), 0xdcd7_9d40);
        assert!(tick_due(1_999_999, 2_200_000, 1_800_000));
        assert!(!tick_due(2_200_000, 2_200_000, 2_200_000));
    }
}

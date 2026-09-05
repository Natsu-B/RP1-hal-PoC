#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
use rp1_hal::prelude::*;
#[cfg(all(target_arch = "arm", feature = "uart-reset-irq-map-proof"))]
use rp1_hal::reset::{ResetController, UartReset};
#[cfg(target_arch = "arm")]
use rp1_rt as _;

#[cfg(all(target_arch = "arm", feature = "inbound-monitor-block-proof"))]
mod inbound_monitor;

#[cfg(feature = "rp1-clock-independence-proof")]
mod clock_independence;

#[cfg(feature = "rp1-linux-clk-uart-ownership-conflict")]
mod linux_clk_uart_ownership;

#[cfg(all(target_arch = "arm", feature = "watchdog-expiry-reason-proof"))]
mod watchdog_expiry_reason;

#[cfg(all(target_arch = "arm", feature = "rp1-adc-one-shot-proof"))]
mod adc_one_shot;

#[cfg(all(
    target_arch = "arm",
    feature = "rp1-i2s-readonly-prerequisite-snapshot"
))]
mod i2s_readonly;

#[cfg(all(
    feature = "rp1-clock-independence-proof",
    feature = "inbound-monitor-block-proof"
))]
compile_error!("rp1-clock-independence-proof and inbound-monitor-block-proof share the mailbox");

#[cfg(all(
    feature = "debug-mailbox-layout-v1",
    any(
        feature = "rp1-clock-independence-proof",
        feature = "bar2-readonly-handshake"
    )
))]
compile_error!("debug-mailbox-layout-v1 is incompatible with legacy clock/BAR2 proof records");

#[cfg(all(
    feature = "rp1-linux-clk-uart-ownership-conflict",
    any(
        feature = "rp1-clock-independence-proof",
        feature = "inbound-monitor-block-proof"
    )
))]
compile_error!(
    "rp1-linux-clk-uart-ownership-conflict is a terminal proof and cannot share clock/inbound proof paths"
);

#[cfg(all(
    feature = "gpio22-start-proof",
    any(feature = "endpoint-clock-only", feature = "pcie-ep-init")
))]
compile_error!("gpio22-start-proof is terminal and cannot enter PCIe endpoint initialization");

#[cfg(all(feature = "spi0-local-irq-proof", feature = "spi0-host-proof"))]
compile_error!("spi0-local-irq-proof is terminal and cannot share the SPI host polling proof");

#[cfg(all(
    feature = "spi0-local-irq-bank1-passive-scout",
    any(feature = "spi0-local-irq-proof", feature = "spi0-host-proof")
))]
compile_error!("spi0-local-irq-bank1-passive-scout cannot share another SPI proof");

#[cfg(all(feature = "i2c1-local-irq-proof", feature = "i2c1-host-proof"))]
compile_error!("i2c1-local-irq-proof is terminal and cannot share the I2C1 host polling proof");

#[cfg(all(
    feature = "i2c1-local-irq-bank1-passive-scout",
    any(feature = "i2c1-local-irq-proof", feature = "i2c1-host-proof")
))]
compile_error!("i2c1-local-irq-bank1-passive-scout cannot share another I2C1 proof");

#[cfg(all(
    feature = "spi0-local-irq-proof",
    any(
        feature = "rp1-clock-independence-proof",
        feature = "rp1-linux-clk-uart-ownership-conflict",
        feature = "pwm0-local-irq-proof",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "uart0-rx-irq"
    )
))]
compile_error!("spi0-local-irq-proof cannot share another terminal proof mode");

#[cfg(all(
    feature = "spi0-local-irq-bank1-passive-scout",
    any(
        feature = "rp1-clock-independence-proof",
        feature = "rp1-linux-clk-uart-ownership-conflict",
        feature = "pwm0-local-irq-proof",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "uart0-rx-irq"
    )
))]
compile_error!("spi0-local-irq-bank1-passive-scout cannot share another terminal proof mode");

#[cfg(all(
    feature = "i2c1-local-irq-proof",
    any(
        feature = "rp1-clock-independence-proof",
        feature = "rp1-linux-clk-uart-ownership-conflict",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "uart0-rx-irq"
    )
))]
compile_error!("i2c1-local-irq-proof cannot share another terminal proof mode");

#[cfg(all(
    feature = "i2c1-local-irq-bank1-passive-scout",
    any(
        feature = "rp1-clock-independence-proof",
        feature = "rp1-linux-clk-uart-ownership-conflict",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "uart0-rx-irq"
    )
))]
compile_error!("i2c1-local-irq-bank1-passive-scout cannot share another terminal proof mode");

#[cfg(all(
    feature = "uart1-local-nvic42-delivery",
    any(
        feature = "uart2-local-nvic43-delivery",
        feature = "uart3-local-nvic44-delivery",
        feature = "uart4-local-nvic45-delivery",
        feature = "uart5-local-nvic46-delivery",
        feature = "uart-reset-irq-map-proof",
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
compile_error!("uart1-local-nvic42-delivery cannot share another terminal IRQ proof mode");

#[cfg(all(
    feature = "uart2-local-nvic43-delivery",
    any(
        feature = "uart1-local-nvic42-delivery",
        feature = "uart3-local-nvic44-delivery",
        feature = "uart4-local-nvic45-delivery",
        feature = "uart5-local-nvic46-delivery",
        feature = "uart-reset-irq-map-proof",
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
compile_error!("uart2-local-nvic43-delivery cannot share another terminal IRQ proof mode");

#[cfg(all(
    feature = "uart3-local-nvic44-delivery",
    any(
        feature = "uart1-local-nvic42-delivery",
        feature = "uart2-local-nvic43-delivery",
        feature = "uart4-local-nvic45-delivery",
        feature = "uart5-local-nvic46-delivery",
        feature = "uart-reset-irq-map-proof",
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
compile_error!("uart3-local-nvic44-delivery cannot share another terminal IRQ proof mode");

#[cfg(all(
    feature = "uart4-local-nvic45-delivery",
    any(
        feature = "uart1-local-nvic42-delivery",
        feature = "uart2-local-nvic43-delivery",
        feature = "uart3-local-nvic44-delivery",
        feature = "uart5-local-nvic46-delivery",
        feature = "uart-reset-irq-map-proof",
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
compile_error!("uart4-local-nvic45-delivery cannot share another terminal IRQ proof mode");

#[cfg(all(
    feature = "uart5-local-nvic46-delivery",
    any(
        feature = "uart1-local-nvic42-delivery",
        feature = "uart2-local-nvic43-delivery",
        feature = "uart3-local-nvic44-delivery",
        feature = "uart4-local-nvic45-delivery",
        feature = "uart-reset-irq-map-proof",
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
compile_error!("uart5-local-nvic46-delivery cannot share another terminal IRQ proof mode");

#[cfg(all(
    feature = "timer0-inte-ints-proof",
    any(
        feature = "raw-timer-proof",
        feature = "timer-register-proof",
        feature = "timer-writable-time-proof",
        feature = "uart-reset-irq-map-proof",
        feature = "rp1-clock-independence-proof",
        feature = "rp1-linux-clk-uart-ownership-conflict",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "uart0-rx-irq"
    )
))]
compile_error!("timer0-inte-ints-proof cannot share another terminal proof mode");

#[cfg(all(
    feature = "timer0-alarm0-local-irq26-candidate",
    any(
        feature = "inbound-monitor-block-proof",
        feature = "cortex-m3-option-proof",
        feature = "boot-rom-readonly-proof",
        feature = "boot-rom-boundary-proof",
        feature = "proc1-boot-rom-proof",
        feature = "dual-core-memory-proof",
        feature = "proc-local-memory-proof",
        feature = "expected-fault-recovery-proof",
        feature = "shared-sram-bitband-proof",
        feature = "internal-memory-boundary-read-proof",
        feature = "shared-sram-64k-mirror-readonly-proof",
        feature = "shared-sram-alias-window-extent-readonly-proof",
        feature = "shared-sram-system-region-alias-readonly-proof",
        feature = "mpu-fault-enforcement-proof",
        feature = "watchdog-proof",
        feature = "watchdog-scratch-proof",
        feature = "timer-register-proof",
        feature = "timer-writable-time-proof",
        feature = "timer0-inte-ints-proof",
        feature = "raw-timer-proof",
        feature = "uart-reset-irq-map-proof",
        feature = "rp1-clock-independence-proof",
        feature = "rp1-linux-clk-uart-ownership-conflict",
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq-proof",
        feature = "spi0-local-irq-proof",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq-proof",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "pll-sys-core-lock-only",
        feature = "gpio-wiring-proof",
        feature = "debug-mailbox-ping"
    )
))]
compile_error!("timer0-alarm0-local-irq26-candidate cannot share another terminal proof mode");

#[cfg(target_arch = "arm")]
fn delay_blink() {
    for _ in 0..500_000 {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "arm")]
fn pulse_group(pin: &mut ConfiguredPin<22, Output>, count: u8) {
    for _ in 0..count {
        pin.set_high();
        delay_blink();
        pin.set_low();
        delay_blink();
    }
    delay_blink();
}

#[cfg(target_arch = "arm")]
fn delay_readback_units(units: u32) {
    for _ in 0..units * 1_000 {
        // Volatile stack reads keep dynamic pulse widths proportional in release builds.
        unsafe {
            core::ptr::read_volatile(&units);
        }
    }
}

#[cfg(target_arch = "arm")]
fn pulse_width(pin: &mut ConfiguredPin<22, Output>, units: u32) {
    pin.set_high();
    delay_readback_units(units);
    pin.set_low();
    delay_readback_units(8);
}

#[cfg(all(target_arch = "arm", feature = "gpio-wiring-proof"))]
fn wiring_pulses<const N: u8>(pin: &mut ConfiguredPin<N, Output>, count: u8) {
    for _ in 0..count {
        pin.set_high();
        busy_wait_us(5_000);
        pin.set_low();
        busy_wait_us(5_000);
    }
    busy_wait_us(10_000);
}

#[cfg(all(target_arch = "arm", feature = "gpio-wiring-proof"))]
fn wait_for_input_level<const N: u8>(
    pin: &ConfiguredPin<N, Input>,
    high: bool,
    timeout_us: u64,
) -> bool {
    let start = raw_timer_us();
    loop {
        if pin.is_high() == high {
            return true;
        }
        if raw_timer_us().wrapping_sub(start) > timeout_us {
            return false;
        }
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PllSysResetError {
    PreconditionMismatch,
    ReadbackMismatch,
    Timeout,
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
fn release_pll_sys_reset_bit29() -> Result<(), PllSysResetError> {
    const CTRL: *mut u32 = 0x4001_4000 as *mut u32;
    const RESET_DONE: *const u32 = 0x4001_4018 as *const u32;
    const PLL_SYS_RESET: u32 = 1 << 29;
    const POLL_LIMIT: usize = 4_096;

    unsafe {
        let old = core::ptr::read_volatile(CTRL);
        if old & PLL_SYS_RESET == 0 {
            return Err(PllSysResetError::PreconditionMismatch);
        }

        core::ptr::write_volatile(CTRL, old & !PLL_SYS_RESET);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if core::ptr::read_volatile(CTRL) & PLL_SYS_RESET != 0 {
            return Err(PllSysResetError::ReadbackMismatch);
        }
    }

    for _ in 0..POLL_LIMIT {
        if unsafe { core::ptr::read_volatile(RESET_DONE) } & PLL_SYS_RESET != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }

    Err(PllSysResetError::Timeout)
}

#[cfg(all(target_arch = "arm", feature = "endpoint-clock-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EndpointClockError {
    ReadbackMismatch,
    Timeout,
}

#[cfg(all(target_arch = "arm", feature = "endpoint-clock-only"))]
fn endpoint_clock_phase(pin: &mut ConfiguredPin<22, Output>, count: u8) {
    for _ in 0..count {
        pulse_width(pin, 16);
    }
    delay_readback_units(32);
}

#[cfg(all(target_arch = "arm", feature = "endpoint-clock-only"))]
fn enable_endpoint_clock_bit26(
    pin: &mut ConfiguredPin<22, Output>,
) -> Result<(), EndpointClockError> {
    const CTRL: *mut u32 = 0x4001_4000 as *mut u32;
    const STATUS: *const u32 = 0x4001_4018 as *const u32;
    const BIT26: u32 = 1 << 26;
    const POLL_LIMIT: usize = 100_000;

    endpoint_clock_phase(pin, 8); // ECLK0: before CTRL read.
    unsafe {
        let old = core::ptr::read_volatile(CTRL);
        core::ptr::write_volatile(CTRL, old & !BIT26);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if core::ptr::read_volatile(CTRL) & BIT26 != 0 {
            return Err(EndpointClockError::ReadbackMismatch);
        }
    }
    endpoint_clock_phase(pin, 9); // ECLK1: write/readback complete.

    for _ in 0..POLL_LIMIT {
        if unsafe { core::ptr::read_volatile(STATUS) } & BIT26 != 0 {
            endpoint_clock_phase(pin, 10); // ECLK2: ready.
            return Ok(());
        }
        core::hint::spin_loop();
    }

    Err(EndpointClockError::Timeout)
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State3Decision {
    CoreAlive,
    CoreAliveTimeout,
    PerstnTimeout,
    PreState1ReadbackMismatch,
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
#[derive(Clone, Copy)]
struct State3Result {
    decision: State3Decision,
    pre_state1_observation: u16,
    perstn_wait_us: u16,
    perstn_observation: u16,
    boundary_us: u16,
    cperstn_to_first_poll_us: u16,
}

#[cfg(all(target_arch = "arm", feature = "state5-composite-boundary"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State5Decision {
    LinkUp,
    LinkTimeout,
}

#[cfg(all(target_arch = "arm", feature = "state5-composite-boundary"))]
#[cfg_attr(feature = "uart-reset-irq-map-proof", allow(dead_code))]
#[derive(Clone, Copy)]
struct State5Result {
    decision: State5Decision,
    wait_us: u16,
    final_monitor2: u32,
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
enum PllSysCoreLockDecision {
    Locked = 1,
    PreconditionMismatch = 2,
    FractionReadbackMismatch = 3,
    Timeout = 4,
    PostReadbackMismatch = 5,
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
#[derive(Clone, Copy)]
struct PllSysSnapshot {
    cs: u32,
    pwr: u32,
    fbdiv_int: u32,
    fbdiv_frac: u32,
    prim: u32,
    sec: u32,
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
#[cfg_attr(feature = "rp1-linux-clk-uart-ownership-conflict", allow(dead_code))]
#[derive(Clone, Copy)]
struct PllSysCoreLockResult {
    decision: PllSysCoreLockDecision,
    elapsed_us: u32,
    first_cs: u32,
    before: PllSysSnapshot,
    after: PllSysSnapshot,
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PllSysPriPhError {
    ParentNotLocked,
    PreconditionMismatch,
    ReadbackMismatch,
}

#[cfg(all(target_arch = "arm", feature = "uart0-reset-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Uart0ResetError {
    PreconditionMismatch,
    ClockReadbackMismatch,
    WriteRejected,
    Timeout,
}

#[cfg(all(target_arch = "arm", feature = "i2c1-reset-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum I2c1ResetError {
    PreconditionMismatch,
    WriteRejected,
    Timeout,
}

#[cfg(all(target_arch = "arm", feature = "spi0-reset-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Spi0ResetError {
    PreconditionMismatch,
    WriteRejected,
    Timeout,
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct PreState1Result {
    ready: bool,
    observation: u16,
    clock_ctrl_before: u32,
    clock_ctrl: u32,
    clock_sel: u32,
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
#[allow(dead_code)]
#[derive(Clone, Copy)]
struct State1_2Result {
    qualified: bool,
    wait_us: u16,
    observation: u16,
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn raw_timer_us() -> u64 {
    const RAW_HIGH: *const u32 = 0x400a_c024 as *const u32;
    const RAW_LOW: *const u32 = 0x400a_c028 as *const u32;

    loop {
        let high_before = unsafe { core::ptr::read_volatile(RAW_HIGH) };
        let low = unsafe { core::ptr::read_volatile(RAW_LOW) };
        let high_after = unsafe { core::ptr::read_volatile(RAW_HIGH) };
        if high_before == high_after {
            return (u64::from(high_before) << 32) | u64::from(low);
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn busy_wait_us(delay_us: u64) {
    let start = raw_timer_us();
    while raw_timer_us().wrapping_sub(start) < delay_us {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn elapsed_u16(start: u64, end: u64) -> u16 {
    core::cmp::min(end.wrapping_sub(start), u64::from(u16::MAX)) as u16
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn pre_state1_reset_clock_boundary() -> PreState1Result {
    const CONTROL: *mut u32 = 0x4010_8004 as *mut u32;
    const PM_CONTROL: *mut u32 = 0x4010_8194 as *mut u32;
    const RESET_SEQUENCE_C0: *mut u32 = 0x4010_81c0 as *mut u32;
    const RESET_SEQUENCE_C8: *mut u32 = 0x4010_81c8 as *mut u32;
    const PCIE_AUX_CTRL: *mut u32 = 0x4001_80e4 as *mut u32;
    const PCIE_AUX_DIV_INT: *mut u32 = 0x4001_80e8 as *mut u32;
    const PCIE_AUX_DIV_FRAC: *mut u32 = 0x4001_80ec as *mut u32;
    const PCIE_AUX_ENABLE_SET: *mut u32 = 0x4001_a0e4 as *mut u32;
    const PCIE_AUX_SEL: *const u32 = 0x4001_80f0 as *const u32;
    const PCIE_AUX_SOURCE_MASK: u32 = 0x0000_01e3;
    const PCIE_AUX_SOURCE_XOSC: u32 = 0x0000_0001;
    const PCIE_AUX_ENABLE: u32 = 1 << 11;

    unsafe {
        let old = core::ptr::read_volatile(RESET_SEQUENCE_C8);
        core::ptr::write_volatile(RESET_SEQUENCE_C8, old | (1 << 1));

        let old = core::ptr::read_volatile(RESET_SEQUENCE_C0);
        core::ptr::write_volatile(RESET_SEQUENCE_C0, old & !(1 << 1));

        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old & !(1 << 2));

        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old | 1);

        let old = core::ptr::read_volatile(PM_CONTROL);
        core::ptr::write_volatile(PM_CONTROL, old & !(1 << 3));

        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old | (1 << 7));

        let clock_ctrl_before = core::ptr::read_volatile(PCIE_AUX_CTRL);
        core::ptr::write_volatile(
            PCIE_AUX_CTRL,
            (clock_ctrl_before & !PCIE_AUX_SOURCE_MASK) | PCIE_AUX_SOURCE_XOSC,
        );
        core::ptr::write_volatile(PCIE_AUX_DIV_INT, 1);
        core::ptr::write_volatile(PCIE_AUX_DIV_FRAC, 0);
        core::ptr::write_volatile(PCIE_AUX_ENABLE_SET, PCIE_AUX_ENABLE);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let mut observation = 0u16;
        observation |= ((core::ptr::read_volatile(RESET_SEQUENCE_C8) >> 1) & 1) as u16;
        observation |= ((((core::ptr::read_volatile(RESET_SEQUENCE_C0) >> 1) & 1) ^ 1) as u16) << 1;
        let control = core::ptr::read_volatile(CONTROL);
        observation |= ((((control >> 2) & 1) ^ 1) as u16) << 2;
        observation |= ((control & 1) as u16) << 3;
        observation |= ((((core::ptr::read_volatile(PM_CONTROL) >> 3) & 1) ^ 1) as u16) << 4;
        observation |= (((control >> 7) & 1) as u16) << 5;
        let clock_ctrl = core::ptr::read_volatile(PCIE_AUX_CTRL);
        if core::ptr::read_volatile(PCIE_AUX_DIV_INT) == 1 {
            observation |= 1 << 7;
        }
        if core::ptr::read_volatile(PCIE_AUX_DIV_FRAC) == 0 {
            observation |= 1 << 8;
        }
        let clock_sel = core::ptr::read_volatile(PCIE_AUX_SEL);
        if clock_ctrl & PCIE_AUX_ENABLE == PCIE_AUX_ENABLE && clock_sel == 1 {
            observation |= 1 << 6;
        }
        if clock_sel != 0 {
            observation |= 1 << 9;
        }

        PreState1Result {
            ready: observation & 0x01ff == 0x01ff,
            observation,
            clock_ctrl_before,
            clock_ctrl,
            clock_sel,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn state1_2_perstn_qualified_boundary() -> State1_2Result {
    const CONTROL: *mut u32 = 0x4010_8004 as *mut u32;
    const PM_CONTROL: *mut u32 = 0x4010_8194 as *mut u32;
    const MONITOR2: *const u32 = 0x4010_81a4 as *const u32;
    const INTR: *mut u32 = 0x4010_81a8 as *mut u32;
    const INTE: *mut u32 = 0x4010_81ac as *mut u32;
    const INTS: *const u32 = 0x4010_81b4 as *const u32;
    const RESET_SEQUENCE: *mut u32 = 0x4010_81c8 as *mut u32;
    const PERSTN_INTERRUPT: u32 = 1 << 1;
    const PERSTN_STATUS: u32 = 1 << 17;
    const TIMEOUT_US: u64 = 100_000;

    unsafe {
        let old = core::ptr::read_volatile(INTE);
        core::ptr::write_volatile(INTE, old & !0x7f);

        let old = core::ptr::read_volatile(RESET_SEQUENCE);
        core::ptr::write_volatile(RESET_SEQUENCE, old | (1 << 1));

        let old = core::ptr::read_volatile(PM_CONTROL);
        core::ptr::write_volatile(PM_CONTROL, old | (1 << 1));

        let old = core::ptr::read_volatile(PM_CONTROL);
        core::ptr::write_volatile(PM_CONTROL, old & !(1 << 3));

        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old & !(1 << 2));

        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old | 1);

        let old = core::ptr::read_volatile(INTE);
        core::ptr::write_volatile(INTE, old | PERSTN_INTERRUPT);
    }

    let start = raw_timer_us();
    let mut observation = 0u16;

    loop {
        let ints = unsafe { core::ptr::read_volatile(INTS) };
        if ints & PERSTN_INTERRUPT != 0 {
            observation |= 1;
            unsafe {
                core::ptr::write_volatile(INTR, PERSTN_INTERRUPT);
            }
            observation |= 1 << 2;

            let event_time = raw_timer_us();
            let monitor2 = unsafe { core::ptr::read_volatile(MONITOR2) };
            if monitor2 & PERSTN_STATUS != 0 {
                observation |= 1 << 1;
                while raw_timer_us().wrapping_sub(event_time) <= 50 {
                    core::hint::spin_loop();
                }
                return State1_2Result {
                    qualified: true,
                    wait_us: elapsed_u16(start, event_time),
                    observation,
                };
            }
        } else if unsafe { core::ptr::read_volatile(MONITOR2) } & PERSTN_STATUS != 0 {
            observation |= 1 << 1;
        }

        let now = raw_timer_us();
        if now.wrapping_sub(start) > TIMEOUT_US {
            return State1_2Result {
                qualified: false,
                wait_us: elapsed_u16(start, now),
                observation,
            };
        }
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn state3_phase(pin: &mut ConfiguredPin<22, Output>, width: u32) {
    pulse_width(pin, width);
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn state3_composite_boundary(state1_2: State1_2Result) -> State3Result {
    const RESET_SEQUENCE: *mut u32 = 0x4010_81c8 as *mut u32;
    const CONTROL: *mut u32 = 0x4010_8004 as *mut u32;
    const CORE_ALIVE_INTE: *mut u32 = 0x4010_81ac as *mut u32;
    const CORE_ALIVE_STATUS: *const u32 = 0x4010_81a4 as *const u32;
    const CORE_ALIVE: u32 = 1 << 16;
    const TIMEOUT_US: u64 = 20_000;

    let boundary_start = raw_timer_us();

    unsafe {
        let old = core::ptr::read_volatile(RESET_SEQUENCE);
        core::ptr::write_volatile(RESET_SEQUENCE, old & !(1 << 1));
    }

    busy_wait_us(2);

    unsafe {
        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old & !(1 << 7));
    }
    let boundary_end = raw_timer_us();

    unsafe {
        let old = core::ptr::read_volatile(CORE_ALIVE_INTE);
        core::ptr::write_volatile(CORE_ALIVE_INTE, old | 1);
    }

    let first_poll = raw_timer_us();
    let mut status = unsafe { core::ptr::read_volatile(CORE_ALIVE_STATUS) };
    let boundary_us = boundary_end.wrapping_sub(boundary_start) as u16;
    let cperstn_to_first_poll_us = first_poll.wrapping_sub(boundary_end) as u16;

    loop {
        if status & CORE_ALIVE != 0 {
            return State3Result {
                decision: State3Decision::CoreAlive,
                pre_state1_observation: 0,
                perstn_wait_us: state1_2.wait_us,
                perstn_observation: state1_2.observation,
                boundary_us,
                cperstn_to_first_poll_us,
            };
        }
        let now = raw_timer_us();
        if now.wrapping_sub(first_poll) > TIMEOUT_US {
            return State3Result {
                decision: State3Decision::CoreAliveTimeout,
                pre_state1_observation: 0,
                perstn_wait_us: state1_2.wait_us,
                perstn_observation: state1_2.observation,
                boundary_us,
                cperstn_to_first_poll_us,
            };
        }
        core::hint::spin_loop();
        status = unsafe { core::ptr::read_volatile(CORE_ALIVE_STATUS) };
    }
}

#[cfg(all(target_arch = "arm", feature = "state5-composite-boundary"))]
fn state5_composite_boundary() -> State5Result {
    const DBI_SELECTOR: *mut u32 = 0x4010_8000 as *mut u32;
    const CONTROL: *mut u32 = 0x4010_8004 as *mut u32;
    const PCIE_CFG_188: *mut u32 = 0x4010_8188 as *mut u32;
    const MONITOR2: *const u32 = 0x4010_81a4 as *const u32;
    const INTE: *mut u32 = 0x4010_81ac as *mut u32;
    const DBI_BASE: usize = 0x4010_9000;
    const RDLH_LINK_UP: u32 = 1 << 20;
    const LINK_TIMEOUT_US: u64 = 24_000;

    unsafe {
        core::ptr::write_volatile(DBI_SELECTOR, 0);

        let port_afr = (DBI_BASE + 0x70c) as *mut u32;
        let old = core::ptr::read_volatile(port_afr);
        core::ptr::write_volatile(port_afr, (old & 0xc700_00ff) | 0x2830_3000);

        let link_width_speed = (DBI_BASE + 0x80c) as *mut u32;
        let old = core::ptr::read_volatile(link_width_speed);
        core::ptr::write_volatile(link_width_speed, (old & 0xffff_ff00) | 0x30);

        let dbi_ro_wr_en = (DBI_BASE + 0x8bc) as *mut u32;
        let old = core::ptr::read_volatile(dbi_ro_wr_en);
        core::ptr::write_volatile(dbi_ro_wr_en, old | 1);

        core::ptr::write_volatile(DBI_SELECTOR, 0);
        core::ptr::write_volatile((DBI_BASE + 0x81c) as *mut u32, 6);

        let dbi_b0 = (DBI_BASE + 0x0b0) as *mut u32;
        let old = core::ptr::read_volatile(dbi_b0);
        core::ptr::write_volatile(dbi_b0, (old & 0x0000_ffff) | 0x003c_0000);
        core::ptr::write_volatile((DBI_BASE + 0x0b4) as *mut u32, 0);
        core::ptr::write_volatile((DBI_BASE + 0x0b8) as *mut u32, 0x2000);
        core::ptr::write_volatile((DBI_BASE + 0x008) as *mut u32, 0x0200_0000);

        core::ptr::write_volatile(DBI_SELECTOR, 1);
        core::ptr::write_volatile((DBI_BASE + 0x010) as *mut u32, 0x0000_3fff);
        core::ptr::write_volatile((DBI_BASE + 0x014) as *mut u32, 0x003f_ffff);
        core::ptr::write_volatile((DBI_BASE + 0x018) as *mut u32, 0x0000_ffff);

        core::ptr::write_volatile(DBI_SELECTOR, 0);
        core::ptr::write_volatile((DBI_BASE + 0x010) as *mut u32, 0xffff_fff0);
        core::ptr::write_volatile((DBI_BASE + 0x014) as *mut u32, 0xffff_fff0);
        core::ptr::write_volatile((DBI_BASE + 0x018) as *mut u32, 0xffff_fff0);

        core::ptr::write_volatile(DBI_SELECTOR, 0x23);
        core::ptr::write_volatile((DBI_BASE + 0x114) as *mut u32, 0x4000_0000);
        core::ptr::write_volatile((DBI_BASE + 0x118) as *mut u32, 0x0000_00c0);
        core::ptr::write_volatile((DBI_BASE + 0x100) as *mut u32, 0);
        core::ptr::write_volatile((DBI_BASE + 0x104) as *mut u32, 0xc000_0100);

        core::ptr::write_volatile(DBI_SELECTOR, 0x63);
        core::ptr::write_volatile((DBI_BASE + 0x114) as *mut u32, 0x2000_0000);
        core::ptr::write_volatile((DBI_BASE + 0x118) as *mut u32, 0x0000_00c0);
        core::ptr::write_volatile((DBI_BASE + 0x100) as *mut u32, 0);
        core::ptr::write_volatile((DBI_BASE + 0x104) as *mut u32, 0xc000_0200);

        core::ptr::write_volatile(DBI_SELECTOR, 0);
        let dbi_8d4 = (DBI_BASE + 0x8d4) as *mut u32;
        let old = core::ptr::read_volatile(dbi_8d4);
        core::ptr::write_volatile(dbi_8d4, old & !(1 << 8));
        core::ptr::write_volatile(PCIE_CFG_188, 0x0100_0000);

        let old = core::ptr::read_volatile(dbi_ro_wr_en);
        core::ptr::write_volatile(dbi_ro_wr_en, old & !1);

        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old | (1 << 2));
        let old = core::ptr::read_volatile(CONTROL);
        core::ptr::write_volatile(CONTROL, old & !1);
    }

    let start = raw_timer_us();
    unsafe {
        let old = core::ptr::read_volatile(INTE);
        core::ptr::write_volatile(INTE, old | 0x48);
    }

    let mut final_monitor2 = 0;
    loop {
        let now = raw_timer_us();
        let elapsed = now.wrapping_sub(start);
        if elapsed > LINK_TIMEOUT_US {
            return State5Result {
                decision: State5Decision::LinkTimeout,
                wait_us: core::cmp::min(elapsed, u64::from(u16::MAX)) as u16,
                final_monitor2,
            };
        }

        final_monitor2 = unsafe { core::ptr::read_volatile(MONITOR2) };
        if final_monitor2 & RDLH_LINK_UP != 0 {
            return State5Result {
                decision: State5Decision::LinkUp,
                wait_us: core::cmp::min(elapsed, u64::from(u16::MAX)) as u16,
                final_monitor2,
            };
        }
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
fn read_pll_sys_snapshot() -> PllSysSnapshot {
    const PLL_SYS_BASE: usize = 0x4002_0000;

    unsafe {
        PllSysSnapshot {
            cs: core::ptr::read_volatile(PLL_SYS_BASE as *const u32),
            pwr: core::ptr::read_volatile((PLL_SYS_BASE + 0x04) as *const u32),
            fbdiv_int: core::ptr::read_volatile((PLL_SYS_BASE + 0x08) as *const u32),
            fbdiv_frac: core::ptr::read_volatile((PLL_SYS_BASE + 0x0c) as *const u32),
            prim: core::ptr::read_volatile((PLL_SYS_BASE + 0x10) as *const u32),
            sec: core::ptr::read_volatile((PLL_SYS_BASE + 0x14) as *const u32),
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
fn pll_sys_core_lock_transition() -> PllSysCoreLockResult {
    const CS: *mut u32 = 0x4002_0000 as *mut u32;
    const PWR: *mut u32 = 0x4002_0004 as *mut u32;
    const FBDIV_INT: *mut u32 = 0x4002_0008 as *mut u32;
    const FBDIV_FRAC: *mut u32 = 0x4002_000c as *mut u32;
    const LOCK: u32 = 1 << 31;
    const LOCK_TIMEOUT_US: u64 = 100_000;

    let before = read_pll_sys_snapshot();
    if before.cs != 0x0000_0001
        || before.pwr != 0x0000_003f
        || before.fbdiv_int != 0
        || before.fbdiv_frac != 0
        || before.prim != 0x0007_7000
        || before.sec != 0x8001_0000
    {
        return PllSysCoreLockResult {
            decision: PllSysCoreLockDecision::PreconditionMismatch,
            elapsed_us: 0,
            first_cs: before.cs,
            before,
            after: before,
        };
    }

    unsafe {
        core::ptr::write_volatile(PWR, 0x0000_003f);
        core::ptr::write_volatile(FBDIV_INT, 20);
        core::ptr::write_volatile(FBDIV_FRAC, 0);
        core::ptr::write_volatile(CS, 0x0000_0001);
    }

    if unsafe { core::ptr::read_volatile(FBDIV_FRAC) } != 0 {
        return PllSysCoreLockResult {
            decision: PllSysCoreLockDecision::FractionReadbackMismatch,
            elapsed_us: 0,
            first_cs: before.cs,
            before,
            after: read_pll_sys_snapshot(),
        };
    }

    unsafe {
        core::ptr::write_volatile(PWR, 0x0000_0004);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    let start = raw_timer_us();
    let first_cs = unsafe { core::ptr::read_volatile(CS) };
    let mut final_cs = first_cs;
    loop {
        if final_cs & LOCK != 0 {
            let after = read_pll_sys_snapshot();
            let decision = if after.cs == 0x8000_0001
                && after.pwr == 0x0000_0004
                && after.fbdiv_int == 20
                && after.fbdiv_frac == 0
                && after.prim == before.prim
                && after.sec == before.sec
            {
                PllSysCoreLockDecision::Locked
            } else {
                PllSysCoreLockDecision::PostReadbackMismatch
            };
            return PllSysCoreLockResult {
                decision,
                elapsed_us: core::cmp::min(raw_timer_us().wrapping_sub(start), u64::from(u32::MAX))
                    as u32,
                first_cs,
                before,
                after,
            };
        }

        let now = raw_timer_us();
        if now.wrapping_sub(start) > LOCK_TIMEOUT_US {
            return PllSysCoreLockResult {
                decision: PllSysCoreLockDecision::Timeout,
                elapsed_us: core::cmp::min(now.wrapping_sub(start), u64::from(u32::MAX)) as u32,
                first_cs,
                before,
                after: read_pll_sys_snapshot(),
            };
        }
        core::hint::spin_loop();
        final_cs = unsafe { core::ptr::read_volatile(CS) };
    }
}

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
fn enable_pll_sys_pri_ph_bit4() -> Result<(), PllSysPriPhError> {
    const PLL_SYS_CS: *const u32 = 0x4002_0000 as *const u32;
    const PLL_SYS_PRIM: *mut u32 = 0x4002_0010 as *mut u32;
    const LOCK: u32 = 1 << 31;
    const PHASE_ENABLE: u32 = 1 << 4;
    const EXPECTED_PRE: u32 = 0x0007_7000;

    unsafe {
        if core::ptr::read_volatile(PLL_SYS_CS) & LOCK == 0 {
            return Err(PllSysPriPhError::ParentNotLocked);
        }

        let old = core::ptr::read_volatile(PLL_SYS_PRIM);
        if old != EXPECTED_PRE {
            return Err(PllSysPriPhError::PreconditionMismatch);
        }

        let new = old | PHASE_ENABLE;
        core::ptr::write_volatile(PLL_SYS_PRIM, new);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if core::ptr::read_volatile(PLL_SYS_PRIM) != new {
            return Err(PllSysPriPhError::ReadbackMismatch);
        }
    }

    Ok(())
}

#[cfg(all(target_arch = "arm", feature = "uart0-reset-only"))]
fn release_uart0_reset_bank1_bit26() -> Result<(), Uart0ResetError> {
    const CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const CLEAR1: *mut u32 = 0x4001_7004 as *mut u32;
    const DONE1: *const u32 = 0x4001_401c as *const u32;
    const UART0_RESET: u32 = 1 << 26;
    const POLL_LIMIT: usize = 100_000;

    #[cfg(feature = "uart0-functional-clock-before-reset-done")]
    const CLK_UART_CTRL: *mut u32 = 0x4001_8054 as *mut u32;
    #[cfg(feature = "uart0-functional-clock-before-reset-done")]
    const CLK_UART_DIV_INT: *mut u32 = 0x4001_8058 as *mut u32;
    #[cfg(feature = "uart0-functional-clock-before-reset-done")]
    const CLK_UART_CTRL_RELEVANT: u32 = 0x0000_0fe0;
    #[cfg(feature = "uart0-functional-clock-before-reset-done")]
    const CLK_UART_SOURCE: u32 = 0x0000_0040;
    #[cfg(feature = "uart0-functional-clock-before-reset-done")]
    const CLK_UART_ENABLED: u32 = 0x0000_0840;

    unsafe {
        let ctrl = core::ptr::read_volatile(CTRL1);
        let done = core::ptr::read_volatile(DONE1);
        if ctrl & UART0_RESET == 0 || done & UART0_RESET != 0 {
            return Err(Uart0ResetError::PreconditionMismatch);
        }

        #[cfg(feature = "uart0-functional-clock-before-reset-done")]
        {
            core::ptr::write_volatile(CLK_UART_DIV_INT, 1);
            let div = core::ptr::read_volatile(CLK_UART_DIV_INT);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            if div != 1 {
                return Err(Uart0ResetError::ClockReadbackMismatch);
            }

            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_SOURCE);
            let source = core::ptr::read_volatile(CLK_UART_CTRL);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            if source & CLK_UART_CTRL_RELEVANT != CLK_UART_SOURCE {
                return Err(Uart0ResetError::ClockReadbackMismatch);
            }

            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_ENABLED);
            let enabled = core::ptr::read_volatile(CLK_UART_CTRL);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            if enabled & CLK_UART_CTRL_RELEVANT != CLK_UART_ENABLED {
                return Err(Uart0ResetError::ClockReadbackMismatch);
            }
        }

        core::ptr::write_volatile(CLEAR1, UART0_RESET);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if core::ptr::read_volatile(CTRL1) & UART0_RESET != 0 {
            return Err(Uart0ResetError::WriteRejected);
        }
    }

    for _ in 0..POLL_LIMIT {
        if unsafe { core::ptr::read_volatile(DONE1) } & UART0_RESET != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }

    Err(Uart0ResetError::Timeout)
}

#[cfg(all(target_arch = "arm", feature = "i2c1-reset-only"))]
fn release_i2c1_reset_bank0_bit8() -> Result<(), I2c1ResetError> {
    const CTRL0: *const u32 = 0x4001_4000 as *const u32;
    const CLEAR0: *mut u32 = 0x4001_7000 as *mut u32;
    const DONE0: *const u32 = 0x4001_4018 as *const u32;
    const I2C1_RESET: u32 = 1 << 8;
    const POLL_LIMIT: usize = 100_000;

    unsafe {
        let ctrl = core::ptr::read_volatile(CTRL0);
        let done = core::ptr::read_volatile(DONE0);
        if ctrl & I2C1_RESET == 0 || done & I2C1_RESET != 0 {
            return Err(I2c1ResetError::PreconditionMismatch);
        }

        core::ptr::write_volatile(CLEAR0, I2C1_RESET);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if core::ptr::read_volatile(CTRL0) & I2C1_RESET != 0 {
            return Err(I2c1ResetError::WriteRejected);
        }
    }

    for _ in 0..POLL_LIMIT {
        if unsafe { core::ptr::read_volatile(DONE0) } & I2C1_RESET != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }

    Err(I2c1ResetError::Timeout)
}

#[cfg(all(target_arch = "arm", feature = "spi0-reset-only"))]
fn release_spi0_reset_bank1_bit10() -> Result<(), Spi0ResetError> {
    const CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const CLEAR1: *mut u32 = 0x4001_7004 as *mut u32;
    const DONE1: *const u32 = 0x4001_401c as *const u32;
    const SPI0_RESET: u32 = 1 << 10;
    const POLL_LIMIT: usize = 100_000;

    unsafe {
        let ctrl = core::ptr::read_volatile(CTRL1);
        let done = core::ptr::read_volatile(DONE1);
        if ctrl & SPI0_RESET == 0 || done & SPI0_RESET != 0 {
            return Err(Spi0ResetError::PreconditionMismatch);
        }

        core::ptr::write_volatile(CLEAR1, SPI0_RESET);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if core::ptr::read_volatile(CTRL1) & SPI0_RESET != 0 {
            return Err(Spi0ResetError::WriteRejected);
        }
    }

    for _ in 0..POLL_LIMIT {
        if unsafe { core::ptr::read_volatile(DONE1) } & SPI0_RESET != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }

    Err(Spi0ResetError::Timeout)
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn emit_state3_trace_u16(pin: &mut ConfiguredPin<22, Output>, value: u16) {
    pulse_width(pin, u32::from(value as u8) + 1);
    pulse_width(pin, u32::from((value >> 8) as u8) + 1);
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
fn emit_state3_result_frame(pin: &mut ConfiguredPin<22, Output>, result: State3Result) {
    state3_phase(pin, 256); // S3T: post-decision timing frame header.
    emit_state3_trace_u16(pin, result.pre_state1_observation);
    emit_state3_trace_u16(pin, result.perstn_wait_us);
    emit_state3_trace_u16(pin, result.perstn_observation);
    emit_state3_trace_u16(pin, result.boundary_us);
    emit_state3_trace_u16(pin, result.cperstn_to_first_poll_us);

    match result.decision {
        State3Decision::CoreAlive => state3_phase(pin, 128), // S3F: final MMIO action.
        State3Decision::CoreAliveTimeout => state3_phase(pin, 160),
        State3Decision::PerstnTimeout => state3_phase(pin, 192),
        State3Decision::PreState1ReadbackMismatch => state3_phase(pin, 224),
    }
}

#[cfg(all(target_arch = "arm", feature = "state5-composite-boundary"))]
#[cfg_attr(feature = "uart-reset-irq-map-proof", allow(dead_code))]
fn emit_state5_result_frame(
    pin: &mut ConfiguredPin<22, Output>,
    state3: State3Result,
    state5: State5Result,
) {
    state3_phase(pin, 288); // S5T: post-decision state-5 frame header.
    emit_state3_trace_u16(pin, state3.pre_state1_observation);
    emit_state3_trace_u16(pin, state3.perstn_wait_us);
    emit_state3_trace_u16(pin, state3.perstn_observation);
    emit_state3_trace_u16(pin, state3.boundary_us);
    emit_state3_trace_u16(pin, state3.cperstn_to_first_poll_us);
    emit_state3_trace_u16(pin, state5.wait_us);
    emit_state3_trace_u16(pin, state5.final_monitor2 as u16);
    emit_state3_trace_u16(pin, (state5.final_monitor2 >> 16) as u16);

    match state5.decision {
        State5Decision::LinkUp => state3_phase(pin, 272), // S5F: final MMIO action.
        State5Decision::LinkTimeout => state3_phase(pin, 304), // S5X: final MMIO action.
    }
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
#[allow(dead_code)]
fn emit_pre_state1_bitwise_group(pin: &mut ConfiguredPin<22, Output>, count: u8) {
    for _ in 0..count {
        pulse_width(pin, 16);
    }
    delay_readback_units(64);
}

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
#[allow(dead_code)]
fn emit_pre_state1_bitwise_readback_frame(
    pin: &mut ConfiguredPin<22, Output>,
    result: PreState1Result,
) {
    const REQUIRED_PREDICATES: u16 = 0x01ff;
    const PCIE_AUX_SOURCE_MASK: u32 = 0x0000_01e3;
    const PCIE_AUX_ENABLE: u32 = 1 << 11;
    const SOURCE_BIT_POSITIONS: [u32; 6] = [0, 1, 5, 6, 7, 8];

    emit_pre_state1_bitwise_group(pin, 13); // AUX_SEL snapshot header.
    for snapshot in [result.clock_ctrl_before, result.clock_ctrl] {
        for bit in SOURCE_BIT_POSITIONS {
            emit_pre_state1_bitwise_group(pin, if snapshot & (1 << bit) != 0 { 2 } else { 1 });
        }
        emit_pre_state1_bitwise_group(
            pin,
            if snapshot & PCIE_AUX_ENABLE != 0 {
                2
            } else {
                1
            },
        );
    }
    for bit in 0..9 {
        emit_pre_state1_bitwise_group(
            pin,
            if result.clock_sel & (1 << bit) != 0 {
                2
            } else {
                1
            },
        );
    }
    emit_pre_state1_bitwise_group(pin, if result.ready { 6 } else { 5 });

    debug_assert_eq!(PCIE_AUX_SOURCE_MASK, 0x0000_01e3);
    debug_assert_eq!(
        result.ready,
        result.observation & REQUIRED_PREDICATES == REQUIRED_PREDICATES
    );
}

#[cfg(all(
    target_arch = "arm",
    any(feature = "state3-composite-boundary", feature = "gpio22-start-proof")
))]
fn quiet_stop() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    feature = "bar2-readonly-handshake",
    not(feature = "uart0-rx-irq"),
    not(feature = "rp1-linux-clk-uart-ownership-conflict")
))]
fn publish_bar2_readonly_identity(flags: u32) {
    #[cfg(feature = "debug-mailbox-layout-v1")]
    const MAILBOX_LIMIT: usize = rp1_hal::debug::COEXISTENCE_PRIVATE_SIZE;
    #[cfg(not(feature = "debug-mailbox-layout-v1"))]
    const MAILBOX_LIMIT: usize = rp1_hal::debug::MAILBOX_SIZE;
    const _: () = assert!(core::mem::size_of::<rp1_hal::debug::DebugMailbox>() <= MAILBOX_LIMIT);

    let header = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        // Magic is the commit marker. Host readers never accept a partial header.
        core::ptr::write_volatile(header, 0);
        core::ptr::write_volatile(header.add(1), rp1_hal::debug::VERSION);
        core::ptr::write_volatile(
            header.add(2),
            core::mem::size_of::<rp1_hal::debug::DebugMailbox>() as u32,
        );
        core::ptr::write_volatile(header.add(3), flags);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(header, rp1_hal::debug::MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "raw-timer-proof"))]
fn publish_raw_timer_proof(immediate_delta_us: u64, delay_delta_us: u64) {
    const MAGIC: u32 = 0x3152_4d54; // TMR1
    const REQUESTED_DELAY_US: u32 = 1_000;
    const RESULT_WORDS: usize = 4;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        core::ptr::write_volatile(words.add(1), immediate_delta_us as u32);
        core::ptr::write_volatile(words.add(2), delay_delta_us as u32);
        core::ptr::write_volatile(words.add(3), REQUESTED_DELAY_US);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "watchdog-proof"))]
#[derive(Clone, Copy)]
struct WatchdogProofResult {
    flags: u32,
    ctrl_before: u32,
    reason_before: u32,
    scratch_before: u32,
    scratch_test: u32,
    scratch_restored: u32,
    legacy_tick_candidate: u32,
    ticks_ctrl: u32,
    ticks_cycles: u32,
    ticks_count_before: u32,
    ticks_count_after: u32,
    ctrl_enabled: u32,
    ctrl_progress: u32,
    ctrl_disabled: u32,
    reason_after: u32,
}

#[cfg(all(target_arch = "arm", feature = "watchdog-proof"))]
fn watchdog_hardware_proof() -> WatchdogProofResult {
    const WATCHDOG_BASE: usize = 0x4015_4000;
    const CTRL: *mut u32 = WATCHDOG_BASE as *mut u32;
    const LOAD: *mut u32 = (WATCHDOG_BASE + 0x04) as *mut u32;
    const REASON: *const u32 = (WATCHDOG_BASE + 0x08) as *const u32;
    const SCRATCH7: *mut u32 = (WATCHDOG_BASE + 0x28) as *mut u32;
    const LEGACY_TICK_CANDIDATE: *const u32 = (WATCHDOG_BASE + 0x2c) as *const u32;
    const TICKS_WATCHDOG_CTRL: *const u32 = 0x4017_400c as *const u32;
    const TICKS_WATCHDOG_CYCLES: *const u32 = 0x4017_4010 as *const u32;
    const TICKS_WATCHDOG_COUNT: *const u32 = 0x4017_4014 as *const u32;
    const CTRL_TIME_MASK: u32 = 0x00ff_ffff;
    const CTRL_ENABLE: u32 = 1 << 30;
    const CTRL_TRIGGER: u32 = 1 << 31;
    const LOAD_MAX: u32 = CTRL_TIME_MASK;
    const SCRATCH_SENTINEL: u32 = 0x5744_4731;

    unsafe {
        let ctrl_before = core::ptr::read_volatile(CTRL);
        let reason_before = core::ptr::read_volatile(REASON);
        let scratch_before = core::ptr::read_volatile(SCRATCH7);
        let legacy_tick_candidate = core::ptr::read_volatile(LEGACY_TICK_CANDIDATE);
        let ticks_ctrl = core::ptr::read_volatile(TICKS_WATCHDOG_CTRL);
        let ticks_cycles = core::ptr::read_volatile(TICKS_WATCHDOG_CYCLES);
        let ticks_count_before = core::ptr::read_volatile(TICKS_WATCHDOG_COUNT);
        let mut ticks_count_after = ticks_count_before;
        let mut flags = 0u32;

        for _ in 0..256 {
            ticks_count_after = core::ptr::read_volatile(TICKS_WATCHDOG_COUNT);
            if ticks_count_after != ticks_count_before {
                flags |= 1 << 4;
                break;
            }
        }
        if ticks_ctrl & 0x3 == 0x3 {
            flags |= 1 << 3;
        }

        core::ptr::write_volatile(SCRATCH7, SCRATCH_SENTINEL);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        let scratch_test = core::ptr::read_volatile(SCRATCH7);
        if scratch_test == SCRATCH_SENTINEL {
            flags |= 1;
        }
        core::ptr::write_volatile(SCRATCH7, scratch_before);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        let scratch_restored = core::ptr::read_volatile(SCRATCH7);
        if scratch_restored == scratch_before {
            flags |= 1 << 1;
        }

        let mut ctrl_enabled = ctrl_before;
        let mut ctrl_progress = ctrl_before;
        let mut ctrl_disabled = ctrl_before;
        if ctrl_before & CTRL_ENABLE == 0 {
            core::ptr::write_volatile(LOAD, LOAD_MAX);
            core::ptr::write_volatile(CTRL, (ctrl_before & !CTRL_TRIGGER) | CTRL_ENABLE);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            ctrl_enabled = core::ptr::read_volatile(CTRL);
            busy_wait_us(256);
            ctrl_progress = core::ptr::read_volatile(CTRL);
            core::ptr::write_volatile(CTRL, ctrl_before & !CTRL_TRIGGER);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            ctrl_disabled = core::ptr::read_volatile(CTRL);

            if ctrl_enabled & CTRL_ENABLE != 0 {
                flags |= 1 << 5;
            }
            if ctrl_progress & CTRL_TIME_MASK < ctrl_enabled & CTRL_TIME_MASK {
                flags |= 1 << 6;
            }
            if ctrl_disabled & CTRL_ENABLE == 0 {
                flags |= 1 << 7;
            }
        } else {
            flags |= 1 << 31;
        }

        let reason_after = core::ptr::read_volatile(REASON);
        if reason_after == reason_before {
            flags |= 1 << 8;
        }

        WatchdogProofResult {
            flags,
            ctrl_before,
            reason_before,
            scratch_before,
            scratch_test,
            scratch_restored,
            legacy_tick_candidate,
            ticks_ctrl,
            ticks_cycles,
            ticks_count_before,
            ticks_count_after,
            ctrl_enabled,
            ctrl_progress,
            ctrl_disabled,
            reason_after,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "watchdog-proof"))]
fn publish_watchdog_proof(result: WatchdogProofResult) {
    const MAGIC: u32 = 0x3147_4457; // WDG1
    const RESULT_WORDS: usize = 16;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
    let fields = [
        result.flags,
        result.ctrl_before,
        result.reason_before,
        result.scratch_before,
        result.scratch_test,
        result.scratch_restored,
        result.legacy_tick_candidate,
        result.ticks_ctrl,
        result.ticks_cycles,
        result.ticks_count_before,
        result.ticks_count_after,
        result.ctrl_enabled,
        result.ctrl_progress,
        result.ctrl_disabled,
        result.reason_after,
    ];
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "watchdog-scratch-proof"))]
fn watchdog_scratch_hardware_proof() -> [u32; 15] {
    const WATCHDOG_BASE: usize = 0x4015_4000;
    const SCRATCH: [(usize, u32); 4] = [
        (WATCHDOG_BASE + 0x14, 0x3253_4457),
        (WATCHDOG_BASE + 0x1c, 0x3453_4457),
        (WATCHDOG_BASE + 0x20, 0x3553_4457),
        (WATCHDOG_BASE + 0x24, 0x3653_4457),
    ];
    const IDENTIFICATION_TAG: *const u32 = (WATCHDOG_BASE + 0x2c) as *const u32;
    const REASON: *const u32 = (WATCHDOG_BASE + 0x08) as *const u32;

    let mut fields = [0u32; 15];
    unsafe {
        for (index, (address, sentinel)) in SCRATCH.into_iter().enumerate() {
            let register = address as *mut u32;
            let before = core::ptr::read_volatile(register);
            core::ptr::write_volatile(register, sentinel);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let test = core::ptr::read_volatile(register);
            core::ptr::write_volatile(register, before);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let restored = core::ptr::read_volatile(register);
            let base = 1 + index * 3;
            fields[base] = before;
            fields[base + 1] = test;
            fields[base + 2] = restored;
            if test == sentinel && restored == before {
                fields[0] |= 1 << index;
            }
        }
        fields[13] = core::ptr::read_volatile(IDENTIFICATION_TAG);
        fields[14] = core::ptr::read_volatile(REASON);
    }
    fields
}

#[cfg(all(target_arch = "arm", feature = "watchdog-scratch-proof"))]
fn publish_watchdog_scratch_proof(fields: [u32; 15]) {
    const MAGIC: u32 = 0x3153_4457; // WDS1
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "timer-register-proof"))]
fn timer_register_hardware_proof() -> [u32; 15] {
    const TIMER_BASE: usize = 0x400a_c000;
    const TIMEHR: *const u32 = (TIMER_BASE + 0x08) as *const u32;
    const TIMELR: *const u32 = (TIMER_BASE + 0x0c) as *const u32;
    const ALARMS: [*mut u32; 4] = [
        (TIMER_BASE + 0x10) as *mut u32,
        (TIMER_BASE + 0x14) as *mut u32,
        (TIMER_BASE + 0x18) as *mut u32,
        (TIMER_BASE + 0x1c) as *mut u32,
    ];
    const ARMED: *const u32 = (TIMER_BASE + 0x20) as *const u32;
    const DEBUG_PAUSE: *const u32 = (TIMER_BASE + 0x2c) as *const u32;
    const PAUSE: *const u32 = (TIMER_BASE + 0x30) as *const u32;
    const INTR: *mut u32 = (TIMER_BASE + 0x34) as *mut u32;
    const INTE: *const u32 = (TIMER_BASE + 0x38) as *const u32;
    const INTF: *const u32 = (TIMER_BASE + 0x3c) as *const u32;
    const INTS: *const u32 = (TIMER_BASE + 0x40) as *const u32;

    let mut fields = [0u32; 15];
    unsafe {
        let timelr = core::ptr::read_volatile(TIMELR);
        let timehr = core::ptr::read_volatile(TIMEHR);
        let raw = raw_timer_us();
        let raw_low = raw as u32;
        let raw_high = (raw >> 32) as u32;
        let delta = raw_low.wrapping_sub(timelr);
        if timehr == raw_high && delta <= 16 {
            fields[0] |= 1;
        }

        fields[1] = timelr;
        fields[2] = raw_low;
        fields[3] = core::ptr::read_volatile(DEBUG_PAUSE);
        fields[4] = core::ptr::read_volatile(PAUSE);
        fields[5] = core::ptr::read_volatile(ARMED);
        fields[6] = core::ptr::read_volatile(INTR);
        fields[11] = core::ptr::read_volatile(INTE);
        fields[12] = core::ptr::read_volatile(INTF);
        fields[13] = core::ptr::read_volatile(INTS);

        if fields[5] & 0xf == 0 && fields[11] & 0xf == 0 {
            core::ptr::write_volatile(INTR, fields[6] & 0xf);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            if core::ptr::read_volatile(INTR) & 0xf == 0 {
                fields[0] |= 1 << 6;
            } else {
                return fields;
            }
            fields[0] |= 1 << 1;
            let proof_start = raw_timer_us();
            for (index, alarm) in ALARMS.into_iter().enumerate() {
                let bit = 1u32 << index;
                let target = (raw_timer_us() as u32).wrapping_add(1_000);
                core::ptr::write_volatile(alarm, target);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                fields[7] |= core::ptr::read_volatile(ARMED) & bit;

                let wait_start = raw_timer_us();
                while raw_timer_us().wrapping_sub(wait_start) <= 5_000 {
                    if core::ptr::read_volatile(INTR) & bit != 0 {
                        fields[8] |= bit;
                        break;
                    }
                    core::hint::spin_loop();
                }
                fields[9] |= core::ptr::read_volatile(ARMED) & bit;
                core::ptr::write_volatile(INTR, bit);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                if core::ptr::read_volatile(INTR) & bit == 0 {
                    fields[10] |= bit;
                }
            }
            fields[14] = core::cmp::min(
                raw_timer_us().wrapping_sub(proof_start),
                u64::from(u32::MAX),
            ) as u32;
            if fields[7] == 0xf {
                fields[0] |= 1 << 2;
            }
            if fields[8] == 0xf {
                fields[0] |= 1 << 3;
            }
            if fields[9] == 0 {
                fields[0] |= 1 << 4;
            }
            if fields[10] == 0xf {
                fields[0] |= 1 << 5;
            }
        }
    }
    fields
}

#[cfg(all(target_arch = "arm", feature = "timer-register-proof"))]
fn publish_timer_register_proof(fields: [u32; 15]) {
    const MAGIC: u32 = 0x3152_4d54; // TMR1
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "timer-writable-time-proof"))]
unsafe fn read_latched_timer_time() -> u64 {
    const TIMEHR: *const u32 = (0x400a_c000 + 0x08) as *const u32;
    const TIMELR: *const u32 = (0x400a_c000 + 0x0c) as *const u32;

    let low = unsafe { core::ptr::read_volatile(TIMELR) };
    let high = unsafe { core::ptr::read_volatile(TIMEHR) };
    (u64::from(high) << 32) | u64::from(low)
}

#[cfg(all(target_arch = "arm", feature = "timer-writable-time-proof"))]
fn timer_writable_time_hardware_proof() -> [u32; 15] {
    const TIMER_BASE: usize = 0x400a_c000;
    const TIMEHW: *mut u32 = TIMER_BASE as *mut u32;
    const TIMELW: *mut u32 = (TIMER_BASE + 0x04) as *mut u32;
    const ARMED: *const u32 = (TIMER_BASE + 0x20) as *const u32;
    const INTE: *const u32 = (TIMER_BASE + 0x38) as *const u32;
    const TEST_LOW_DELTA: u32 = 1_000_000;
    const MAX_OBSERVATION_DELTA_US: u64 = 128;
    const RESTORE_LEAD_US: u64 = 1_000;
    const REQUIRED_FLAGS: u32 = 0x7f;

    let mut fields = [0u32; 15];
    unsafe {
        let armed = core::ptr::read_volatile(ARMED) & 0xf;
        let inte = core::ptr::read_volatile(INTE) & 0xf;
        fields[14] = armed | (inte << 16);
        if armed != 0 || inte != 0 {
            return fields;
        }
        fields[0] |= 1;

        let raw_before = raw_timer_us();
        let before = read_latched_timer_time();
        let before_low = before as u32;
        let before_high = (before >> 32) as u32;
        fields[1] = before_low;
        fields[2] = before_high;

        let test_low = before_low.wrapping_add(TEST_LOW_DELTA);
        let test_high = before_high.wrapping_add(1);
        let test = (u64::from(test_high) << 32) | u64::from(test_low);
        fields[3] = test_low;
        fields[4] = test_high;

        core::ptr::write_volatile(TIMELW, test_low);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        let raw_after_stage = raw_timer_us();
        let staged = read_latched_timer_time();
        fields[5] = staged as u32;
        fields[6] = (staged >> 32) as u32;
        let expected_staged = before.wrapping_add(raw_after_stage.wrapping_sub(raw_before));
        let staged_delta = staged.abs_diff(expected_staged);
        if staged_delta <= MAX_OBSERVATION_DELTA_US && fields[6] == before_high {
            fields[0] |= 1 << 1;
        }

        core::ptr::write_volatile(TIMEHW, test_high);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        let committed = read_latched_timer_time();
        fields[7] = committed as u32;
        fields[8] = (committed >> 32) as u32;
        let commit_delta = committed.wrapping_sub(test);
        fields[11] = core::cmp::min(commit_delta, u64::from(u32::MAX)) as u32;
        if fields[8] == test_high {
            fields[0] |= 1 << 2;
        }
        if commit_delta <= MAX_OBSERVATION_DELTA_US {
            fields[0] |= 1 << 3;
        }

        let restore_target = before.wrapping_add(RESTORE_LEAD_US);
        let restore_low = restore_target as u32;
        let restore_high = (restore_target >> 32) as u32;
        core::ptr::write_volatile(TIMELW, restore_low);
        core::ptr::write_volatile(TIMEHW, restore_high);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        let restored = read_latched_timer_time();
        fields[9] = restored as u32;
        fields[10] = (restored >> 32) as u32;
        let restore_delta = restored.wrapping_sub(restore_target);
        fields[12] = core::cmp::min(restore_delta, u64::from(u32::MAX)) as u32;
        if fields[10] == restore_high {
            fields[0] |= 1 << 4;
        }
        if restore_delta <= MAX_OBSERVATION_DELTA_US {
            fields[0] |= 1 << 5;
        }

        let raw_elapsed = raw_timer_us().wrapping_sub(raw_before);
        fields[13] = core::cmp::min(raw_elapsed, u64::from(u32::MAX)) as u32;
        if raw_elapsed > 0 && raw_elapsed <= 2_000 {
            fields[0] |= 1 << 6;
        }
        if fields[0] & REQUIRED_FLAGS != REQUIRED_FLAGS {
            return fields;
        }
    }
    fields
}

#[cfg(all(target_arch = "arm", feature = "timer-writable-time-proof"))]
fn publish_timer_writable_time_proof(fields: [u32; 15]) {
    const MAGIC: u32 = 0x3157_4d54; // TMW1
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
#[derive(Clone, Copy)]
struct Timer0InterruptState {
    armed: u32,
    intr: u32,
    inte: u32,
    intf: u32,
    ints: u32,
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
impl Timer0InterruptState {
    fn clean(self) -> bool {
        self.armed == 0 && self.intr == 0 && self.inte == 0 && self.intf == 0 && self.ints == 0
    }

    fn pack(self) -> u32 {
        (self.armed & 0xf)
            | ((self.intr & 0xf) << 4)
            | ((self.inte & 0xf) << 8)
            | ((self.intf & 0xf) << 12)
            | ((self.ints & 0xf) << 16)
            | (u32::from(self.armed & !0xf != 0) << 20)
            | (u32::from(self.intr & !0xf != 0) << 21)
            | (u32::from(self.inte & !0xf != 0) << 22)
            | (u32::from(self.intf & !0xf != 0) << 23)
            | (u32::from(self.ints & !0xf != 0) << 24)
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "timer0-inte-ints-proof",
        feature = "timer0-alarm0-local-irq26-candidate"
    )
))]
fn read_timer0_interrupt_state() -> Timer0InterruptState {
    const TIMER_BASE: usize = 0x400a_c000;
    const ARMED: *const u32 = (TIMER_BASE + 0x20) as *const u32;
    const INTR: *const u32 = (TIMER_BASE + 0x34) as *const u32;
    const INTE: *const u32 = (TIMER_BASE + 0x38) as *const u32;
    const INTF: *const u32 = (TIMER_BASE + 0x3c) as *const u32;
    const INTS: *const u32 = (TIMER_BASE + 0x40) as *const u32;

    unsafe {
        Timer0InterruptState {
            armed: core::ptr::read_volatile(ARMED),
            intr: core::ptr::read_volatile(INTR),
            inte: core::ptr::read_volatile(INTE),
            intf: core::ptr::read_volatile(INTF),
            ints: core::ptr::read_volatile(INTS),
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "timer0-inte-ints-proof"))]
fn timer0_inte_ints_proof() -> [u32; 15] {
    const TIMER_BASE: usize = 0x400a_c000;
    const ALARM0: *mut u32 = (TIMER_BASE + 0x10) as *mut u32;
    const INTR: *mut u32 = (TIMER_BASE + 0x34) as *mut u32;
    const INTE: *mut u32 = (TIMER_BASE + 0x38) as *mut u32;
    const BIT0: u32 = 1;
    const SCHEDULE_DELTA_US: u32 = 20_000;
    const POLL_TIMEOUT_US: u64 = 250_000;
    const PASS: u32 = 1;
    const PRECONDITION: u32 = 0x501;
    const INTE_READBACK: u32 = 0x502;
    const ARM_FAILURE: u32 = 0x503;
    const RAW_TIMEOUT: u32 = 0x504;
    const FILTER_MISMATCH: u32 = 0x505;
    const FINAL_CLEANUP: u32 = 0x506;
    const REQUIRED_FLAGS: u32 = 0x0fff;

    let mut fields = [0u32; 15];
    let mut decision = PASS;
    let mut flags = 0u32;
    let mut inte_write_attempted = false;
    let mut alarm_write_attempted = false;
    let raw_before = raw_timer_us();
    let target = (raw_before as u32).wrapping_add(SCHEDULE_DELTA_US);
    let pre = read_timer0_interrupt_state();

    fields[1] = pre.pack();
    fields[3] = raw_before as u32;
    fields[4] = target;
    fields[12] = SCHEDULE_DELTA_US;
    fields[13] = pre.intr & 0xf;

    unsafe {
        if pre.armed == 0
            && pre.intr & !0xf == 0
            && pre.inte == 0
            && pre.intf == 0
            && pre.ints == 0
        {
            flags |= 1;
        } else {
            decision = PRECONDITION;
        }

        if decision == PASS && pre.intr != 0 {
            core::ptr::write_volatile(INTR, pre.intr);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let scrubbed = read_timer0_interrupt_state();
            if !scrubbed.clean() {
                fields[2] = scrubbed.pack();
                decision = PRECONDITION;
            }
        }

        if decision == PASS {
            inte_write_attempted = true;
            core::ptr::write_volatile(INTE, BIT0);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let enabled = read_timer0_interrupt_state();
            fields[2] = enabled.pack();
            if enabled.armed == 0
                && enabled.intr == 0
                && enabled.inte == BIT0
                && enabled.intf == 0
                && enabled.ints == 0
            {
                flags |= 1 << 1;
            } else if enabled.inte != BIT0 {
                decision = INTE_READBACK;
            } else {
                decision = FILTER_MISMATCH;
            }
        }

        if decision == PASS {
            alarm_write_attempted = true;
            core::ptr::write_volatile(ALARM0, target);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            let after_alarm = read_timer0_interrupt_state();
            fields[5] = after_alarm.pack();
            if after_alarm.armed == BIT0
                && after_alarm.intr == 0
                && after_alarm.inte == BIT0
                && after_alarm.intf == 0
                && after_alarm.ints == 0
            {
                flags |= 1 << 2;
            } else if after_alarm.armed & BIT0 == 0 {
                decision = ARM_FAILURE;
            } else {
                decision = FILTER_MISMATCH;
            }
        }

        if decision == PASS {
            let start = raw_timer_us();
            loop {
                let state = read_timer0_interrupt_state();
                if state.intr & BIT0 != 0 && state.ints & BIT0 != 0 {
                    break;
                }
                if raw_timer_us().wrapping_sub(start) > POLL_TIMEOUT_US {
                    break;
                }
                core::hint::spin_loop();
            }
            let terminal = read_timer0_interrupt_state();
            fields[6] = terminal.pack();
            fields[7] =
                core::cmp::min(raw_timer_us().wrapping_sub(start), u64::from(u32::MAX)) as u32;
            if terminal.intr & BIT0 != 0 {
                flags |= 1 << 3;
            }
            if terminal.ints & BIT0 != 0 {
                flags |= 1 << 4;
            }
            if terminal.armed & BIT0 == 0 {
                flags |= 1 << 5;
            }
            if terminal.intr & BIT0 == 0 {
                decision = RAW_TIMEOUT;
            } else if terminal.armed != 0
                || terminal.intr != BIT0
                || terminal.inte != BIT0
                || terminal.intf != 0
                || terminal.ints != BIT0
            {
                decision = FILTER_MISMATCH;
            }
        }

        if inte_write_attempted {
            core::ptr::write_volatile(INTE, 0);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        let masked = read_timer0_interrupt_state();
        fields[8] = masked.pack();
        if masked.inte == 0 {
            flags |= 1 << 6;
        }
        if masked.intr & BIT0 != 0 {
            flags |= 1 << 7;
        }
        if masked.ints & BIT0 == 0 {
            flags |= 1 << 8;
        }
        if decision == PASS
            && (masked.armed != 0
                || masked.intr != BIT0
                || masked.inte != 0
                || masked.intf != 0
                || masked.ints != 0)
        {
            decision = FILTER_MISMATCH;
        }

        if alarm_write_attempted && masked.intr & BIT0 != 0 {
            core::ptr::write_volatile(INTR, BIT0);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        let cleared = read_timer0_interrupt_state();
        fields[9] = cleared.pack();
        if cleared.clean() {
            flags |= 1 << 9;
        }

        let final_state = read_timer0_interrupt_state();
        let raw_after = raw_timer_us();
        fields[10] = final_state.pack();
        fields[11] = raw_after as u32;
        if final_state.clean() {
            flags |= 1 << 10;
        }
        if raw_after.wrapping_sub(raw_before) > 0 {
            flags |= 1 << 11;
        }
        if flags & REQUIRED_FLAGS == REQUIRED_FLAGS && decision == PASS {
            flags |= 1 << 31;
        } else if decision == PASS {
            decision = FINAL_CLEANUP;
        }
    }

    fields[0] = decision;
    fields[14] = flags;
    fields
}

#[cfg(all(target_arch = "arm", feature = "timer0-inte-ints-proof"))]
fn publish_timer0_inte_ints_proof(fields: [u32; 15]) {
    const MAGIC: u32 = 0x3153_4954; // TIS1
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-local-irq26-candidate"))]
mod timer0_alarm0_local_irq26_candidate {
    use core::sync::atomic::{AtomicU32, Ordering};

    const TIMER_BASE: usize = 0x400a_c000;
    const ALARM0: *mut u32 = (TIMER_BASE + 0x10) as *mut u32;
    const INTR: *mut u32 = (TIMER_BASE + 0x34) as *mut u32;
    const INTE: *mut u32 = (TIMER_BASE + 0x38) as *mut u32;
    const BIT0: u32 = 1;
    const SCHEDULE_DELTA_US: u32 = 20_000;
    const IRQ_TIMEOUT_US: u64 = 250_000;
    const STABILITY_US: u64 = 4_000;
    const PASS: u32 = 1;
    const PRECONDITION: u32 = 0x601;
    const ROUTE: u32 = 0x602;
    const INTE_READBACK: u32 = 0x603;
    const ALARM_READBACK: u32 = 0x604;
    const RAW_ONLY_TIMEOUT: u32 = 0x605;
    const HANDLER_MISMATCH: u32 = 0x606;
    const TIMER_MISMATCH: u32 = 0x607;
    const TIMER_CLEANUP: u32 = 0x608;
    const ROUTE_CLEANUP: u32 = 0x609;
    const REQUIRED_FLAGS: u32 = 0x1fff;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static FIRST_IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_TIMER: AtomicU32 = AtomicU32::new(0);
    static FIRST_ROUTE: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let timer = super::read_timer0_interrupt_state();
        let route = rp1_rt::timer0_alarm0_irq26_candidate_route_snapshot();
        let old = COUNT.load(Ordering::Relaxed);
        if old == 0 {
            FIRST_IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_TIMER.store(timer.pack(), Ordering::Relaxed);
            FIRST_ROUTE.store(route.pack(), Ordering::Relaxed);
        }

        unsafe {
            core::ptr::write_volatile(INTE, 0);
            if timer.intr & BIT0 != 0 {
                core::ptr::write_volatile(INTR, BIT0);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        COUNT.store(old.wrapping_add(1), Ordering::Release);
    }

    pub fn run_and_publish() -> u32 {
        let mut fields = [0u32; 15];
        let mut decision = PASS;
        let mut flags = 0u32;
        let mut timer_write_attempted = false;
        let mut inte_write_attempted = false;
        let mut alarm_write_attempted = false;
        let mut route_prepare_attempted = false;
        let mut wait_start = 0u64;

        COUNT.store(0, Ordering::Relaxed);
        FIRST_IPSR.store(0, Ordering::Relaxed);
        FIRST_TIMER.store(0, Ordering::Relaxed);
        FIRST_ROUTE.store(0, Ordering::Relaxed);

        let pre = super::read_timer0_interrupt_state();
        let route_before = rp1_rt::timer0_alarm0_irq26_candidate_route_snapshot();
        fields[3] = pre.pack();

        let timer_safe = pre.armed == 0
            && pre.intr & !0xf == 0
            && pre.inte == 0
            && pre.intf == 0
            && pre.ints == 0;
        let route_safe = route_before.pack() == 0x10;
        if !timer_safe {
            decision = PRECONDITION;
        } else if !route_safe {
            decision = ROUTE;
        }

        unsafe {
            if decision == PASS && pre.intr != 0 {
                core::ptr::write_volatile(INTR, pre.intr & 0xf);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                if !super::read_timer0_interrupt_state().clean() {
                    decision = PRECONDITION;
                }
            }

            if decision == PASS {
                flags |= 1;
                route_prepare_attempted = true;
                if rp1_rt::prepare_timer0_alarm0_irq26_candidate()
                    && rp1_rt::timer0_alarm0_irq26_candidate_route_snapshot().pack() == 0x10
                {
                    flags |= 1 << 1;
                } else {
                    decision = ROUTE;
                }
            }

            if decision == PASS {
                timer_write_attempted = true;
                inte_write_attempted = true;
                core::ptr::write_volatile(INTE, BIT0);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                let enabled = super::read_timer0_interrupt_state();
                fields[4] = enabled.pack();
                if enabled.pack() == 0x100 {
                    flags |= 1 << 2;
                } else {
                    decision = INTE_READBACK;
                }
            }

            if decision == PASS {
                let target = (super::raw_timer_us() as u32).wrapping_add(SCHEDULE_DELTA_US);
                alarm_write_attempted = true;
                core::ptr::write_volatile(ALARM0, target);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                let armed = super::read_timer0_interrupt_state();
                fields[5] = armed.pack();
                if armed.pack() == 0x101 {
                    flags |= 1 << 3;
                } else {
                    decision = ALARM_READBACK;
                }
            }

            if decision == PASS {
                wait_start = super::raw_timer_us();
                rp1_rt::enable_timer0_alarm0_irq26_candidate();
                let enabled_route = rp1_rt::timer0_alarm0_irq26_candidate_route_snapshot();
                fields[6] = enabled_route.pack();
                if enabled_route.pack() != 0x11 {
                    decision = ROUTE;
                }
            }
        }

        if alarm_write_attempted {
            if wait_start == 0 {
                wait_start = super::raw_timer_us();
            }
            loop {
                let elapsed = super::raw_timer_us().wrapping_sub(wait_start);
                let state = super::read_timer0_interrupt_state();
                let handler_seen = COUNT.load(Ordering::Acquire) != 0;
                if (decision == PASS && handler_seen)
                    || (decision != PASS && state.armed & BIT0 == 0)
                    || elapsed >= IRQ_TIMEOUT_US
                {
                    fields[13] = core::cmp::min(elapsed, IRQ_TIMEOUT_US) as u32;
                    break;
                }
                core::hint::spin_loop();
            }
        }

        let terminal = super::read_timer0_interrupt_state();
        fields[9] = terminal.pack();
        let count_before_stability = COUNT.load(Ordering::Acquire);
        if alarm_write_attempted {
            super::busy_wait_us(STABILITY_US);
            if COUNT.load(Ordering::Acquire) == count_before_stability {
                flags |= 1 << 9;
            }
        }

        unsafe {
            if inte_write_attempted {
                core::ptr::write_volatile(INTE, 0);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
        }
        let masked = super::read_timer0_interrupt_state();
        fields[10] = masked.pack();
        if (count_before_stability != 0 && masked.clean())
            || (count_before_stability == 0 && terminal.pack() == 0x1_0110 && masked.pack() == 0x10)
        {
            flags |= 1 << 8;
        }

        unsafe {
            if alarm_write_attempted && masked.intr & BIT0 != 0 {
                core::ptr::write_volatile(INTR, BIT0);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            if route_prepare_attempted {
                rp1_rt::disable_timer0_alarm0_irq26_candidate();
            }
        }

        let final_timer = super::read_timer0_interrupt_state();
        let final_route = rp1_rt::timer0_alarm0_irq26_candidate_route_snapshot();
        let final_count = COUNT.load(Ordering::Acquire);
        let first_ipsr = FIRST_IPSR.load(Ordering::Relaxed);
        let first_timer = FIRST_TIMER.load(Ordering::Relaxed);
        let first_route = FIRST_ROUTE.load(Ordering::Relaxed);
        fields[1] = final_count;
        fields[2] = first_ipsr;
        fields[7] = first_timer;
        fields[8] = first_route;
        fields[11] = final_timer.pack();
        fields[12] = final_route.pack();

        if final_count == 1 {
            flags |= 1 << 4;
        }
        if first_ipsr == rp1_rt::TIMER0_ALARM0_IRQ26_CANDIDATE_VECTOR_INDEX as u32 {
            flags |= 1 << 5;
        }
        if first_timer == 0x1_0110 {
            flags |= 1 << 6;
        }
        if first_route == 0x15 || first_route == 0x17 {
            flags |= 1 << 7;
        }
        if final_timer.clean() {
            flags |= 1 << 10;
        }
        if final_route.pack() == 0x10 {
            flags |= 1 << 11;
        }
        if fields[13] > 0 && fields[13] <= IRQ_TIMEOUT_US as u32 {
            flags |= 1 << 12;
        }

        if timer_write_attempted && !final_timer.clean() {
            decision = TIMER_CLEANUP;
        } else if route_prepare_attempted && final_route.pack() != 0x10 {
            decision = ROUTE_CLEANUP;
        } else if decision == PASS {
            if final_count == 0 {
                decision = if terminal.pack() == 0x1_0110 {
                    RAW_ONLY_TIMEOUT
                } else {
                    TIMER_MISMATCH
                };
            } else if final_count != 1
                || first_ipsr != rp1_rt::TIMER0_ALARM0_IRQ26_CANDIDATE_VECTOR_INDEX as u32
                || first_timer != 0x1_0110
                || !(first_route == 0x15 || first_route == 0x17)
                || !masked.clean()
                || flags & (1 << 9) == 0
            {
                decision = HANDLER_MISMATCH;
            } else if fields[13] == 0 || fields[13] > IRQ_TIMEOUT_US as u32 {
                decision = TIMER_MISMATCH;
            }
        }

        if decision == PASS && flags & REQUIRED_FLAGS == REQUIRED_FLAGS {
            flags |= 1 << 31;
        } else if decision == PASS {
            decision = TIMER_MISMATCH;
        }
        fields[0] = decision;
        fields[14] = flags;
        publish(fields);
        decision
    }

    fn publish(fields: [u32; 15]) {
        const MAGIC: u32 = 0x3151_3054; // T0Q1
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            for (index, value) in fields.into_iter().enumerate() {
                core::ptr::write_volatile(words.add(index + 1), value);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart-reset-irq-map-proof"))]
fn uart_reset_irq_map_hardware_proof(resets: &mut ResetController) -> [u32; 15] {
    const RESET_CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const RESET_CLEAR1: *mut u32 = 0x4001_7004 as *mut u32;
    const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
    const UART_BASES: [usize; 6] = [
        0x4003_0000,
        0x4003_4000,
        0x4003_8000,
        0x4003_c000,
        0x4004_0000,
        0x4004_4000,
    ];
    const UART_IRQS: [u32; 6] = [25, 42, 43, 44, 45, 46];
    const UART_RESETS: [UartReset; 6] = [
        UartReset::Uart0,
        UartReset::Uart1,
        UartReset::Uart2,
        UartReset::Uart3,
        UartReset::Uart4,
        UartReset::Uart5,
    ];
    const UART_RESET_MASK: u32 = 0xfc00_0000;
    const UART_DR: usize = 0x00;
    const UART_RSR_ECR: usize = 0x04;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2c;
    const UART_CR: usize = 0x30;
    const UART_IMSC: usize = 0x38;
    const UART_RIS: usize = 0x3c;
    const UART_MIS: usize = 0x40;
    const UART_ICR: usize = 0x44;
    const UART_PERIPH_ID0: usize = 0xfe0;
    const UART_CR_LOOPBACK: u32 = 1 | (1 << 7) | (1 << 8) | (1 << 9);
    const UART_INT_RX: u32 = 1 << 4;
    const UART_IRQ_CANDIDATES0: u32 = 1 << 25;
    const UART_IRQ_CANDIDATES1: u32 = 0x1f << 10;
    const NVIC_ICER0: *mut u32 = 0xe000_e180 as *mut u32;
    const NVIC_ICER1: *mut u32 = 0xe000_e184 as *mut u32;
    const NVIC_ISPR0: *const u32 = 0xe000_e200 as *const u32;
    const NVIC_ISPR1: *const u32 = 0xe000_e204 as *const u32;
    const NVIC_ICPR0: *mut u32 = 0xe000_e280 as *mut u32;
    const NVIC_ICPR1: *mut u32 = 0xe000_e284 as *mut u32;
    const POLL_LIMIT: usize = 100_000;
    const SENTINEL_IBRD: u32 = 0x155;
    const REQUIRED_UART_STATUS: u32 = 0x7fff;

    let mut fields = [0u32; 15];
    unsafe {
        fields[13] = core::ptr::read_volatile(RESET_CTRL1);
        fields[14] = core::ptr::read_volatile(RESET_DONE1);
        core::ptr::write_volatile(NVIC_ICER0, u32::MAX);
        core::ptr::write_volatile(NVIC_ICER1, u32::MAX);
        core::ptr::write_volatile(NVIC_ICPR0, u32::MAX);
        core::ptr::write_volatile(NVIC_ICPR1, u32::MAX);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));

        for index in 0..UART_BASES.len() {
            let base = UART_BASES[index];
            let reset_mask = 1u32 << (26 + index);
            let irq = UART_IRQS[index];
            let expected_pending0 = if irq < 32 { 1u32 << irq } else { 0 };
            let expected_pending1 = if irq >= 32 { 1u32 << (irq - 32) } else { 0 };
            let mut status = 0u32;

            if core::ptr::read_volatile(RESET_CTRL1) & reset_mask != 0 {
                core::ptr::write_volatile(RESET_CLEAR1, reset_mask);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            for _ in 0..POLL_LIMIT {
                if core::ptr::read_volatile(RESET_CTRL1) & reset_mask == 0
                    && core::ptr::read_volatile(RESET_DONE1) & reset_mask != 0
                {
                    status |= 1;
                    break;
                }
            }

            core::ptr::write_volatile((base + UART_CR) as *mut u32, 0);
            core::ptr::write_volatile((base + UART_IBRD) as *mut u32, SENTINEL_IBRD);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            if core::ptr::read_volatile((base + UART_IBRD) as *const u32) == SENTINEL_IBRD {
                status |= 1 << 1;
            }

            if let Ok(state) = resets.assert_uart_clock_ready(UART_RESETS[index], POLL_LIMIT) {
                if state.asserted {
                    status |= 1 << 2;
                }
                if !state.done {
                    status |= 1 << 3;
                }
            }

            if let Ok(state) = resets.deassert_uart_clock_ready(UART_RESETS[index], POLL_LIMIT) {
                if !state.asserted {
                    status |= 1 << 4;
                }
                if state.done {
                    status |= 1 << 5;
                }
            }
            if core::ptr::read_volatile((base + UART_IBRD) as *const u32) == 0 {
                status |= 1 << 6;
            }

            core::ptr::write_volatile((base + UART_CR) as *mut u32, 0);
            core::ptr::write_volatile((base + UART_IMSC) as *mut u32, 0);
            core::ptr::write_volatile((base + UART_ICR) as *mut u32, 0x7ff);
            core::ptr::write_volatile((base + UART_RSR_ECR) as *mut u32, 0);
            core::ptr::write_volatile((base + UART_IBRD) as *mut u32, 27);
            core::ptr::write_volatile((base + UART_FBRD) as *mut u32, 8);
            core::ptr::write_volatile((base + UART_LCRH) as *mut u32, 3 << 5);
            core::ptr::write_volatile((base + UART_CR) as *mut u32, UART_CR_LOOPBACK);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            if core::ptr::read_volatile((base + UART_CR) as *const u32) & UART_CR_LOOPBACK
                == UART_CR_LOOPBACK
            {
                status |= 1 << 7;
            }
            if core::ptr::read_volatile((base + UART_PERIPH_ID0) as *const u32) & 0xff == 0x11 {
                status |= 1 << 14;
            }

            core::ptr::write_volatile(NVIC_ICPR0, u32::MAX);
            core::ptr::write_volatile(NVIC_ICPR1, u32::MAX);
            core::ptr::write_volatile((base + UART_IMSC) as *mut u32, UART_INT_RX);
            let test_byte = 0xa0 | index as u32;
            core::ptr::write_volatile((base + UART_DR) as *mut u32, test_byte);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));

            let mut ris = 0u32;
            let mut mis = 0u32;
            let mut pending0 = 0u32;
            let mut pending1 = 0u32;
            for _ in 0..POLL_LIMIT {
                ris = core::ptr::read_volatile((base + UART_RIS) as *const u32);
                mis = core::ptr::read_volatile((base + UART_MIS) as *const u32);
                pending0 = core::ptr::read_volatile(NVIC_ISPR0);
                pending1 = core::ptr::read_volatile(NVIC_ISPR1);
                if ris & UART_INT_RX != 0
                    && pending0 & expected_pending0 == expected_pending0
                    && pending1 & expected_pending1 == expected_pending1
                {
                    break;
                }
            }
            if ris & UART_INT_RX != 0 {
                status |= 1 << 8;
            }
            if mis & UART_INT_RX != 0 {
                status |= 1 << 9;
            }
            if pending0 & expected_pending0 == expected_pending0
                && pending1 & expected_pending1 == expected_pending1
            {
                status |= 1 << 10;
            }
            let observed_uart_mask = ((pending0 & UART_IRQ_CANDIDATES0) >> 25)
                | (((pending1 & UART_IRQ_CANDIDATES1) >> 10) << 1);
            if observed_uart_mask == 1 << index {
                status |= 1 << 11;
            }

            let received =
                if core::ptr::read_volatile((base + UART_FR) as *const u32) & (1 << 4) == 0 {
                    core::ptr::read_volatile((base + UART_DR) as *const u32) & 0xff
                } else {
                    0x100
                };
            if received == test_byte {
                status |= 1 << 12;
            }

            core::ptr::write_volatile((base + UART_IMSC) as *mut u32, 0);
            core::ptr::write_volatile((base + UART_ICR) as *mut u32, 0x7ff);
            core::ptr::write_volatile((base + UART_CR) as *mut u32, 0);
            core::ptr::write_volatile(NVIC_ICPR0, expected_pending0);
            core::ptr::write_volatile(NVIC_ICPR1, expected_pending1);
            core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
            if core::ptr::read_volatile(NVIC_ISPR0) & expected_pending0 == 0
                && core::ptr::read_volatile(NVIC_ISPR1) & expected_pending1 == 0
            {
                status |= 1 << 13;
            }

            fields[1 + index * 2] = status;
            fields[2 + index * 2] = observed_uart_mask
                | ((received & 0x1ff) << 8)
                | ((ris & 0x1f) << 20)
                | ((mis & 0x1f) << 25);
            if status & REQUIRED_UART_STATUS == REQUIRED_UART_STATUS {
                fields[0] |= 1 << index;
            }
        }

        if fields[0] & 0x3f == 0x3f {
            fields[0] |= 1 << 8;
        }
        let final_ctrl = core::ptr::read_volatile(RESET_CTRL1);
        let final_done = core::ptr::read_volatile(RESET_DONE1);
        if final_ctrl & UART_RESET_MASK == 0 && final_done & UART_RESET_MASK == UART_RESET_MASK {
            fields[0] |= 1 << 9;
        }
    }
    fields
}

#[cfg(all(target_arch = "arm", feature = "uart-reset-irq-map-proof"))]
fn publish_uart_reset_irq_map_hardware_proof(fields: [u32; 15]) {
    const MAGIC: u32 = 0x314d_5255; // URM1
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart1-local-nvic42-delivery"))]
mod uart1_local_nvic42_delivery {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rp1_hal::reset::{ResetController, UartReset};

    const MAGIC: u32 = u32::from_le_bytes(*b"U1I1");
    const PASS: u32 = 1;
    const FAIL_PREFLIGHT: u32 = 2;
    const FAIL_RESET: u32 = 3;
    const FAIL_SETUP: u32 = 4;
    const FAIL_SOURCE: u32 = 5;
    const FAIL_TIMEOUT: u32 = 6;
    const FAIL_IPSR: u32 = 7;
    const FAIL_STORM: u32 = 8;
    const FAIL_RESET_DONE: u32 = 9;
    const FAIL_IRQ42_RESTORE: u32 = 10;
    const FAIL_IRQ53_CHANGED: u32 = 11;

    const UART1_BASE: usize = 0x4003_4000;
    const UART_DR: usize = 0x00;
    const UART_RSR_ECR: usize = 0x04;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2c;
    const UART_CR: usize = 0x30;
    const UART_IMSC: usize = 0x38;
    const UART_RIS: usize = 0x3c;
    const UART_MIS: usize = 0x40;
    const UART_ICR: usize = 0x44;
    const UART_PERIPH_ID0: usize = 0xfe0;
    const UART_CR_LOOPBACK: u32 = 1 | (1 << 7) | (1 << 8) | (1 << 9);
    const UART_INT_RX: u32 = 1 << 4;
    const UART_RXFE: u32 = 1 << 4;
    const TEST_BYTE: u32 = 0xa1;
    const POLL_LIMIT: usize = 100_000;
    const STABILITY_US: u64 = 4_000;
    const IRQ42_BIT1: u32 = 1 << 10;
    const IRQ53_BIT1: u32 = 1 << 21;
    const RESET_CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
    const RESET_UART1: u32 = 1 << 27;
    const CLK_UART_CTRL: *mut u32 = 0x4001_8054 as *mut u32;
    const CLK_UART_DIV_INT: *const u32 = 0x4001_8058 as *const u32;
    const CLK_UART_SEL: *const u32 = 0x4001_8060 as *const u32;
    const PLL_SYS_CS: *const u32 = 0x4002_0000 as *const u32;
    const PLL_SYS_PRIM: *const u32 = 0x4002_0010 as *const u32;
    const CLK_UART_CTRL_RELEVANT: u32 = 0x0000_0fe0;
    const CLK_UART_SOURCE: u32 = 0x0000_0040;
    const CLK_UART_ENABLED: u32 = 0x0000_0840;
    const PLL_SYS_CS_EXPECTED: u32 = 0x8000_0001;
    const PLL_SYS_PRIM_PRI_PH: u32 = 1 << 4;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_RIS: AtomicU32 = AtomicU32::new(0);
    static FIRST_MIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_RIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_MIS: AtomicU32 = AtomicU32::new(0);
    static HANDLER_ROUTE: AtomicU32 = AtomicU32::new(0);
    static BYTE_RSR: AtomicU32 = AtomicU32::new(0);
    static STORM: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn UART1_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Release);
        if old > 3 {
            STORM.store(1, Ordering::Relaxed);
        }
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_RIS.store(read(UART_RIS), Ordering::Relaxed);
            FIRST_MIS.store(read(UART_MIS), Ordering::Relaxed);
            HANDLER_ROUTE.store(
                pack_route(rp1_rt::uart1_irq_route_snapshot()),
                Ordering::Relaxed,
            );
            let byte = if read(UART_FR) & UART_RXFE == 0 {
                read(UART_DR) & 0xff
            } else {
                0x100
            };
            BYTE_RSR.store(
                byte | ((read(UART_RSR_ECR) & 0xff) << 16),
                Ordering::Relaxed,
            );
        }
        write(UART_IMSC, 0);
        write(UART_ICR, UART_INT_RX);
        FINAL_RIS.store(read(UART_RIS), Ordering::Relaxed);
        FINAL_MIS.store(read(UART_MIS), Ordering::Relaxed);
    }

    pub fn run_and_publish(resets: &mut ResetController) -> u32 {
        reset_atomics();
        let mut fields = [0u32; 15];
        let mut saved = None;
        let mut decision = PASS;
        let mut flags = 0u32;
        let mut armed_imsc = 0u32;
        let mut enabled_iser1 = 0u32;
        let mut source_ready = 0u32;
        let mut source_ispr1 = 0u32;
        let mut reset_attempted = false;
        let mut uart_touched = false;
        let clock_initial = read_clock_state();
        let mut clock_evidence = ClockEvidence::from_initial(clock_initial);
        let reset_ctrl_before = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_before = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        let reset_initial_asserted = reset_ctrl_before & RESET_UART1 != 0;
        let reset_initial_done = reset_done_before & RESET_UART1 != 0;
        let route_before = rp1_rt::uart1_irq_route_snapshot();
        let mut identity_ok = false;

        if !preflight_ok(route_before, reset_initial_asserted, reset_initial_done)
            || !prepare_clock(&mut clock_evidence)
        {
            decision = FAIL_PREFLIGHT;
        } else {
            reset_attempted = true;
            if !exercise_reset(resets, reset_initial_asserted) {
                decision = FAIL_RESET;
            } else {
                identity_ok = read(UART_PERIPH_ID0) & 0xff == 0x11;
                if !identity_ok {
                    decision = FAIL_PREFLIGHT;
                } else {
                    uart_touched = true;
                    setup_uart1();
                    if read(UART_CR) & UART_CR_LOOPBACK != UART_CR_LOOPBACK {
                        decision = FAIL_SETUP;
                    }
                }
            }
        }

        if decision == PASS {
            saved = unsafe { rp1_rt::prepare_uart1_irq() };
            if saved.is_none() {
                decision = FAIL_PREFLIGHT;
            }
        }
        if decision == PASS {
            write(UART_IMSC, UART_INT_RX);
            armed_imsc = read(UART_IMSC);
            write(UART_DR, TEST_BYTE);
            unsafe {
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            let mut source_ris = 0;
            let mut source_mis = 0;
            let mut source_route = rp1_rt::uart1_irq_route_snapshot();
            for poll in 0..POLL_LIMIT {
                source_ris = read(UART_RIS);
                source_mis = read(UART_MIS);
                source_route = rp1_rt::uart1_irq_route_snapshot();
                if source_ris & UART_INT_RX != 0
                    && source_mis & UART_INT_RX != 0
                    && source_route.ispr1 & IRQ42_BIT1 != 0
                {
                    fields[8] = poll as u32;
                    break;
                }
            }
            source_ready = pack_source_ready(source_ris, source_mis, source_route);
            source_ispr1 = source_route.ispr1;
            if source_ris & UART_INT_RX == 0
                || source_mis & UART_INT_RX == 0
                || source_route.ispr1 & IRQ42_BIT1 == 0
                || source_route.iser1 & IRQ42_BIT1 != 0
            {
                decision = FAIL_SOURCE;
            } else {
                flags |= 1 << 1;
                unsafe {
                    rp1_rt::enable_uart1_irq_after_source_asserted();
                }
                enabled_iser1 = rp1_rt::uart1_irq_route_snapshot().iser1;
            }
        }

        if decision == PASS {
            for _ in 0..POLL_LIMIT {
                if COUNT.load(Ordering::Acquire) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            if COUNT.load(Ordering::Acquire) == 0 {
                decision = FAIL_TIMEOUT;
            } else {
                super::busy_wait_us(STABILITY_US);
            }
        }

        cleanup_uart_irq(saved, uart_touched);
        let (final_ris, final_mis, final_imsc) = if uart_touched {
            (read(UART_RIS), read(UART_MIS), read(UART_IMSC))
        } else {
            (0, 0, 0)
        };
        restore_initial_reset(resets, reset_attempted, reset_initial_asserted);
        restore_clock(clock_initial, &mut clock_evidence);
        let route_final = rp1_rt::uart1_irq_route_snapshot();
        let reset_ctrl_final = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_final = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        fields[2] = pack_event();
        fields[3] = (FIRST_RIS.load(Ordering::Relaxed) & 0xffff)
            | (FIRST_MIS.load(Ordering::Relaxed) << 16);
        fields[4] = (final_ris & 0xffff) | (final_mis << 16);
        fields[5] = (armed_imsc & 0xffff) | (final_imsc << 16);
        fields[6] = route_before.iser0;
        fields[7] = route_before.iser1;
        fields[8] = enabled_iser1;
        fields[9] = route_final.iser0;
        fields[10] = route_final.iser1;
        fields[11] = route_before.ispr1;
        fields[12] = source_ispr1;
        fields[13] = route_final.ispr1;
        fields[14] = pack_tail(
            reset_ctrl_before,
            reset_ctrl_final,
            reset_done_before,
            reset_done_final,
            source_ready,
            route_before,
            HANDLER_ROUTE.load(Ordering::Relaxed),
            route_final,
            clock_evidence,
        );

        let irq42_restored = route_final.iser1 == route_before.iser1
            && route_final.ispr1 & IRQ42_BIT1 == 0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask;
        let irq53_unchanged = route_final.iser1 & IRQ53_BIT1 == route_before.iser1 & IRQ53_BIT1
            && route_final.ispr1 & IRQ53_BIT1 == route_before.ispr1 & IRQ53_BIT1
            && route_final.iabr1 & IRQ53_BIT1 == route_before.iabr1 & IRQ53_BIT1
            && route_final.ispr1 & !IRQ42_BIT1 == route_before.ispr1 & !IRQ42_BIT1;
        let reset_restored = reset_state_matches(
            reset_ctrl_final & RESET_UART1 != 0,
            reset_done_final & RESET_UART1 != 0,
            reset_initial_asserted,
        );
        let no_storm = STORM.load(Ordering::Relaxed) == 0 && COUNT.load(Ordering::Acquire) == 1;
        let source_clean = final_imsc == 0 && final_mis == 0 && final_ris & UART_INT_RX == 0;
        let ipsr_ok = IPSR.load(Ordering::Relaxed) == rp1_rt::UART1_VECTOR_INDEX as u32;
        let byte_rsr = BYTE_RSR.load(Ordering::Relaxed);
        let byte_rsr_ok = byte_rsr & 0xff == TEST_BYTE && byte_rsr & 0x00ff_0000 == 0;
        let source_before_enable = source_ready & 0x0f == 0x07;
        let first_source_ok = FIRST_RIS.load(Ordering::Relaxed) & UART_INT_RX != 0
            && FIRST_MIS.load(Ordering::Relaxed) & UART_INT_RX != 0;
        let handler_active_ok = HANDLER_ROUTE.load(Ordering::Relaxed) & (1 << 2) != 0;
        let enable_ok = enabled_iser1 == IRQ42_BIT1;
        let final_exact = route_final.iser0 == route_before.iser0
            && route_final.iser1 == route_before.iser1
            && route_final.ispr0 == route_before.ispr0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr0 == route_before.iabr0
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask
            && clock_evidence.final_ctrl_exact;

        flags |= (route_before.vtor == 0x2000_0000) as u32;
        flags |= (source_before_enable as u32) << 1;
        flags |= (ipsr_ok as u32) << 2;
        flags |= (no_storm as u32) << 3;
        flags |= (reset_restored as u32) << 4;
        flags |= (irq42_restored as u32) << 5;
        flags |= (irq53_unchanged as u32) << 6;
        flags |= (identity_ok as u32) << 7;
        flags |= (source_clean as u32) << 8;
        flags |= (byte_rsr_ok as u32) << 9;
        flags |= (enable_ok as u32) << 10;
        flags |= (first_source_ok as u32) << 11;
        flags |= (handler_active_ok as u32) << 12;
        flags |= (final_exact as u32) << 13;
        if flags & 0x3fff == 0x3fff && decision == PASS {
            flags |= 1 << 31;
        } else if decision == PASS && !ipsr_ok {
            decision = FAIL_IPSR;
        } else if decision == PASS && (!no_storm || !handler_active_ok) {
            decision = FAIL_STORM;
        } else if decision == PASS && !reset_restored {
            decision = FAIL_RESET_DONE;
        } else if decision == PASS && (!irq42_restored || !final_exact) {
            decision = FAIL_IRQ42_RESTORE;
        } else if decision == PASS && !irq53_unchanged {
            decision = FAIL_IRQ53_CHANGED;
        } else if decision == PASS && (!source_clean || !enable_ok || !first_source_ok) {
            decision = FAIL_SETUP;
        } else if decision == PASS && !byte_rsr_ok {
            decision = FAIL_SETUP;
        }

        fields[0] = decision;
        fields[1] = flags;
        publish(fields);
        decision
    }

    #[derive(Copy, Clone)]
    struct ClockState {
        ctrl: u32,
        div_int: u32,
        sel: u32,
        pll_cs: u32,
        pll_prim: u32,
    }

    #[derive(Copy, Clone)]
    struct ClockEvidence {
        initial_active: bool,
        initial_inactive_exact: bool,
        promoted: bool,
        source_rb_ok: bool,
        sel1: bool,
        enabled_rb_ok: bool,
        final_ctrl_exact: bool,
        div1: bool,
        pll_exact_lock: bool,
        pri_ph_bit4: bool,
    }

    impl ClockEvidence {
        const fn from_initial(state: ClockState) -> Self {
            let initial_active = state.ctrl & CLK_UART_CTRL_RELEVANT == CLK_UART_ENABLED;
            let initial_inactive_exact = state.ctrl == 0;
            let div1 = state.div_int == 1;
            let pll_exact_lock = state.pll_cs == PLL_SYS_CS_EXPECTED;
            let pri_ph_bit4 = state.pll_prim & PLL_SYS_PRIM_PRI_PH != 0;
            Self {
                initial_active,
                initial_inactive_exact,
                promoted: false,
                source_rb_ok: initial_active,
                sel1: state.sel == 1,
                enabled_rb_ok: initial_active,
                final_ctrl_exact: false,
                div1,
                pll_exact_lock,
                pri_ph_bit4,
            }
        }
    }

    fn read_clock_state() -> ClockState {
        unsafe {
            ClockState {
                ctrl: core::ptr::read_volatile(CLK_UART_CTRL),
                div_int: core::ptr::read_volatile(CLK_UART_DIV_INT),
                sel: core::ptr::read_volatile(CLK_UART_SEL),
                pll_cs: core::ptr::read_volatile(PLL_SYS_CS),
                pll_prim: core::ptr::read_volatile(PLL_SYS_PRIM),
            }
        }
    }

    fn prepare_clock(evidence: &mut ClockEvidence) -> bool {
        if !evidence.div1 || !evidence.pll_exact_lock || !evidence.pri_ph_bit4 {
            return false;
        }
        if evidence.initial_active {
            return evidence.sel1;
        }
        if !evidence.initial_inactive_exact {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_SOURCE);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.promoted = true;
        evidence.source_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_SOURCE;
        evidence.sel1 = unsafe { core::ptr::read_volatile(CLK_UART_SEL) } == 1;
        if !evidence.source_rb_ok || !evidence.sel1 {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_ENABLED);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.enabled_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_ENABLED;
        evidence.enabled_rb_ok
    }

    fn restore_clock(initial: ClockState, evidence: &mut ClockEvidence) {
        if evidence.promoted {
            unsafe {
                core::ptr::write_volatile(CLK_UART_CTRL, initial.ctrl);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
        }
        evidence.final_ctrl_exact =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } == initial.ctrl;
    }

    fn setup_uart1() {
        write(UART_CR, 0);
        write(UART_IMSC, 0);
        write(UART_ICR, 0x7ff);
        write(UART_RSR_ECR, 0);
        write(UART_IBRD, 27);
        write(UART_FBRD, 8);
        write(UART_LCRH, 3 << 5);
        write(UART_CR, UART_CR_LOOPBACK);
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn cleanup_uart_irq(saved: Option<rp1_rt::Uart1IrqSaved>, uart_touched: bool) {
        if uart_touched {
            write(UART_IMSC, 0);
            write(UART_ICR, UART_INT_RX);
            write(UART_CR, 0);
        }
        if let Some(saved) = saved {
            unsafe {
                rp1_rt::restore_uart1_irq(saved);
            }
        }
    }

    fn preflight_ok(
        route: rp1_rt::Uart1IrqRouteSnapshot,
        reset_asserted: bool,
        reset_done: bool,
    ) -> bool {
        let reset_coherent = reset_state_matches(reset_asserted, reset_done, reset_asserted);
        route.vtor == 0x2000_0000
            && route.primask == 0
            && route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & IRQ42_BIT1 == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
            && reset_coherent
    }

    const fn reset_state_matches(asserted: bool, done: bool, want_asserted: bool) -> bool {
        if want_asserted {
            asserted && !done
        } else {
            !asserted && done
        }
    }

    fn exercise_reset(resets: &mut ResetController, initially_asserted: bool) -> bool {
        if initially_asserted
            && resets
                .deassert_uart_clock_ready(UartReset::Uart1, POLL_LIMIT)
                .is_err()
        {
            return false;
        }
        resets
            .assert_uart_clock_ready(UartReset::Uart1, POLL_LIMIT)
            .is_ok()
            && resets
                .deassert_uart_clock_ready(UartReset::Uart1, POLL_LIMIT)
                .is_ok()
    }

    fn restore_initial_reset(
        resets: &mut ResetController,
        reset_attempted: bool,
        initially_asserted: bool,
    ) {
        if !reset_attempted {
            return;
        }
        let asserted = unsafe { core::ptr::read_volatile(RESET_CTRL1) } & RESET_UART1 != 0;
        let done = unsafe { core::ptr::read_volatile(RESET_DONE1) } & RESET_UART1 != 0;
        if reset_state_matches(asserted, done, initially_asserted) {
            return;
        }
        if initially_asserted && !asserted && done {
            let _ = resets.assert_uart_clock_ready(UartReset::Uart1, POLL_LIMIT);
        } else if !initially_asserted && asserted && !done {
            let _ = resets.deassert_uart_clock_ready(UartReset::Uart1, POLL_LIMIT);
        }
    }

    fn pack_route(route: rp1_rt::Uart1IrqRouteSnapshot) -> u32 {
        ((route.iser1 & IRQ42_BIT1 != 0) as u32)
            | (((route.ispr1 & IRQ42_BIT1 != 0) as u32) << 1)
            | (((route.iabr1 & IRQ42_BIT1 != 0) as u32) << 2)
            | ((route.primask & 1) << 3)
            | (((route.vtor == 0x2000_0000) as u32) << 4)
            | (((route.iser1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 10)
            | (((route.iser0 != 0) as u32) << 16)
            | (((route.ispr0 != 0) as u32) << 17)
            | (((route.iabr0 != 0) as u32) << 18)
    }

    fn pack_event() -> u32 {
        (IPSR.load(Ordering::Relaxed) & 0xff)
            | ((COUNT.load(Ordering::Acquire) & 0xff) << 8)
            | ((BYTE_RSR.load(Ordering::Relaxed) & 0xff) << 16)
            | (((BYTE_RSR.load(Ordering::Relaxed) >> 16) & 0xff) << 24)
    }

    fn pack_source_ready(ris: u32, mis: u32, route: rp1_rt::Uart1IrqRouteSnapshot) -> u32 {
        ((ris & UART_INT_RX != 0) as u32)
            | (((mis & UART_INT_RX != 0) as u32) << 1)
            | (((route.ispr1 & IRQ42_BIT1 != 0) as u32) << 2)
            | (((route.iser1 & IRQ42_BIT1 != 0) as u32) << 3)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | ((ris & 0xff) << 16)
            | ((mis & 0xff) << 24)
    }

    fn pack_tail(
        reset_ctrl_before: u32,
        reset_ctrl_final: u32,
        reset_done_before: u32,
        reset_done_final: u32,
        source_ready: u32,
        route_before: rp1_rt::Uart1IrqRouteSnapshot,
        handler_route: u32,
        route_final: rp1_rt::Uart1IrqRouteSnapshot,
        clock: ClockEvidence,
    ) -> u32 {
        ((reset_ctrl_before & RESET_UART1 != 0) as u32)
            | (((reset_ctrl_final & RESET_UART1 != 0) as u32) << 1)
            | (((reset_done_before & RESET_UART1 != 0) as u32) << 2)
            | (((reset_done_final & RESET_UART1 != 0) as u32) << 3)
            | ((route_before.primask & 1) << 4)
            | ((route_final.primask & 1) << 5)
            | (((route_before.iabr1 & IRQ42_BIT1 != 0) as u32) << 8)
            | (((handler_route & (1 << 2) != 0) as u32) << 9)
            | (((route_final.iabr1 & IRQ42_BIT1 != 0) as u32) << 10)
            | ((source_ready & 0x03) << 16)
            | ((clock.initial_active as u32) << 20)
            | ((clock.initial_inactive_exact as u32) << 21)
            | ((clock.promoted as u32) << 22)
            | ((clock.source_rb_ok as u32) << 23)
            | ((clock.sel1 as u32) << 24)
            | ((clock.enabled_rb_ok as u32) << 25)
            | ((clock.final_ctrl_exact as u32) << 26)
            | ((clock.div1 as u32) << 27)
            | ((clock.pll_exact_lock as u32) << 28)
            | ((clock.pri_ph_bit4 as u32) << 29)
    }

    fn reset_atomics() {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_RIS.store(0, Ordering::Relaxed);
        FIRST_MIS.store(0, Ordering::Relaxed);
        FINAL_RIS.store(0, Ordering::Relaxed);
        FINAL_MIS.store(0, Ordering::Relaxed);
        HANDLER_ROUTE.store(0, Ordering::Relaxed);
        BYTE_RSR.store(0, Ordering::Relaxed);
        STORM.store(0, Ordering::Relaxed);
    }

    fn publish(fields: [u32; 15]) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            for (index, value) in fields.into_iter().enumerate() {
                core::ptr::write_volatile(words.add(index + 1), value);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn read(offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((UART1_BASE + offset) as *const u32) }
    }

    fn write(offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((UART1_BASE + offset) as *mut u32, value);
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-nvic43-delivery"))]
mod uart2_local_nvic43_delivery {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rp1_hal::reset::{ResetController, UartReset};

    const MAGIC: u32 = u32::from_le_bytes(*b"U2I1");
    const PASS: u32 = 1;
    const FAIL_PREFLIGHT: u32 = 2;
    const FAIL_RESET: u32 = 3;
    const FAIL_SETUP: u32 = 4;
    const FAIL_SOURCE: u32 = 5;
    const FAIL_TIMEOUT: u32 = 6;
    const FAIL_IPSR: u32 = 7;
    const FAIL_STORM: u32 = 8;
    const FAIL_RESET_DONE: u32 = 9;
    const FAIL_IRQ43_RESTORE: u32 = 10;
    const FAIL_IRQ53_CHANGED: u32 = 11;

    const UART2_BASE: usize = 0x4003_8000;
    const UART_DR: usize = 0x00;
    const UART_RSR_ECR: usize = 0x04;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2c;
    const UART_CR: usize = 0x30;
    const UART_IMSC: usize = 0x38;
    const UART_RIS: usize = 0x3c;
    const UART_MIS: usize = 0x40;
    const UART_ICR: usize = 0x44;
    const UART_PERIPH_ID0: usize = 0xfe0;
    const UART_CR_LOOPBACK: u32 = 1 | (1 << 7) | (1 << 8) | (1 << 9);
    const UART_INT_RX: u32 = 1 << 4;
    const UART_RXFE: u32 = 1 << 4;
    const TEST_BYTE: u32 = 0xa2;
    const POLL_LIMIT: usize = 100_000;
    const STABILITY_US: u64 = 4_000;
    const IRQ43_BIT1: u32 = 1 << 11;
    const IRQ53_BIT1: u32 = 1 << 21;
    const RESET_CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
    const RESET_UART2: u32 = 1 << 28;
    const CLK_UART_CTRL: *mut u32 = 0x4001_8054 as *mut u32;
    const CLK_UART_DIV_INT: *const u32 = 0x4001_8058 as *const u32;
    const CLK_UART_SEL: *const u32 = 0x4001_8060 as *const u32;
    const PLL_SYS_CS: *const u32 = 0x4002_0000 as *const u32;
    const PLL_SYS_PRIM: *const u32 = 0x4002_0010 as *const u32;
    const CLK_UART_CTRL_RELEVANT: u32 = 0x0000_0fe0;
    const CLK_UART_SOURCE: u32 = 0x0000_0040;
    const CLK_UART_ENABLED: u32 = 0x0000_0840;
    const PLL_SYS_CS_EXPECTED: u32 = 0x8000_0001;
    const PLL_SYS_PRIM_PRI_PH: u32 = 1 << 4;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_RIS: AtomicU32 = AtomicU32::new(0);
    static FIRST_MIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_RIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_MIS: AtomicU32 = AtomicU32::new(0);
    static HANDLER_ROUTE: AtomicU32 = AtomicU32::new(0);
    static BYTE_RSR: AtomicU32 = AtomicU32::new(0);
    static STORM: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn UART2_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Release);
        if old > 3 {
            STORM.store(1, Ordering::Relaxed);
        }
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_RIS.store(read(UART_RIS), Ordering::Relaxed);
            FIRST_MIS.store(read(UART_MIS), Ordering::Relaxed);
            HANDLER_ROUTE.store(
                pack_route(rp1_rt::uart2_irq_route_snapshot()),
                Ordering::Relaxed,
            );
            let byte = if read(UART_FR) & UART_RXFE == 0 {
                read(UART_DR) & 0xff
            } else {
                0x100
            };
            BYTE_RSR.store(
                byte | ((read(UART_RSR_ECR) & 0xff) << 16),
                Ordering::Relaxed,
            );
        }
        write(UART_IMSC, 0);
        write(UART_ICR, UART_INT_RX);
        FINAL_RIS.store(read(UART_RIS), Ordering::Relaxed);
        FINAL_MIS.store(read(UART_MIS), Ordering::Relaxed);
    }

    pub fn run_and_publish(resets: &mut ResetController) -> u32 {
        reset_atomics();
        let mut fields = [0u32; 15];
        let mut saved = None;
        let mut decision = PASS;
        let mut flags = 0u32;
        let mut armed_imsc = 0u32;
        let mut enabled_iser1 = 0u32;
        let mut source_ready = 0u32;
        let mut source_ispr1 = 0u32;
        let mut reset_attempted = false;
        let mut uart_touched = false;
        let clock_initial = read_clock_state();
        let mut clock_evidence = ClockEvidence::from_initial(clock_initial);
        let reset_ctrl_before = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_before = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        let reset_initial_asserted = reset_ctrl_before & RESET_UART2 != 0;
        let reset_initial_done = reset_done_before & RESET_UART2 != 0;
        let route_before = rp1_rt::uart2_irq_route_snapshot();
        let mut identity_ok = false;

        if !preflight_ok(route_before, reset_initial_asserted, reset_initial_done)
            || !prepare_clock(&mut clock_evidence)
        {
            decision = FAIL_PREFLIGHT;
        } else {
            reset_attempted = true;
            if !exercise_reset(resets, reset_initial_asserted) {
                decision = FAIL_RESET;
            } else {
                identity_ok = read(UART_PERIPH_ID0) & 0xff == 0x11;
                if !identity_ok {
                    decision = FAIL_PREFLIGHT;
                } else {
                    uart_touched = true;
                    setup_uart2();
                    if read(UART_CR) & UART_CR_LOOPBACK != UART_CR_LOOPBACK {
                        decision = FAIL_SETUP;
                    }
                }
            }
        }

        if decision == PASS {
            saved = unsafe { rp1_rt::prepare_uart2_irq() };
            if saved.is_none() {
                decision = FAIL_PREFLIGHT;
            }
        }
        if decision == PASS {
            write(UART_IMSC, UART_INT_RX);
            armed_imsc = read(UART_IMSC);
            write(UART_DR, TEST_BYTE);
            unsafe {
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            let mut source_ris = 0;
            let mut source_mis = 0;
            let mut source_route = rp1_rt::uart2_irq_route_snapshot();
            for poll in 0..POLL_LIMIT {
                source_ris = read(UART_RIS);
                source_mis = read(UART_MIS);
                source_route = rp1_rt::uart2_irq_route_snapshot();
                if source_ris & UART_INT_RX != 0
                    && source_mis & UART_INT_RX != 0
                    && source_route.ispr1 & IRQ43_BIT1 != 0
                {
                    fields[8] = poll as u32;
                    break;
                }
            }
            source_ready = pack_source_ready(source_ris, source_mis, source_route);
            source_ispr1 = source_route.ispr1;
            if source_ris & UART_INT_RX == 0
                || source_mis & UART_INT_RX == 0
                || source_route.ispr1 & IRQ43_BIT1 == 0
                || source_route.iser1 & IRQ43_BIT1 != 0
            {
                decision = FAIL_SOURCE;
            } else {
                flags |= 1 << 1;
                unsafe {
                    rp1_rt::enable_uart2_irq_after_source_asserted();
                }
                enabled_iser1 = rp1_rt::uart2_irq_route_snapshot().iser1;
            }
        }

        if decision == PASS {
            for _ in 0..POLL_LIMIT {
                if COUNT.load(Ordering::Acquire) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            if COUNT.load(Ordering::Acquire) == 0 {
                decision = FAIL_TIMEOUT;
            } else {
                super::busy_wait_us(STABILITY_US);
            }
        }

        cleanup_uart_irq(saved, uart_touched);
        let (final_ris, final_mis, final_imsc) = if uart_touched {
            (read(UART_RIS), read(UART_MIS), read(UART_IMSC))
        } else {
            (0, 0, 0)
        };
        restore_initial_reset(resets, reset_attempted, reset_initial_asserted);
        restore_clock(clock_initial, &mut clock_evidence);
        let route_final = rp1_rt::uart2_irq_route_snapshot();
        let reset_ctrl_final = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_final = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        fields[2] = pack_event();
        fields[3] = (FIRST_RIS.load(Ordering::Relaxed) & 0xffff)
            | (FIRST_MIS.load(Ordering::Relaxed) << 16);
        fields[4] = (final_ris & 0xffff) | (final_mis << 16);
        fields[5] = (armed_imsc & 0xffff) | (final_imsc << 16);
        fields[6] = route_before.iser0;
        fields[7] = route_before.iser1;
        fields[8] = enabled_iser1;
        fields[9] = route_final.iser0;
        fields[10] = route_final.iser1;
        fields[11] = route_before.ispr1;
        fields[12] = source_ispr1;
        fields[13] = route_final.ispr1;
        fields[14] = pack_tail(
            reset_ctrl_before,
            reset_ctrl_final,
            reset_done_before,
            reset_done_final,
            source_ready,
            route_before,
            HANDLER_ROUTE.load(Ordering::Relaxed),
            route_final,
            clock_evidence,
        );

        let irq43_restored = route_final.iser1 == route_before.iser1
            && route_final.ispr1 & IRQ43_BIT1 == 0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask;
        let irq53_unchanged = route_final.iser1 & IRQ53_BIT1 == route_before.iser1 & IRQ53_BIT1
            && route_final.ispr1 & IRQ53_BIT1 == route_before.ispr1 & IRQ53_BIT1
            && route_final.iabr1 & IRQ53_BIT1 == route_before.iabr1 & IRQ53_BIT1
            && route_final.ispr1 & !IRQ43_BIT1 == route_before.ispr1 & !IRQ43_BIT1;
        let reset_restored = reset_state_matches(
            reset_ctrl_final & RESET_UART2 != 0,
            reset_done_final & RESET_UART2 != 0,
            reset_initial_asserted,
        );
        let no_storm = STORM.load(Ordering::Relaxed) == 0 && COUNT.load(Ordering::Acquire) == 1;
        let source_clean = final_imsc == 0 && final_mis == 0 && final_ris & UART_INT_RX == 0;
        let ipsr_ok = IPSR.load(Ordering::Relaxed) == rp1_rt::UART2_VECTOR_INDEX as u32;
        let byte_rsr = BYTE_RSR.load(Ordering::Relaxed);
        let byte_rsr_ok = byte_rsr & 0xff == TEST_BYTE && byte_rsr & 0x00ff_0000 == 0;
        let source_before_enable = source_ready & 0x0f == 0x07;
        let first_source_ok = FIRST_RIS.load(Ordering::Relaxed) & UART_INT_RX != 0
            && FIRST_MIS.load(Ordering::Relaxed) & UART_INT_RX != 0;
        let handler_active_ok = HANDLER_ROUTE.load(Ordering::Relaxed) & (1 << 2) != 0;
        let enable_ok = enabled_iser1 == IRQ43_BIT1;
        let final_exact = route_final.iser0 == route_before.iser0
            && route_final.iser1 == route_before.iser1
            && route_final.ispr0 == route_before.ispr0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr0 == route_before.iabr0
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask
            && clock_evidence.final_ctrl_exact;

        flags |= (route_before.vtor == 0x2000_0000) as u32;
        flags |= (source_before_enable as u32) << 1;
        flags |= (ipsr_ok as u32) << 2;
        flags |= (no_storm as u32) << 3;
        flags |= (reset_restored as u32) << 4;
        flags |= (irq43_restored as u32) << 5;
        flags |= (irq53_unchanged as u32) << 6;
        flags |= (identity_ok as u32) << 7;
        flags |= (source_clean as u32) << 8;
        flags |= (byte_rsr_ok as u32) << 9;
        flags |= (enable_ok as u32) << 10;
        flags |= (first_source_ok as u32) << 11;
        flags |= (handler_active_ok as u32) << 12;
        flags |= (final_exact as u32) << 13;
        if flags & 0x3fff == 0x3fff && decision == PASS {
            flags |= 1 << 31;
        } else if decision == PASS && !ipsr_ok {
            decision = FAIL_IPSR;
        } else if decision == PASS && (!no_storm || !handler_active_ok) {
            decision = FAIL_STORM;
        } else if decision == PASS && !reset_restored {
            decision = FAIL_RESET_DONE;
        } else if decision == PASS && (!irq43_restored || !final_exact) {
            decision = FAIL_IRQ43_RESTORE;
        } else if decision == PASS && !irq53_unchanged {
            decision = FAIL_IRQ53_CHANGED;
        } else if decision == PASS && (!source_clean || !enable_ok || !first_source_ok) {
            decision = FAIL_SETUP;
        } else if decision == PASS && !byte_rsr_ok {
            decision = FAIL_SETUP;
        }

        fields[0] = decision;
        fields[1] = flags;
        publish(fields);
        decision
    }

    #[derive(Copy, Clone)]
    struct ClockState {
        ctrl: u32,
        div_int: u32,
        sel: u32,
        pll_cs: u32,
        pll_prim: u32,
    }

    #[derive(Copy, Clone)]
    struct ClockEvidence {
        initial_active: bool,
        initial_inactive_exact: bool,
        promoted: bool,
        source_rb_ok: bool,
        sel1: bool,
        enabled_rb_ok: bool,
        final_ctrl_exact: bool,
        div1: bool,
        pll_exact_lock: bool,
        pri_ph_bit4: bool,
    }

    impl ClockEvidence {
        const fn from_initial(state: ClockState) -> Self {
            let initial_active = state.ctrl & CLK_UART_CTRL_RELEVANT == CLK_UART_ENABLED;
            let initial_inactive_exact = state.ctrl == 0;
            let div1 = state.div_int == 1;
            let pll_exact_lock = state.pll_cs == PLL_SYS_CS_EXPECTED;
            let pri_ph_bit4 = state.pll_prim & PLL_SYS_PRIM_PRI_PH != 0;
            Self {
                initial_active,
                initial_inactive_exact,
                promoted: false,
                source_rb_ok: initial_active,
                sel1: state.sel == 1,
                enabled_rb_ok: initial_active,
                final_ctrl_exact: false,
                div1,
                pll_exact_lock,
                pri_ph_bit4,
            }
        }
    }

    fn read_clock_state() -> ClockState {
        unsafe {
            ClockState {
                ctrl: core::ptr::read_volatile(CLK_UART_CTRL),
                div_int: core::ptr::read_volatile(CLK_UART_DIV_INT),
                sel: core::ptr::read_volatile(CLK_UART_SEL),
                pll_cs: core::ptr::read_volatile(PLL_SYS_CS),
                pll_prim: core::ptr::read_volatile(PLL_SYS_PRIM),
            }
        }
    }

    fn prepare_clock(evidence: &mut ClockEvidence) -> bool {
        if !evidence.div1 || !evidence.pll_exact_lock || !evidence.pri_ph_bit4 {
            return false;
        }
        if evidence.initial_active {
            return evidence.sel1;
        }
        if !evidence.initial_inactive_exact {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_SOURCE);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.promoted = true;
        evidence.source_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_SOURCE;
        evidence.sel1 = unsafe { core::ptr::read_volatile(CLK_UART_SEL) } == 1;
        if !evidence.source_rb_ok || !evidence.sel1 {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_ENABLED);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.enabled_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_ENABLED;
        evidence.enabled_rb_ok
    }

    fn restore_clock(initial: ClockState, evidence: &mut ClockEvidence) {
        if evidence.promoted {
            unsafe {
                core::ptr::write_volatile(CLK_UART_CTRL, initial.ctrl);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
        }
        evidence.final_ctrl_exact =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } == initial.ctrl;
    }

    fn setup_uart2() {
        write(UART_CR, 0);
        write(UART_IMSC, 0);
        write(UART_ICR, 0x7ff);
        write(UART_RSR_ECR, 0);
        write(UART_IBRD, 27);
        write(UART_FBRD, 8);
        write(UART_LCRH, 3 << 5);
        write(UART_CR, UART_CR_LOOPBACK);
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn cleanup_uart_irq(saved: Option<rp1_rt::Uart2IrqSaved>, uart_touched: bool) {
        if uart_touched {
            write(UART_IMSC, 0);
            write(UART_ICR, UART_INT_RX);
            write(UART_CR, 0);
        }
        if let Some(saved) = saved {
            unsafe {
                rp1_rt::restore_uart2_irq(saved);
            }
        }
    }

    fn preflight_ok(
        route: rp1_rt::Uart2IrqRouteSnapshot,
        reset_asserted: bool,
        reset_done: bool,
    ) -> bool {
        let reset_coherent = reset_state_matches(reset_asserted, reset_done, reset_asserted);
        route.vtor == 0x2000_0000
            && route.primask == 0
            && route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & IRQ43_BIT1 == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
            && reset_coherent
    }

    const fn reset_state_matches(asserted: bool, done: bool, want_asserted: bool) -> bool {
        if want_asserted {
            asserted && !done
        } else {
            !asserted && done
        }
    }

    fn exercise_reset(resets: &mut ResetController, initially_asserted: bool) -> bool {
        if initially_asserted
            && resets
                .deassert_uart_clock_ready(UartReset::Uart2, POLL_LIMIT)
                .is_err()
        {
            return false;
        }
        resets
            .assert_uart_clock_ready(UartReset::Uart2, POLL_LIMIT)
            .is_ok()
            && resets
                .deassert_uart_clock_ready(UartReset::Uart2, POLL_LIMIT)
                .is_ok()
    }

    fn restore_initial_reset(
        resets: &mut ResetController,
        reset_attempted: bool,
        initially_asserted: bool,
    ) {
        if !reset_attempted {
            return;
        }
        let asserted = unsafe { core::ptr::read_volatile(RESET_CTRL1) } & RESET_UART2 != 0;
        let done = unsafe { core::ptr::read_volatile(RESET_DONE1) } & RESET_UART2 != 0;
        if reset_state_matches(asserted, done, initially_asserted) {
            return;
        }
        if initially_asserted && !asserted && done {
            let _ = resets.assert_uart_clock_ready(UartReset::Uart2, POLL_LIMIT);
        } else if !initially_asserted && asserted && !done {
            let _ = resets.deassert_uart_clock_ready(UartReset::Uart2, POLL_LIMIT);
        }
    }

    fn pack_route(route: rp1_rt::Uart2IrqRouteSnapshot) -> u32 {
        ((route.iser1 & IRQ43_BIT1 != 0) as u32)
            | (((route.ispr1 & IRQ43_BIT1 != 0) as u32) << 1)
            | (((route.iabr1 & IRQ43_BIT1 != 0) as u32) << 2)
            | ((route.primask & 1) << 3)
            | (((route.vtor == 0x2000_0000) as u32) << 4)
            | (((route.iser1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 10)
            | (((route.iser0 != 0) as u32) << 16)
            | (((route.ispr0 != 0) as u32) << 17)
            | (((route.iabr0 != 0) as u32) << 18)
    }

    fn pack_event() -> u32 {
        (IPSR.load(Ordering::Relaxed) & 0xff)
            | ((COUNT.load(Ordering::Acquire) & 0xff) << 8)
            | ((BYTE_RSR.load(Ordering::Relaxed) & 0xff) << 16)
            | (((BYTE_RSR.load(Ordering::Relaxed) >> 16) & 0xff) << 24)
    }

    fn pack_source_ready(ris: u32, mis: u32, route: rp1_rt::Uart2IrqRouteSnapshot) -> u32 {
        ((ris & UART_INT_RX != 0) as u32)
            | (((mis & UART_INT_RX != 0) as u32) << 1)
            | (((route.ispr1 & IRQ43_BIT1 != 0) as u32) << 2)
            | (((route.iser1 & IRQ43_BIT1 != 0) as u32) << 3)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | ((ris & 0xff) << 16)
            | ((mis & 0xff) << 24)
    }

    fn pack_tail(
        reset_ctrl_before: u32,
        reset_ctrl_final: u32,
        reset_done_before: u32,
        reset_done_final: u32,
        source_ready: u32,
        route_before: rp1_rt::Uart2IrqRouteSnapshot,
        handler_route: u32,
        route_final: rp1_rt::Uart2IrqRouteSnapshot,
        clock: ClockEvidence,
    ) -> u32 {
        ((reset_ctrl_before & RESET_UART2 != 0) as u32)
            | (((reset_ctrl_final & RESET_UART2 != 0) as u32) << 1)
            | (((reset_done_before & RESET_UART2 != 0) as u32) << 2)
            | (((reset_done_final & RESET_UART2 != 0) as u32) << 3)
            | ((route_before.primask & 1) << 4)
            | ((route_final.primask & 1) << 5)
            | (((route_before.iabr1 & IRQ43_BIT1 != 0) as u32) << 8)
            | (((handler_route & (1 << 2) != 0) as u32) << 9)
            | (((route_final.iabr1 & IRQ43_BIT1 != 0) as u32) << 10)
            | ((source_ready & 0x03) << 16)
            | ((clock.initial_active as u32) << 20)
            | ((clock.initial_inactive_exact as u32) << 21)
            | ((clock.promoted as u32) << 22)
            | ((clock.source_rb_ok as u32) << 23)
            | ((clock.sel1 as u32) << 24)
            | ((clock.enabled_rb_ok as u32) << 25)
            | ((clock.final_ctrl_exact as u32) << 26)
            | ((clock.div1 as u32) << 27)
            | ((clock.pll_exact_lock as u32) << 28)
            | ((clock.pri_ph_bit4 as u32) << 29)
    }

    fn reset_atomics() {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_RIS.store(0, Ordering::Relaxed);
        FIRST_MIS.store(0, Ordering::Relaxed);
        FINAL_RIS.store(0, Ordering::Relaxed);
        FINAL_MIS.store(0, Ordering::Relaxed);
        HANDLER_ROUTE.store(0, Ordering::Relaxed);
        BYTE_RSR.store(0, Ordering::Relaxed);
        STORM.store(0, Ordering::Relaxed);
    }

    fn publish(fields: [u32; 15]) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            for (index, value) in fields.into_iter().enumerate() {
                core::ptr::write_volatile(words.add(index + 1), value);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn read(offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((UART2_BASE + offset) as *const u32) }
    }

    fn write(offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((UART2_BASE + offset) as *mut u32, value);
        }
    }
}
#[cfg(all(target_arch = "arm", feature = "uart3-local-nvic44-delivery"))]
mod uart3_local_nvic44_delivery {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rp1_hal::reset::{ResetController, UartReset};

    const MAGIC: u32 = u32::from_le_bytes(*b"U3I1");
    const PASS: u32 = 1;
    const FAIL_PREFLIGHT: u32 = 2;
    const FAIL_RESET: u32 = 3;
    const FAIL_SETUP: u32 = 4;
    const FAIL_SOURCE: u32 = 5;
    const FAIL_TIMEOUT: u32 = 6;
    const FAIL_IPSR: u32 = 7;
    const FAIL_STORM: u32 = 8;
    const FAIL_RESET_DONE: u32 = 9;
    const FAIL_IRQ44_RESTORE: u32 = 10;
    const FAIL_IRQ53_CHANGED: u32 = 11;

    const UART3_BASE: usize = 0x4003_c000;
    const UART_DR: usize = 0x00;
    const UART_RSR_ECR: usize = 0x04;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2c;
    const UART_CR: usize = 0x30;
    const UART_IMSC: usize = 0x38;
    const UART_RIS: usize = 0x3c;
    const UART_MIS: usize = 0x40;
    const UART_ICR: usize = 0x44;
    const UART_PERIPH_ID0: usize = 0xfe0;
    const UART_CR_LOOPBACK: u32 = 1 | (1 << 7) | (1 << 8) | (1 << 9);
    const UART_INT_RX: u32 = 1 << 4;
    const UART_RXFE: u32 = 1 << 4;
    const TEST_BYTE: u32 = 0xa3;
    const POLL_LIMIT: usize = 100_000;
    const STABILITY_US: u64 = 4_000;
    const IRQ44_BIT1: u32 = 1 << 12;
    const IRQ53_BIT1: u32 = 1 << 21;
    const RESET_CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
    const RESET_UART3: u32 = 1 << 29;
    const CLK_UART_CTRL: *mut u32 = 0x4001_8054 as *mut u32;
    const CLK_UART_DIV_INT: *const u32 = 0x4001_8058 as *const u32;
    const CLK_UART_SEL: *const u32 = 0x4001_8060 as *const u32;
    const PLL_SYS_CS: *const u32 = 0x4002_0000 as *const u32;
    const PLL_SYS_PRIM: *const u32 = 0x4002_0010 as *const u32;
    const CLK_UART_CTRL_RELEVANT: u32 = 0x0000_0fe0;
    const CLK_UART_SOURCE: u32 = 0x0000_0040;
    const CLK_UART_ENABLED: u32 = 0x0000_0840;
    const PLL_SYS_CS_EXPECTED: u32 = 0x8000_0001;
    const PLL_SYS_PRIM_PRI_PH: u32 = 1 << 4;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_RIS: AtomicU32 = AtomicU32::new(0);
    static FIRST_MIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_RIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_MIS: AtomicU32 = AtomicU32::new(0);
    static HANDLER_ROUTE: AtomicU32 = AtomicU32::new(0);
    static BYTE_RSR: AtomicU32 = AtomicU32::new(0);
    static STORM: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn UART3_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Release);
        if old > 3 {
            STORM.store(1, Ordering::Relaxed);
        }
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_RIS.store(read(UART_RIS), Ordering::Relaxed);
            FIRST_MIS.store(read(UART_MIS), Ordering::Relaxed);
            HANDLER_ROUTE.store(
                pack_route(rp1_rt::uart3_irq_route_snapshot()),
                Ordering::Relaxed,
            );
            let byte = if read(UART_FR) & UART_RXFE == 0 {
                read(UART_DR) & 0xff
            } else {
                0x100
            };
            BYTE_RSR.store(
                byte | ((read(UART_RSR_ECR) & 0xff) << 16),
                Ordering::Relaxed,
            );
        }
        write(UART_IMSC, 0);
        write(UART_ICR, UART_INT_RX);
        FINAL_RIS.store(read(UART_RIS), Ordering::Relaxed);
        FINAL_MIS.store(read(UART_MIS), Ordering::Relaxed);
    }

    pub fn run_and_publish(resets: &mut ResetController) -> u32 {
        reset_atomics();
        let mut fields = [0u32; 15];
        let mut saved = None;
        let mut decision = PASS;
        let mut flags = 0u32;
        let mut armed_imsc = 0u32;
        let mut enabled_iser1 = 0u32;
        let mut source_ready = 0u32;
        let mut source_ispr1 = 0u32;
        let mut reset_attempted = false;
        let mut uart_touched = false;
        let clock_initial = read_clock_state();
        let mut clock_evidence = ClockEvidence::from_initial(clock_initial);
        let reset_ctrl_before = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_before = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        let reset_initial_asserted = reset_ctrl_before & RESET_UART3 != 0;
        let reset_initial_done = reset_done_before & RESET_UART3 != 0;
        let route_before = rp1_rt::uart3_irq_route_snapshot();
        let mut identity_ok = false;

        if !preflight_ok(route_before, reset_initial_asserted, reset_initial_done)
            || !prepare_clock(&mut clock_evidence)
        {
            decision = FAIL_PREFLIGHT;
        } else {
            reset_attempted = true;
            if !exercise_reset(resets, reset_initial_asserted) {
                decision = FAIL_RESET;
            } else {
                identity_ok = read(UART_PERIPH_ID0) & 0xff == 0x11;
                if !identity_ok {
                    decision = FAIL_PREFLIGHT;
                } else {
                    uart_touched = true;
                    setup_uart3();
                    if read(UART_CR) & UART_CR_LOOPBACK != UART_CR_LOOPBACK {
                        decision = FAIL_SETUP;
                    }
                }
            }
        }

        if decision == PASS {
            saved = unsafe { rp1_rt::prepare_uart3_irq() };
            if saved.is_none() {
                decision = FAIL_PREFLIGHT;
            }
        }
        if decision == PASS {
            write(UART_IMSC, UART_INT_RX);
            armed_imsc = read(UART_IMSC);
            write(UART_DR, TEST_BYTE);
            unsafe {
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            let mut source_ris = 0;
            let mut source_mis = 0;
            let mut source_route = rp1_rt::uart3_irq_route_snapshot();
            for poll in 0..POLL_LIMIT {
                source_ris = read(UART_RIS);
                source_mis = read(UART_MIS);
                source_route = rp1_rt::uart3_irq_route_snapshot();
                if source_ris & UART_INT_RX != 0
                    && source_mis & UART_INT_RX != 0
                    && source_route.ispr1 & IRQ44_BIT1 != 0
                {
                    fields[8] = poll as u32;
                    break;
                }
            }
            source_ready = pack_source_ready(source_ris, source_mis, source_route);
            source_ispr1 = source_route.ispr1;
            if source_ris & UART_INT_RX == 0
                || source_mis & UART_INT_RX == 0
                || source_route.ispr1 & IRQ44_BIT1 == 0
                || source_route.iser1 & IRQ44_BIT1 != 0
            {
                decision = FAIL_SOURCE;
            } else {
                flags |= 1 << 1;
                unsafe {
                    rp1_rt::enable_uart3_irq_after_source_asserted();
                }
                enabled_iser1 = rp1_rt::uart3_irq_route_snapshot().iser1;
            }
        }

        if decision == PASS {
            for _ in 0..POLL_LIMIT {
                if COUNT.load(Ordering::Acquire) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            if COUNT.load(Ordering::Acquire) == 0 {
                decision = FAIL_TIMEOUT;
            } else {
                super::busy_wait_us(STABILITY_US);
            }
        }

        cleanup_uart_irq(saved, uart_touched);
        let (final_ris, final_mis, final_imsc) = if uart_touched {
            (read(UART_RIS), read(UART_MIS), read(UART_IMSC))
        } else {
            (0, 0, 0)
        };
        restore_initial_reset(resets, reset_attempted, reset_initial_asserted);
        restore_clock(clock_initial, &mut clock_evidence);
        let route_final = rp1_rt::uart3_irq_route_snapshot();
        let reset_ctrl_final = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_final = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        fields[2] = pack_event();
        fields[3] = (FIRST_RIS.load(Ordering::Relaxed) & 0xffff)
            | (FIRST_MIS.load(Ordering::Relaxed) << 16);
        fields[4] = (final_ris & 0xffff) | (final_mis << 16);
        fields[5] = (armed_imsc & 0xffff) | (final_imsc << 16);
        fields[6] = route_before.iser0;
        fields[7] = route_before.iser1;
        fields[8] = enabled_iser1;
        fields[9] = route_final.iser0;
        fields[10] = route_final.iser1;
        fields[11] = route_before.ispr1;
        fields[12] = source_ispr1;
        fields[13] = route_final.ispr1;
        fields[14] = pack_tail(
            reset_ctrl_before,
            reset_ctrl_final,
            reset_done_before,
            reset_done_final,
            source_ready,
            route_before,
            HANDLER_ROUTE.load(Ordering::Relaxed),
            route_final,
            clock_evidence,
        );

        let irq44_restored = route_final.iser1 == route_before.iser1
            && route_final.ispr1 & IRQ44_BIT1 == 0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask;
        let irq53_unchanged = route_final.iser1 & IRQ53_BIT1 == route_before.iser1 & IRQ53_BIT1
            && route_final.ispr1 & IRQ53_BIT1 == route_before.ispr1 & IRQ53_BIT1
            && route_final.iabr1 & IRQ53_BIT1 == route_before.iabr1 & IRQ53_BIT1
            && route_final.ispr1 & !IRQ44_BIT1 == route_before.ispr1 & !IRQ44_BIT1;
        let reset_restored = reset_state_matches(
            reset_ctrl_final & RESET_UART3 != 0,
            reset_done_final & RESET_UART3 != 0,
            reset_initial_asserted,
        );
        let no_storm = STORM.load(Ordering::Relaxed) == 0 && COUNT.load(Ordering::Acquire) == 1;
        let source_clean = final_imsc == 0 && final_mis == 0 && final_ris & UART_INT_RX == 0;
        let ipsr_ok = IPSR.load(Ordering::Relaxed) == rp1_rt::UART3_VECTOR_INDEX as u32;
        let byte_rsr = BYTE_RSR.load(Ordering::Relaxed);
        let byte_rsr_ok = byte_rsr & 0xff == TEST_BYTE && byte_rsr & 0x00ff_0000 == 0;
        let source_before_enable = source_ready & 0x0f == 0x07;
        let first_source_ok = FIRST_RIS.load(Ordering::Relaxed) & UART_INT_RX != 0
            && FIRST_MIS.load(Ordering::Relaxed) & UART_INT_RX != 0;
        let handler_active_ok = HANDLER_ROUTE.load(Ordering::Relaxed) & (1 << 2) != 0;
        let enable_ok = enabled_iser1 == IRQ44_BIT1;
        let final_exact = route_final.iser0 == route_before.iser0
            && route_final.iser1 == route_before.iser1
            && route_final.ispr0 == route_before.ispr0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr0 == route_before.iabr0
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask
            && clock_evidence.final_ctrl_exact;

        flags |= (route_before.vtor == 0x2000_0000) as u32;
        flags |= (source_before_enable as u32) << 1;
        flags |= (ipsr_ok as u32) << 2;
        flags |= (no_storm as u32) << 3;
        flags |= (reset_restored as u32) << 4;
        flags |= (irq44_restored as u32) << 5;
        flags |= (irq53_unchanged as u32) << 6;
        flags |= (identity_ok as u32) << 7;
        flags |= (source_clean as u32) << 8;
        flags |= (byte_rsr_ok as u32) << 9;
        flags |= (enable_ok as u32) << 10;
        flags |= (first_source_ok as u32) << 11;
        flags |= (handler_active_ok as u32) << 12;
        flags |= (final_exact as u32) << 13;
        if flags & 0x3fff == 0x3fff && decision == PASS {
            flags |= 1 << 31;
        } else if decision == PASS && !ipsr_ok {
            decision = FAIL_IPSR;
        } else if decision == PASS && (!no_storm || !handler_active_ok) {
            decision = FAIL_STORM;
        } else if decision == PASS && !reset_restored {
            decision = FAIL_RESET_DONE;
        } else if decision == PASS && (!irq44_restored || !final_exact) {
            decision = FAIL_IRQ44_RESTORE;
        } else if decision == PASS && !irq53_unchanged {
            decision = FAIL_IRQ53_CHANGED;
        } else if decision == PASS && (!source_clean || !enable_ok || !first_source_ok) {
            decision = FAIL_SETUP;
        } else if decision == PASS && !byte_rsr_ok {
            decision = FAIL_SETUP;
        }

        fields[0] = decision;
        fields[1] = flags;
        publish(fields);
        decision
    }

    #[derive(Copy, Clone)]
    struct ClockState {
        ctrl: u32,
        div_int: u32,
        sel: u32,
        pll_cs: u32,
        pll_prim: u32,
    }

    #[derive(Copy, Clone)]
    struct ClockEvidence {
        initial_active: bool,
        initial_inactive_exact: bool,
        promoted: bool,
        source_rb_ok: bool,
        sel1: bool,
        enabled_rb_ok: bool,
        final_ctrl_exact: bool,
        div1: bool,
        pll_exact_lock: bool,
        pri_ph_bit4: bool,
    }

    impl ClockEvidence {
        const fn from_initial(state: ClockState) -> Self {
            let initial_active = state.ctrl & CLK_UART_CTRL_RELEVANT == CLK_UART_ENABLED;
            let initial_inactive_exact = state.ctrl == 0;
            let div1 = state.div_int == 1;
            let pll_exact_lock = state.pll_cs == PLL_SYS_CS_EXPECTED;
            let pri_ph_bit4 = state.pll_prim & PLL_SYS_PRIM_PRI_PH != 0;
            Self {
                initial_active,
                initial_inactive_exact,
                promoted: false,
                source_rb_ok: initial_active,
                sel1: state.sel == 1,
                enabled_rb_ok: initial_active,
                final_ctrl_exact: false,
                div1,
                pll_exact_lock,
                pri_ph_bit4,
            }
        }
    }

    fn read_clock_state() -> ClockState {
        unsafe {
            ClockState {
                ctrl: core::ptr::read_volatile(CLK_UART_CTRL),
                div_int: core::ptr::read_volatile(CLK_UART_DIV_INT),
                sel: core::ptr::read_volatile(CLK_UART_SEL),
                pll_cs: core::ptr::read_volatile(PLL_SYS_CS),
                pll_prim: core::ptr::read_volatile(PLL_SYS_PRIM),
            }
        }
    }

    fn prepare_clock(evidence: &mut ClockEvidence) -> bool {
        if !evidence.div1 || !evidence.pll_exact_lock || !evidence.pri_ph_bit4 {
            return false;
        }
        if evidence.initial_active {
            return evidence.sel1;
        }
        if !evidence.initial_inactive_exact {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_SOURCE);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.promoted = true;
        evidence.source_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_SOURCE;
        evidence.sel1 = unsafe { core::ptr::read_volatile(CLK_UART_SEL) } == 1;
        if !evidence.source_rb_ok || !evidence.sel1 {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_ENABLED);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.enabled_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_ENABLED;
        evidence.enabled_rb_ok
    }

    fn restore_clock(initial: ClockState, evidence: &mut ClockEvidence) {
        if evidence.promoted {
            unsafe {
                core::ptr::write_volatile(CLK_UART_CTRL, initial.ctrl);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
        }
        evidence.final_ctrl_exact =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } == initial.ctrl;
    }

    fn setup_uart3() {
        write(UART_CR, 0);
        write(UART_IMSC, 0);
        write(UART_ICR, 0x7ff);
        write(UART_RSR_ECR, 0);
        write(UART_IBRD, 27);
        write(UART_FBRD, 8);
        write(UART_LCRH, 3 << 5);
        write(UART_CR, UART_CR_LOOPBACK);
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn cleanup_uart_irq(saved: Option<rp1_rt::Uart3IrqSaved>, uart_touched: bool) {
        if uart_touched {
            write(UART_IMSC, 0);
            write(UART_ICR, UART_INT_RX);
            write(UART_CR, 0);
        }
        if let Some(saved) = saved {
            unsafe {
                rp1_rt::restore_uart3_irq(saved);
            }
        }
    }

    fn preflight_ok(
        route: rp1_rt::Uart3IrqRouteSnapshot,
        reset_asserted: bool,
        reset_done: bool,
    ) -> bool {
        let reset_coherent = reset_state_matches(reset_asserted, reset_done, reset_asserted);
        route.vtor == 0x2000_0000
            && route.primask == 0
            && route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & IRQ44_BIT1 == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
            && reset_coherent
    }

    const fn reset_state_matches(asserted: bool, done: bool, want_asserted: bool) -> bool {
        if want_asserted {
            asserted && !done
        } else {
            !asserted && done
        }
    }

    fn exercise_reset(resets: &mut ResetController, initially_asserted: bool) -> bool {
        if initially_asserted
            && resets
                .deassert_uart_clock_ready(UartReset::Uart3, POLL_LIMIT)
                .is_err()
        {
            return false;
        }
        resets
            .assert_uart_clock_ready(UartReset::Uart3, POLL_LIMIT)
            .is_ok()
            && resets
                .deassert_uart_clock_ready(UartReset::Uart3, POLL_LIMIT)
                .is_ok()
    }

    fn restore_initial_reset(
        resets: &mut ResetController,
        reset_attempted: bool,
        initially_asserted: bool,
    ) {
        if !reset_attempted {
            return;
        }
        let asserted = unsafe { core::ptr::read_volatile(RESET_CTRL1) } & RESET_UART3 != 0;
        let done = unsafe { core::ptr::read_volatile(RESET_DONE1) } & RESET_UART3 != 0;
        if reset_state_matches(asserted, done, initially_asserted) {
            return;
        }
        if initially_asserted && !asserted && done {
            let _ = resets.assert_uart_clock_ready(UartReset::Uart3, POLL_LIMIT);
        } else if !initially_asserted && asserted && !done {
            let _ = resets.deassert_uart_clock_ready(UartReset::Uart3, POLL_LIMIT);
        }
    }

    fn pack_route(route: rp1_rt::Uart3IrqRouteSnapshot) -> u32 {
        ((route.iser1 & IRQ44_BIT1 != 0) as u32)
            | (((route.ispr1 & IRQ44_BIT1 != 0) as u32) << 1)
            | (((route.iabr1 & IRQ44_BIT1 != 0) as u32) << 2)
            | ((route.primask & 1) << 3)
            | (((route.vtor == 0x2000_0000) as u32) << 4)
            | (((route.iser1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 10)
            | (((route.iser0 != 0) as u32) << 16)
            | (((route.ispr0 != 0) as u32) << 17)
            | (((route.iabr0 != 0) as u32) << 18)
    }

    fn pack_event() -> u32 {
        (IPSR.load(Ordering::Relaxed) & 0xff)
            | ((COUNT.load(Ordering::Acquire) & 0xff) << 8)
            | ((BYTE_RSR.load(Ordering::Relaxed) & 0xff) << 16)
            | (((BYTE_RSR.load(Ordering::Relaxed) >> 16) & 0xff) << 24)
    }

    fn pack_source_ready(ris: u32, mis: u32, route: rp1_rt::Uart3IrqRouteSnapshot) -> u32 {
        ((ris & UART_INT_RX != 0) as u32)
            | (((mis & UART_INT_RX != 0) as u32) << 1)
            | (((route.ispr1 & IRQ44_BIT1 != 0) as u32) << 2)
            | (((route.iser1 & IRQ44_BIT1 != 0) as u32) << 3)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | ((ris & 0xff) << 16)
            | ((mis & 0xff) << 24)
    }

    fn pack_tail(
        reset_ctrl_before: u32,
        reset_ctrl_final: u32,
        reset_done_before: u32,
        reset_done_final: u32,
        source_ready: u32,
        route_before: rp1_rt::Uart3IrqRouteSnapshot,
        handler_route: u32,
        route_final: rp1_rt::Uart3IrqRouteSnapshot,
        clock: ClockEvidence,
    ) -> u32 {
        ((reset_ctrl_before & RESET_UART3 != 0) as u32)
            | (((reset_ctrl_final & RESET_UART3 != 0) as u32) << 1)
            | (((reset_done_before & RESET_UART3 != 0) as u32) << 2)
            | (((reset_done_final & RESET_UART3 != 0) as u32) << 3)
            | ((route_before.primask & 1) << 4)
            | ((route_final.primask & 1) << 5)
            | (((route_before.iabr1 & IRQ44_BIT1 != 0) as u32) << 8)
            | (((handler_route & (1 << 2) != 0) as u32) << 9)
            | (((route_final.iabr1 & IRQ44_BIT1 != 0) as u32) << 10)
            | ((source_ready & 0x03) << 16)
            | ((clock.initial_active as u32) << 20)
            | ((clock.initial_inactive_exact as u32) << 21)
            | ((clock.promoted as u32) << 22)
            | ((clock.source_rb_ok as u32) << 23)
            | ((clock.sel1 as u32) << 24)
            | ((clock.enabled_rb_ok as u32) << 25)
            | ((clock.final_ctrl_exact as u32) << 26)
            | ((clock.div1 as u32) << 27)
            | ((clock.pll_exact_lock as u32) << 28)
            | ((clock.pri_ph_bit4 as u32) << 29)
    }

    fn reset_atomics() {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_RIS.store(0, Ordering::Relaxed);
        FIRST_MIS.store(0, Ordering::Relaxed);
        FINAL_RIS.store(0, Ordering::Relaxed);
        FINAL_MIS.store(0, Ordering::Relaxed);
        HANDLER_ROUTE.store(0, Ordering::Relaxed);
        BYTE_RSR.store(0, Ordering::Relaxed);
        STORM.store(0, Ordering::Relaxed);
    }

    fn publish(fields: [u32; 15]) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            for (index, value) in fields.into_iter().enumerate() {
                core::ptr::write_volatile(words.add(index + 1), value);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn read(offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((UART3_BASE + offset) as *const u32) }
    }

    fn write(offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((UART3_BASE + offset) as *mut u32, value);
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-nvic45-delivery"))]
mod uart4_local_nvic45_delivery {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rp1_hal::reset::{ResetController, UartReset};

    const MAGIC: u32 = u32::from_le_bytes(*b"U4I1");
    const PASS: u32 = 1;
    const FAIL_PREFLIGHT: u32 = 2;
    const FAIL_RESET: u32 = 3;
    const FAIL_SETUP: u32 = 4;
    const FAIL_SOURCE: u32 = 5;
    const FAIL_TIMEOUT: u32 = 6;
    const FAIL_IPSR: u32 = 7;
    const FAIL_STORM: u32 = 8;
    const FAIL_RESET_DONE: u32 = 9;
    const FAIL_IRQ45_RESTORE: u32 = 10;
    const FAIL_IRQ53_CHANGED: u32 = 11;

    const UART4_BASE: usize = 0x4004_0000;
    const UART_DR: usize = 0x00;
    const UART_RSR_ECR: usize = 0x04;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2c;
    const UART_CR: usize = 0x30;
    const UART_IMSC: usize = 0x38;
    const UART_RIS: usize = 0x3c;
    const UART_MIS: usize = 0x40;
    const UART_ICR: usize = 0x44;
    const UART_PERIPH_ID0: usize = 0xfe0;
    const UART_CR_LOOPBACK: u32 = 1 | (1 << 7) | (1 << 8) | (1 << 9);
    const UART_INT_RX: u32 = 1 << 4;
    const UART_RXFE: u32 = 1 << 4;
    const TEST_BYTE: u32 = 0xa4;
    const POLL_LIMIT: usize = 100_000;
    const STABILITY_US: u64 = 4_000;
    const IRQ45_BIT1: u32 = 1 << 13;
    const IRQ53_BIT1: u32 = 1 << 21;
    const RESET_CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
    const RESET_UART4: u32 = 1 << 30;
    const CLK_UART_CTRL: *mut u32 = 0x4001_8054 as *mut u32;
    const CLK_UART_DIV_INT: *const u32 = 0x4001_8058 as *const u32;
    const CLK_UART_SEL: *const u32 = 0x4001_8060 as *const u32;
    const PLL_SYS_CS: *const u32 = 0x4002_0000 as *const u32;
    const PLL_SYS_PRIM: *const u32 = 0x4002_0010 as *const u32;
    const CLK_UART_CTRL_RELEVANT: u32 = 0x0000_0fe0;
    const CLK_UART_SOURCE: u32 = 0x0000_0040;
    const CLK_UART_ENABLED: u32 = 0x0000_0840;
    const PLL_SYS_CS_EXPECTED: u32 = 0x8000_0001;
    const PLL_SYS_PRIM_PRI_PH: u32 = 1 << 4;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_RIS: AtomicU32 = AtomicU32::new(0);
    static FIRST_MIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_RIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_MIS: AtomicU32 = AtomicU32::new(0);
    static HANDLER_ROUTE: AtomicU32 = AtomicU32::new(0);
    static BYTE_RSR: AtomicU32 = AtomicU32::new(0);
    static STORM: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn UART4_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Release);
        if old > 3 {
            STORM.store(1, Ordering::Relaxed);
        }
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_RIS.store(read(UART_RIS), Ordering::Relaxed);
            FIRST_MIS.store(read(UART_MIS), Ordering::Relaxed);
            HANDLER_ROUTE.store(
                pack_route(rp1_rt::uart4_irq_route_snapshot()),
                Ordering::Relaxed,
            );
            let byte = if read(UART_FR) & UART_RXFE == 0 {
                read(UART_DR) & 0xff
            } else {
                0x100
            };
            BYTE_RSR.store(
                byte | ((read(UART_RSR_ECR) & 0xff) << 16),
                Ordering::Relaxed,
            );
        }
        write(UART_IMSC, 0);
        write(UART_ICR, UART_INT_RX);
        FINAL_RIS.store(read(UART_RIS), Ordering::Relaxed);
        FINAL_MIS.store(read(UART_MIS), Ordering::Relaxed);
    }

    pub fn run_and_publish(resets: &mut ResetController) -> u32 {
        reset_atomics();
        let mut fields = [0u32; 15];
        let mut saved = None;
        let mut decision = PASS;
        let mut flags = 0u32;
        let mut armed_imsc = 0u32;
        let mut enabled_iser1 = 0u32;
        let mut source_ready = 0u32;
        let mut source_ispr1 = 0u32;
        let mut reset_attempted = false;
        let mut uart_touched = false;
        let clock_initial = read_clock_state();
        let mut clock_evidence = ClockEvidence::from_initial(clock_initial);
        let reset_ctrl_before = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_before = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        let reset_initial_asserted = reset_ctrl_before & RESET_UART4 != 0;
        let reset_initial_done = reset_done_before & RESET_UART4 != 0;
        let route_before = rp1_rt::uart4_irq_route_snapshot();
        let mut identity_ok = false;

        if !preflight_ok(route_before, reset_initial_asserted, reset_initial_done)
            || !prepare_clock(&mut clock_evidence)
        {
            decision = FAIL_PREFLIGHT;
        } else {
            reset_attempted = true;
            if !exercise_reset(resets, reset_initial_asserted) {
                decision = FAIL_RESET;
            } else {
                identity_ok = read(UART_PERIPH_ID0) & 0xff == 0x11;
                if !identity_ok {
                    decision = FAIL_PREFLIGHT;
                } else {
                    uart_touched = true;
                    setup_uart4();
                    if read(UART_CR) & UART_CR_LOOPBACK != UART_CR_LOOPBACK {
                        decision = FAIL_SETUP;
                    }
                }
            }
        }

        if decision == PASS {
            saved = unsafe { rp1_rt::prepare_uart4_irq() };
            if saved.is_none() {
                decision = FAIL_PREFLIGHT;
            }
        }
        if decision == PASS {
            write(UART_IMSC, UART_INT_RX);
            armed_imsc = read(UART_IMSC);
            write(UART_DR, TEST_BYTE);
            unsafe {
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            let mut source_ris = 0;
            let mut source_mis = 0;
            let mut source_route = rp1_rt::uart4_irq_route_snapshot();
            for poll in 0..POLL_LIMIT {
                source_ris = read(UART_RIS);
                source_mis = read(UART_MIS);
                source_route = rp1_rt::uart4_irq_route_snapshot();
                if source_ris & UART_INT_RX != 0
                    && source_mis & UART_INT_RX != 0
                    && source_route.ispr1 & IRQ45_BIT1 != 0
                {
                    fields[8] = poll as u32;
                    break;
                }
            }
            source_ready = pack_source_ready(source_ris, source_mis, source_route);
            source_ispr1 = source_route.ispr1;
            if source_ris & UART_INT_RX == 0
                || source_mis & UART_INT_RX == 0
                || source_route.ispr1 & IRQ45_BIT1 == 0
                || source_route.iser1 & IRQ45_BIT1 != 0
            {
                decision = FAIL_SOURCE;
            } else {
                flags |= 1 << 1;
                unsafe {
                    rp1_rt::enable_uart4_irq_after_source_asserted();
                }
                enabled_iser1 = rp1_rt::uart4_irq_route_snapshot().iser1;
            }
        }

        if decision == PASS {
            for _ in 0..POLL_LIMIT {
                if COUNT.load(Ordering::Acquire) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            if COUNT.load(Ordering::Acquire) == 0 {
                decision = FAIL_TIMEOUT;
            } else {
                super::busy_wait_us(STABILITY_US);
            }
        }

        cleanup_uart_irq(saved, uart_touched);
        let (final_ris, final_mis, final_imsc) = if uart_touched {
            (read(UART_RIS), read(UART_MIS), read(UART_IMSC))
        } else {
            (0, 0, 0)
        };
        restore_initial_reset(resets, reset_attempted, reset_initial_asserted);
        restore_clock(clock_initial, &mut clock_evidence);
        let route_final = rp1_rt::uart4_irq_route_snapshot();
        let reset_ctrl_final = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_final = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        fields[2] = pack_event();
        fields[3] = (FIRST_RIS.load(Ordering::Relaxed) & 0xffff)
            | (FIRST_MIS.load(Ordering::Relaxed) << 16);
        fields[4] = (final_ris & 0xffff) | (final_mis << 16);
        fields[5] = (armed_imsc & 0xffff) | (final_imsc << 16);
        fields[6] = route_before.iser0;
        fields[7] = route_before.iser1;
        fields[8] = enabled_iser1;
        fields[9] = route_final.iser0;
        fields[10] = route_final.iser1;
        fields[11] = route_before.ispr1;
        fields[12] = source_ispr1;
        fields[13] = route_final.ispr1;
        fields[14] = pack_tail(
            reset_ctrl_before,
            reset_ctrl_final,
            reset_done_before,
            reset_done_final,
            source_ready,
            route_before,
            HANDLER_ROUTE.load(Ordering::Relaxed),
            route_final,
            clock_evidence,
        );

        let irq45_restored = route_final.iser1 == route_before.iser1
            && route_final.ispr1 & IRQ45_BIT1 == 0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask;
        let irq53_unchanged = route_final.iser1 & IRQ53_BIT1 == route_before.iser1 & IRQ53_BIT1
            && route_final.ispr1 & IRQ53_BIT1 == route_before.ispr1 & IRQ53_BIT1
            && route_final.iabr1 & IRQ53_BIT1 == route_before.iabr1 & IRQ53_BIT1
            && route_final.ispr1 & !IRQ45_BIT1 == route_before.ispr1 & !IRQ45_BIT1;
        let reset_restored = reset_state_matches(
            reset_ctrl_final & RESET_UART4 != 0,
            reset_done_final & RESET_UART4 != 0,
            reset_initial_asserted,
        );
        let no_storm = STORM.load(Ordering::Relaxed) == 0 && COUNT.load(Ordering::Acquire) == 1;
        let source_clean = final_imsc == 0 && final_mis == 0 && final_ris & UART_INT_RX == 0;
        let ipsr_ok = IPSR.load(Ordering::Relaxed) == rp1_rt::UART4_VECTOR_INDEX as u32;
        let byte_rsr = BYTE_RSR.load(Ordering::Relaxed);
        let byte_rsr_ok = byte_rsr & 0xff == TEST_BYTE && byte_rsr & 0x00ff_0000 == 0;
        let source_before_enable = source_ready & 0x0f == 0x07;
        let first_source_ok = FIRST_RIS.load(Ordering::Relaxed) & UART_INT_RX != 0
            && FIRST_MIS.load(Ordering::Relaxed) & UART_INT_RX != 0;
        let handler_active_ok = HANDLER_ROUTE.load(Ordering::Relaxed) & (1 << 2) != 0;
        let enable_ok = enabled_iser1 == IRQ45_BIT1;
        let final_exact = route_final.iser0 == route_before.iser0
            && route_final.iser1 == route_before.iser1
            && route_final.ispr0 == route_before.ispr0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr0 == route_before.iabr0
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask
            && clock_evidence.final_ctrl_exact;

        flags |= (route_before.vtor == 0x2000_0000) as u32;
        flags |= (source_before_enable as u32) << 1;
        flags |= (ipsr_ok as u32) << 2;
        flags |= (no_storm as u32) << 3;
        flags |= (reset_restored as u32) << 4;
        flags |= (irq45_restored as u32) << 5;
        flags |= (irq53_unchanged as u32) << 6;
        flags |= (identity_ok as u32) << 7;
        flags |= (source_clean as u32) << 8;
        flags |= (byte_rsr_ok as u32) << 9;
        flags |= (enable_ok as u32) << 10;
        flags |= (first_source_ok as u32) << 11;
        flags |= (handler_active_ok as u32) << 12;
        flags |= (final_exact as u32) << 13;
        if flags & 0x3fff == 0x3fff && decision == PASS {
            flags |= 1 << 31;
        } else if decision == PASS && !ipsr_ok {
            decision = FAIL_IPSR;
        } else if decision == PASS && (!no_storm || !handler_active_ok) {
            decision = FAIL_STORM;
        } else if decision == PASS && !reset_restored {
            decision = FAIL_RESET_DONE;
        } else if decision == PASS && (!irq45_restored || !final_exact) {
            decision = FAIL_IRQ45_RESTORE;
        } else if decision == PASS && !irq53_unchanged {
            decision = FAIL_IRQ53_CHANGED;
        } else if decision == PASS && (!source_clean || !enable_ok || !first_source_ok) {
            decision = FAIL_SETUP;
        } else if decision == PASS && !byte_rsr_ok {
            decision = FAIL_SETUP;
        }

        fields[0] = decision;
        fields[1] = flags;
        publish(fields);
        decision
    }

    #[derive(Copy, Clone)]
    struct ClockState {
        ctrl: u32,
        div_int: u32,
        sel: u32,
        pll_cs: u32,
        pll_prim: u32,
    }

    #[derive(Copy, Clone)]
    struct ClockEvidence {
        initial_active: bool,
        initial_inactive_exact: bool,
        promoted: bool,
        source_rb_ok: bool,
        sel1: bool,
        enabled_rb_ok: bool,
        final_ctrl_exact: bool,
        div1: bool,
        pll_exact_lock: bool,
        pri_ph_bit4: bool,
    }

    impl ClockEvidence {
        const fn from_initial(state: ClockState) -> Self {
            let initial_active = state.ctrl & CLK_UART_CTRL_RELEVANT == CLK_UART_ENABLED;
            let initial_inactive_exact = state.ctrl == 0;
            let div1 = state.div_int == 1;
            let pll_exact_lock = state.pll_cs == PLL_SYS_CS_EXPECTED;
            let pri_ph_bit4 = state.pll_prim & PLL_SYS_PRIM_PRI_PH != 0;
            Self {
                initial_active,
                initial_inactive_exact,
                promoted: false,
                source_rb_ok: initial_active,
                sel1: state.sel == 1,
                enabled_rb_ok: initial_active,
                final_ctrl_exact: false,
                div1,
                pll_exact_lock,
                pri_ph_bit4,
            }
        }
    }

    fn read_clock_state() -> ClockState {
        unsafe {
            ClockState {
                ctrl: core::ptr::read_volatile(CLK_UART_CTRL),
                div_int: core::ptr::read_volatile(CLK_UART_DIV_INT),
                sel: core::ptr::read_volatile(CLK_UART_SEL),
                pll_cs: core::ptr::read_volatile(PLL_SYS_CS),
                pll_prim: core::ptr::read_volatile(PLL_SYS_PRIM),
            }
        }
    }

    fn prepare_clock(evidence: &mut ClockEvidence) -> bool {
        if !evidence.div1 || !evidence.pll_exact_lock || !evidence.pri_ph_bit4 {
            return false;
        }
        if evidence.initial_active {
            return evidence.sel1;
        }
        if !evidence.initial_inactive_exact {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_SOURCE);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.promoted = true;
        evidence.source_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_SOURCE;
        evidence.sel1 = unsafe { core::ptr::read_volatile(CLK_UART_SEL) } == 1;
        if !evidence.source_rb_ok || !evidence.sel1 {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_ENABLED);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.enabled_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_ENABLED;
        evidence.enabled_rb_ok
    }

    fn restore_clock(initial: ClockState, evidence: &mut ClockEvidence) {
        if evidence.promoted {
            unsafe {
                core::ptr::write_volatile(CLK_UART_CTRL, initial.ctrl);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
        }
        evidence.final_ctrl_exact =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } == initial.ctrl;
    }

    fn setup_uart4() {
        write(UART_CR, 0);
        write(UART_IMSC, 0);
        write(UART_ICR, 0x7ff);
        write(UART_RSR_ECR, 0);
        write(UART_IBRD, 27);
        write(UART_FBRD, 8);
        write(UART_LCRH, 3 << 5);
        write(UART_CR, UART_CR_LOOPBACK);
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn cleanup_uart_irq(saved: Option<rp1_rt::Uart4IrqSaved>, uart_touched: bool) {
        if uart_touched {
            write(UART_IMSC, 0);
            write(UART_ICR, UART_INT_RX);
            write(UART_CR, 0);
        }
        if let Some(saved) = saved {
            unsafe {
                rp1_rt::restore_uart4_irq(saved);
            }
        }
    }

    fn preflight_ok(
        route: rp1_rt::Uart4IrqRouteSnapshot,
        reset_asserted: bool,
        reset_done: bool,
    ) -> bool {
        let reset_coherent = reset_state_matches(reset_asserted, reset_done, reset_asserted);
        route.vtor == 0x2000_0000
            && route.primask == 0
            && route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & IRQ45_BIT1 == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
            && reset_coherent
    }

    const fn reset_state_matches(asserted: bool, done: bool, want_asserted: bool) -> bool {
        if want_asserted {
            asserted && !done
        } else {
            !asserted && done
        }
    }

    fn exercise_reset(resets: &mut ResetController, initially_asserted: bool) -> bool {
        if initially_asserted
            && resets
                .deassert_uart_clock_ready(UartReset::Uart4, POLL_LIMIT)
                .is_err()
        {
            return false;
        }
        resets
            .assert_uart_clock_ready(UartReset::Uart4, POLL_LIMIT)
            .is_ok()
            && resets
                .deassert_uart_clock_ready(UartReset::Uart4, POLL_LIMIT)
                .is_ok()
    }

    fn restore_initial_reset(
        resets: &mut ResetController,
        reset_attempted: bool,
        initially_asserted: bool,
    ) {
        if !reset_attempted {
            return;
        }
        let asserted = unsafe { core::ptr::read_volatile(RESET_CTRL1) } & RESET_UART4 != 0;
        let done = unsafe { core::ptr::read_volatile(RESET_DONE1) } & RESET_UART4 != 0;
        if reset_state_matches(asserted, done, initially_asserted) {
            return;
        }
        if initially_asserted && !asserted && done {
            let _ = resets.assert_uart_clock_ready(UartReset::Uart4, POLL_LIMIT);
        } else if !initially_asserted && asserted && !done {
            let _ = resets.deassert_uart_clock_ready(UartReset::Uart4, POLL_LIMIT);
        }
    }

    fn pack_route(route: rp1_rt::Uart4IrqRouteSnapshot) -> u32 {
        ((route.iser1 & IRQ45_BIT1 != 0) as u32)
            | (((route.ispr1 & IRQ45_BIT1 != 0) as u32) << 1)
            | (((route.iabr1 & IRQ45_BIT1 != 0) as u32) << 2)
            | ((route.primask & 1) << 3)
            | (((route.vtor == 0x2000_0000) as u32) << 4)
            | (((route.iser1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 10)
            | (((route.iser0 != 0) as u32) << 16)
            | (((route.ispr0 != 0) as u32) << 17)
            | (((route.iabr0 != 0) as u32) << 18)
    }

    fn pack_event() -> u32 {
        (IPSR.load(Ordering::Relaxed) & 0xff)
            | ((COUNT.load(Ordering::Acquire) & 0xff) << 8)
            | ((BYTE_RSR.load(Ordering::Relaxed) & 0xff) << 16)
            | (((BYTE_RSR.load(Ordering::Relaxed) >> 16) & 0xff) << 24)
    }

    fn pack_source_ready(ris: u32, mis: u32, route: rp1_rt::Uart4IrqRouteSnapshot) -> u32 {
        ((ris & UART_INT_RX != 0) as u32)
            | (((mis & UART_INT_RX != 0) as u32) << 1)
            | (((route.ispr1 & IRQ45_BIT1 != 0) as u32) << 2)
            | (((route.iser1 & IRQ45_BIT1 != 0) as u32) << 3)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | ((ris & 0xff) << 16)
            | ((mis & 0xff) << 24)
    }

    fn pack_tail(
        reset_ctrl_before: u32,
        reset_ctrl_final: u32,
        reset_done_before: u32,
        reset_done_final: u32,
        source_ready: u32,
        route_before: rp1_rt::Uart4IrqRouteSnapshot,
        handler_route: u32,
        route_final: rp1_rt::Uart4IrqRouteSnapshot,
        clock: ClockEvidence,
    ) -> u32 {
        ((reset_ctrl_before & RESET_UART4 != 0) as u32)
            | (((reset_ctrl_final & RESET_UART4 != 0) as u32) << 1)
            | (((reset_done_before & RESET_UART4 != 0) as u32) << 2)
            | (((reset_done_final & RESET_UART4 != 0) as u32) << 3)
            | ((route_before.primask & 1) << 4)
            | ((route_final.primask & 1) << 5)
            | (((route_before.iabr1 & IRQ45_BIT1 != 0) as u32) << 8)
            | (((handler_route & (1 << 2) != 0) as u32) << 9)
            | (((route_final.iabr1 & IRQ45_BIT1 != 0) as u32) << 10)
            | ((source_ready & 0x03) << 16)
            | ((clock.initial_active as u32) << 20)
            | ((clock.initial_inactive_exact as u32) << 21)
            | ((clock.promoted as u32) << 22)
            | ((clock.source_rb_ok as u32) << 23)
            | ((clock.sel1 as u32) << 24)
            | ((clock.enabled_rb_ok as u32) << 25)
            | ((clock.final_ctrl_exact as u32) << 26)
            | ((clock.div1 as u32) << 27)
            | ((clock.pll_exact_lock as u32) << 28)
            | ((clock.pri_ph_bit4 as u32) << 29)
    }

    fn reset_atomics() {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_RIS.store(0, Ordering::Relaxed);
        FIRST_MIS.store(0, Ordering::Relaxed);
        FINAL_RIS.store(0, Ordering::Relaxed);
        FINAL_MIS.store(0, Ordering::Relaxed);
        HANDLER_ROUTE.store(0, Ordering::Relaxed);
        BYTE_RSR.store(0, Ordering::Relaxed);
        STORM.store(0, Ordering::Relaxed);
    }

    fn publish(fields: [u32; 15]) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            for (index, value) in fields.into_iter().enumerate() {
                core::ptr::write_volatile(words.add(index + 1), value);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn read(offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((UART4_BASE + offset) as *const u32) }
    }

    fn write(offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((UART4_BASE + offset) as *mut u32, value);
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-nvic46-delivery"))]
mod uart5_local_nvic46_delivery {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rp1_hal::reset::{ResetController, UartReset};

    const MAGIC: u32 = u32::from_le_bytes(*b"U5I1");
    const PASS: u32 = 1;
    const FAIL_PREFLIGHT: u32 = 2;
    const FAIL_RESET: u32 = 3;
    const FAIL_SETUP: u32 = 4;
    const FAIL_SOURCE: u32 = 5;
    const FAIL_TIMEOUT: u32 = 6;
    const FAIL_IPSR: u32 = 7;
    const FAIL_STORM: u32 = 8;
    const FAIL_RESET_DONE: u32 = 9;
    const FAIL_IRQ46_RESTORE: u32 = 10;
    const FAIL_IRQ53_CHANGED: u32 = 11;

    const UART5_BASE: usize = 0x4004_4000;
    const UART_DR: usize = 0x00;
    const UART_RSR_ECR: usize = 0x04;
    const UART_FR: usize = 0x18;
    const UART_IBRD: usize = 0x24;
    const UART_FBRD: usize = 0x28;
    const UART_LCRH: usize = 0x2c;
    const UART_CR: usize = 0x30;
    const UART_IMSC: usize = 0x38;
    const UART_RIS: usize = 0x3c;
    const UART_MIS: usize = 0x40;
    const UART_ICR: usize = 0x44;
    const UART_PERIPH_ID0: usize = 0xfe0;
    const UART_CR_LOOPBACK: u32 = 1 | (1 << 7) | (1 << 8) | (1 << 9);
    const UART_INT_RX: u32 = 1 << 4;
    const UART_RXFE: u32 = 1 << 4;
    const TEST_BYTE: u32 = 0xa5;
    const POLL_LIMIT: usize = 100_000;
    const STABILITY_US: u64 = 4_000;
    const IRQ46_BIT1: u32 = 1 << 14;
    const IRQ53_BIT1: u32 = 1 << 21;
    const RESET_CTRL1: *const u32 = 0x4001_4004 as *const u32;
    const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
    const RESET_UART5: u32 = 1 << 31;
    const CLK_UART_CTRL: *mut u32 = 0x4001_8054 as *mut u32;
    const CLK_UART_DIV_INT: *const u32 = 0x4001_8058 as *const u32;
    const CLK_UART_SEL: *const u32 = 0x4001_8060 as *const u32;
    const PLL_SYS_CS: *const u32 = 0x4002_0000 as *const u32;
    const PLL_SYS_PRIM: *const u32 = 0x4002_0010 as *const u32;
    const CLK_UART_CTRL_RELEVANT: u32 = 0x0000_0fe0;
    const CLK_UART_SOURCE: u32 = 0x0000_0040;
    const CLK_UART_ENABLED: u32 = 0x0000_0840;
    const PLL_SYS_CS_EXPECTED: u32 = 0x8000_0001;
    const PLL_SYS_PRIM_PRI_PH: u32 = 1 << 4;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_RIS: AtomicU32 = AtomicU32::new(0);
    static FIRST_MIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_RIS: AtomicU32 = AtomicU32::new(0);
    static FINAL_MIS: AtomicU32 = AtomicU32::new(0);
    static HANDLER_ROUTE: AtomicU32 = AtomicU32::new(0);
    static BYTE_RSR: AtomicU32 = AtomicU32::new(0);
    static STORM: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn UART5_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Release);
        if old > 3 {
            STORM.store(1, Ordering::Relaxed);
        }
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_RIS.store(read(UART_RIS), Ordering::Relaxed);
            FIRST_MIS.store(read(UART_MIS), Ordering::Relaxed);
            HANDLER_ROUTE.store(
                pack_route(rp1_rt::uart5_irq_route_snapshot()),
                Ordering::Relaxed,
            );
            let byte = if read(UART_FR) & UART_RXFE == 0 {
                read(UART_DR) & 0xff
            } else {
                0x100
            };
            BYTE_RSR.store(
                byte | ((read(UART_RSR_ECR) & 0xff) << 16),
                Ordering::Relaxed,
            );
        }
        write(UART_IMSC, 0);
        write(UART_ICR, UART_INT_RX);
        FINAL_RIS.store(read(UART_RIS), Ordering::Relaxed);
        FINAL_MIS.store(read(UART_MIS), Ordering::Relaxed);
    }

    pub fn run_and_publish(resets: &mut ResetController) -> u32 {
        reset_atomics();
        let mut fields = [0u32; 15];
        let mut saved = None;
        let mut decision = PASS;
        let mut flags = 0u32;
        let mut armed_imsc = 0u32;
        let mut enabled_iser1 = 0u32;
        let mut source_ready = 0u32;
        let mut source_ispr1 = 0u32;
        let mut reset_attempted = false;
        let mut uart_touched = false;
        let clock_initial = read_clock_state();
        let mut clock_evidence = ClockEvidence::from_initial(clock_initial);
        let reset_ctrl_before = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_before = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        let reset_initial_asserted = reset_ctrl_before & RESET_UART5 != 0;
        let reset_initial_done = reset_done_before & RESET_UART5 != 0;
        let route_before = rp1_rt::uart5_irq_route_snapshot();
        let mut identity_ok = false;

        if !preflight_ok(route_before, reset_initial_asserted, reset_initial_done)
            || !prepare_clock(&mut clock_evidence)
        {
            decision = FAIL_PREFLIGHT;
        } else {
            reset_attempted = true;
            if !exercise_reset(resets, reset_initial_asserted) {
                decision = FAIL_RESET;
            } else {
                identity_ok = read(UART_PERIPH_ID0) & 0xff == 0x11;
                if !identity_ok {
                    decision = FAIL_PREFLIGHT;
                } else {
                    uart_touched = true;
                    setup_uart5();
                    if read(UART_CR) & UART_CR_LOOPBACK != UART_CR_LOOPBACK {
                        decision = FAIL_SETUP;
                    }
                }
            }
        }

        if decision == PASS {
            saved = unsafe { rp1_rt::prepare_uart5_irq() };
            if saved.is_none() {
                decision = FAIL_PREFLIGHT;
            }
        }
        if decision == PASS {
            write(UART_IMSC, UART_INT_RX);
            armed_imsc = read(UART_IMSC);
            write(UART_DR, TEST_BYTE);
            unsafe {
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
            let mut source_ris = 0;
            let mut source_mis = 0;
            let mut source_route = rp1_rt::uart5_irq_route_snapshot();
            for poll in 0..POLL_LIMIT {
                source_ris = read(UART_RIS);
                source_mis = read(UART_MIS);
                source_route = rp1_rt::uart5_irq_route_snapshot();
                if source_ris & UART_INT_RX != 0
                    && source_mis & UART_INT_RX != 0
                    && source_route.ispr1 & IRQ46_BIT1 != 0
                {
                    fields[8] = poll as u32;
                    break;
                }
            }
            source_ready = pack_source_ready(source_ris, source_mis, source_route);
            source_ispr1 = source_route.ispr1;
            if source_ris & UART_INT_RX == 0
                || source_mis & UART_INT_RX == 0
                || source_route.ispr1 & IRQ46_BIT1 == 0
                || source_route.iser1 & IRQ46_BIT1 != 0
            {
                decision = FAIL_SOURCE;
            } else {
                flags |= 1 << 1;
                unsafe {
                    rp1_rt::enable_uart5_irq_after_source_asserted();
                }
                enabled_iser1 = rp1_rt::uart5_irq_route_snapshot().iser1;
            }
        }

        if decision == PASS {
            for _ in 0..POLL_LIMIT {
                if COUNT.load(Ordering::Acquire) != 0 {
                    break;
                }
                core::hint::spin_loop();
            }
            if COUNT.load(Ordering::Acquire) == 0 {
                decision = FAIL_TIMEOUT;
            } else {
                super::busy_wait_us(STABILITY_US);
            }
        }

        cleanup_uart_irq(saved, uart_touched);
        let (final_ris, final_mis, final_imsc) = if uart_touched {
            (read(UART_RIS), read(UART_MIS), read(UART_IMSC))
        } else {
            (0, 0, 0)
        };
        restore_initial_reset(resets, reset_attempted, reset_initial_asserted);
        restore_clock(clock_initial, &mut clock_evidence);
        let route_final = rp1_rt::uart5_irq_route_snapshot();
        let reset_ctrl_final = unsafe { core::ptr::read_volatile(RESET_CTRL1) };
        let reset_done_final = unsafe { core::ptr::read_volatile(RESET_DONE1) };
        fields[2] = pack_event();
        fields[3] = (FIRST_RIS.load(Ordering::Relaxed) & 0xffff)
            | (FIRST_MIS.load(Ordering::Relaxed) << 16);
        fields[4] = (final_ris & 0xffff) | (final_mis << 16);
        fields[5] = (armed_imsc & 0xffff) | (final_imsc << 16);
        fields[6] = route_before.iser0;
        fields[7] = route_before.iser1;
        fields[8] = enabled_iser1;
        fields[9] = route_final.iser0;
        fields[10] = route_final.iser1;
        fields[11] = route_before.ispr1;
        fields[12] = source_ispr1;
        fields[13] = route_final.ispr1;
        fields[14] = pack_tail(
            reset_ctrl_before,
            reset_ctrl_final,
            reset_done_before,
            reset_done_final,
            source_ready,
            route_before,
            HANDLER_ROUTE.load(Ordering::Relaxed),
            route_final,
            clock_evidence,
        );

        let irq46_restored = route_final.iser1 == route_before.iser1
            && route_final.ispr1 & IRQ46_BIT1 == 0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask;
        let irq53_unchanged = route_final.iser1 & IRQ53_BIT1 == route_before.iser1 & IRQ53_BIT1
            && route_final.ispr1 & IRQ53_BIT1 == route_before.ispr1 & IRQ53_BIT1
            && route_final.iabr1 & IRQ53_BIT1 == route_before.iabr1 & IRQ53_BIT1
            && route_final.ispr1 & !IRQ46_BIT1 == route_before.ispr1 & !IRQ46_BIT1;
        let reset_restored = reset_state_matches(
            reset_ctrl_final & RESET_UART5 != 0,
            reset_done_final & RESET_UART5 != 0,
            reset_initial_asserted,
        );
        let no_storm = STORM.load(Ordering::Relaxed) == 0 && COUNT.load(Ordering::Acquire) == 1;
        let source_clean = final_imsc == 0 && final_mis == 0 && final_ris & UART_INT_RX == 0;
        let ipsr_ok = IPSR.load(Ordering::Relaxed) == rp1_rt::UART5_VECTOR_INDEX as u32;
        let byte_rsr = BYTE_RSR.load(Ordering::Relaxed);
        let byte_rsr_ok = byte_rsr & 0xff == TEST_BYTE && byte_rsr & 0x00ff_0000 == 0;
        let source_before_enable = source_ready & 0x0f == 0x07;
        let first_source_ok = FIRST_RIS.load(Ordering::Relaxed) & UART_INT_RX != 0
            && FIRST_MIS.load(Ordering::Relaxed) & UART_INT_RX != 0;
        let handler_active_ok = HANDLER_ROUTE.load(Ordering::Relaxed) & (1 << 2) != 0;
        let enable_ok = enabled_iser1 == IRQ46_BIT1;
        let final_exact = route_final.iser0 == route_before.iser0
            && route_final.iser1 == route_before.iser1
            && route_final.ispr0 == route_before.ispr0
            && route_final.ispr1 == route_before.ispr1
            && route_final.iabr0 == route_before.iabr0
            && route_final.iabr1 == route_before.iabr1
            && route_final.primask == route_before.primask
            && clock_evidence.final_ctrl_exact;

        flags |= (route_before.vtor == 0x2000_0000) as u32;
        flags |= (source_before_enable as u32) << 1;
        flags |= (ipsr_ok as u32) << 2;
        flags |= (no_storm as u32) << 3;
        flags |= (reset_restored as u32) << 4;
        flags |= (irq46_restored as u32) << 5;
        flags |= (irq53_unchanged as u32) << 6;
        flags |= (identity_ok as u32) << 7;
        flags |= (source_clean as u32) << 8;
        flags |= (byte_rsr_ok as u32) << 9;
        flags |= (enable_ok as u32) << 10;
        flags |= (first_source_ok as u32) << 11;
        flags |= (handler_active_ok as u32) << 12;
        flags |= (final_exact as u32) << 13;
        if flags & 0x3fff == 0x3fff && decision == PASS {
            flags |= 1 << 31;
        } else if decision == PASS && !ipsr_ok {
            decision = FAIL_IPSR;
        } else if decision == PASS && (!no_storm || !handler_active_ok) {
            decision = FAIL_STORM;
        } else if decision == PASS && !reset_restored {
            decision = FAIL_RESET_DONE;
        } else if decision == PASS && (!irq46_restored || !final_exact) {
            decision = FAIL_IRQ46_RESTORE;
        } else if decision == PASS && !irq53_unchanged {
            decision = FAIL_IRQ53_CHANGED;
        } else if decision == PASS && (!source_clean || !enable_ok || !first_source_ok) {
            decision = FAIL_SETUP;
        } else if decision == PASS && !byte_rsr_ok {
            decision = FAIL_SETUP;
        }

        fields[0] = decision;
        fields[1] = flags;
        publish(fields);
        decision
    }

    #[derive(Copy, Clone)]
    struct ClockState {
        ctrl: u32,
        div_int: u32,
        sel: u32,
        pll_cs: u32,
        pll_prim: u32,
    }

    #[derive(Copy, Clone)]
    struct ClockEvidence {
        initial_active: bool,
        initial_inactive_exact: bool,
        promoted: bool,
        source_rb_ok: bool,
        sel1: bool,
        enabled_rb_ok: bool,
        final_ctrl_exact: bool,
        div1: bool,
        pll_exact_lock: bool,
        pri_ph_bit4: bool,
    }

    impl ClockEvidence {
        const fn from_initial(state: ClockState) -> Self {
            let initial_active = state.ctrl & CLK_UART_CTRL_RELEVANT == CLK_UART_ENABLED;
            let initial_inactive_exact = state.ctrl == 0;
            let div1 = state.div_int == 1;
            let pll_exact_lock = state.pll_cs == PLL_SYS_CS_EXPECTED;
            let pri_ph_bit4 = state.pll_prim & PLL_SYS_PRIM_PRI_PH != 0;
            Self {
                initial_active,
                initial_inactive_exact,
                promoted: false,
                source_rb_ok: initial_active,
                sel1: state.sel == 1,
                enabled_rb_ok: initial_active,
                final_ctrl_exact: false,
                div1,
                pll_exact_lock,
                pri_ph_bit4,
            }
        }
    }

    fn read_clock_state() -> ClockState {
        unsafe {
            ClockState {
                ctrl: core::ptr::read_volatile(CLK_UART_CTRL),
                div_int: core::ptr::read_volatile(CLK_UART_DIV_INT),
                sel: core::ptr::read_volatile(CLK_UART_SEL),
                pll_cs: core::ptr::read_volatile(PLL_SYS_CS),
                pll_prim: core::ptr::read_volatile(PLL_SYS_PRIM),
            }
        }
    }

    fn prepare_clock(evidence: &mut ClockEvidence) -> bool {
        if !evidence.div1 || !evidence.pll_exact_lock || !evidence.pri_ph_bit4 {
            return false;
        }
        if evidence.initial_active {
            return evidence.sel1;
        }
        if !evidence.initial_inactive_exact {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_SOURCE);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.promoted = true;
        evidence.source_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_SOURCE;
        evidence.sel1 = unsafe { core::ptr::read_volatile(CLK_UART_SEL) } == 1;
        if !evidence.source_rb_ok || !evidence.sel1 {
            return false;
        }

        unsafe {
            core::ptr::write_volatile(CLK_UART_CTRL, CLK_UART_ENABLED);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        evidence.enabled_rb_ok =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } & CLK_UART_CTRL_RELEVANT
                == CLK_UART_ENABLED;
        evidence.enabled_rb_ok
    }

    fn restore_clock(initial: ClockState, evidence: &mut ClockEvidence) {
        if evidence.promoted {
            unsafe {
                core::ptr::write_volatile(CLK_UART_CTRL, initial.ctrl);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            }
        }
        evidence.final_ctrl_exact =
            unsafe { core::ptr::read_volatile(CLK_UART_CTRL) } == initial.ctrl;
    }

    fn setup_uart5() {
        write(UART_CR, 0);
        write(UART_IMSC, 0);
        write(UART_ICR, 0x7ff);
        write(UART_RSR_ECR, 0);
        write(UART_IBRD, 27);
        write(UART_FBRD, 8);
        write(UART_LCRH, 3 << 5);
        write(UART_CR, UART_CR_LOOPBACK);
        unsafe {
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn cleanup_uart_irq(saved: Option<rp1_rt::Uart5IrqSaved>, uart_touched: bool) {
        if uart_touched {
            write(UART_IMSC, 0);
            write(UART_ICR, UART_INT_RX);
            write(UART_CR, 0);
        }
        if let Some(saved) = saved {
            unsafe {
                rp1_rt::restore_uart5_irq(saved);
            }
        }
    }

    fn preflight_ok(
        route: rp1_rt::Uart5IrqRouteSnapshot,
        reset_asserted: bool,
        reset_done: bool,
    ) -> bool {
        let reset_coherent = reset_state_matches(reset_asserted, reset_done, reset_asserted);
        route.vtor == 0x2000_0000
            && route.primask == 0
            && route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & IRQ46_BIT1 == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
            && reset_coherent
    }

    const fn reset_state_matches(asserted: bool, done: bool, want_asserted: bool) -> bool {
        if want_asserted {
            asserted && !done
        } else {
            !asserted && done
        }
    }

    fn exercise_reset(resets: &mut ResetController, initially_asserted: bool) -> bool {
        if initially_asserted
            && resets
                .deassert_uart_clock_ready(UartReset::Uart5, POLL_LIMIT)
                .is_err()
        {
            return false;
        }
        resets
            .assert_uart_clock_ready(UartReset::Uart5, POLL_LIMIT)
            .is_ok()
            && resets
                .deassert_uart_clock_ready(UartReset::Uart5, POLL_LIMIT)
                .is_ok()
    }

    fn restore_initial_reset(
        resets: &mut ResetController,
        reset_attempted: bool,
        initially_asserted: bool,
    ) {
        if !reset_attempted {
            return;
        }
        let asserted = unsafe { core::ptr::read_volatile(RESET_CTRL1) } & RESET_UART5 != 0;
        let done = unsafe { core::ptr::read_volatile(RESET_DONE1) } & RESET_UART5 != 0;
        if reset_state_matches(asserted, done, initially_asserted) {
            return;
        }
        if initially_asserted && !asserted && done {
            let _ = resets.assert_uart_clock_ready(UartReset::Uart5, POLL_LIMIT);
        } else if !initially_asserted && asserted && !done {
            let _ = resets.deassert_uart_clock_ready(UartReset::Uart5, POLL_LIMIT);
        }
    }

    fn pack_route(route: rp1_rt::Uart5IrqRouteSnapshot) -> u32 {
        ((route.iser1 & IRQ46_BIT1 != 0) as u32)
            | (((route.ispr1 & IRQ46_BIT1 != 0) as u32) << 1)
            | (((route.iabr1 & IRQ46_BIT1 != 0) as u32) << 2)
            | ((route.primask & 1) << 3)
            | (((route.vtor == 0x2000_0000) as u32) << 4)
            | (((route.iser1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 10)
            | (((route.iser0 != 0) as u32) << 16)
            | (((route.ispr0 != 0) as u32) << 17)
            | (((route.iabr0 != 0) as u32) << 18)
    }

    fn pack_event() -> u32 {
        (IPSR.load(Ordering::Relaxed) & 0xff)
            | ((COUNT.load(Ordering::Acquire) & 0xff) << 8)
            | ((BYTE_RSR.load(Ordering::Relaxed) & 0xff) << 16)
            | (((BYTE_RSR.load(Ordering::Relaxed) >> 16) & 0xff) << 24)
    }

    fn pack_source_ready(ris: u32, mis: u32, route: rp1_rt::Uart5IrqRouteSnapshot) -> u32 {
        ((ris & UART_INT_RX != 0) as u32)
            | (((mis & UART_INT_RX != 0) as u32) << 1)
            | (((route.ispr1 & IRQ46_BIT1 != 0) as u32) << 2)
            | (((route.iser1 & IRQ46_BIT1 != 0) as u32) << 3)
            | (((route.ispr1 & IRQ53_BIT1 != 0) as u32) << 8)
            | (((route.iabr1 & IRQ53_BIT1 != 0) as u32) << 9)
            | ((ris & 0xff) << 16)
            | ((mis & 0xff) << 24)
    }

    fn pack_tail(
        reset_ctrl_before: u32,
        reset_ctrl_final: u32,
        reset_done_before: u32,
        reset_done_final: u32,
        source_ready: u32,
        route_before: rp1_rt::Uart5IrqRouteSnapshot,
        handler_route: u32,
        route_final: rp1_rt::Uart5IrqRouteSnapshot,
        clock: ClockEvidence,
    ) -> u32 {
        ((reset_ctrl_before & RESET_UART5 != 0) as u32)
            | (((reset_ctrl_final & RESET_UART5 != 0) as u32) << 1)
            | (((reset_done_before & RESET_UART5 != 0) as u32) << 2)
            | (((reset_done_final & RESET_UART5 != 0) as u32) << 3)
            | ((route_before.primask & 1) << 4)
            | ((route_final.primask & 1) << 5)
            | (((route_before.iabr1 & IRQ46_BIT1 != 0) as u32) << 8)
            | (((handler_route & (1 << 2) != 0) as u32) << 9)
            | (((route_final.iabr1 & IRQ46_BIT1 != 0) as u32) << 10)
            | ((source_ready & 0x03) << 16)
            | ((clock.initial_active as u32) << 20)
            | ((clock.initial_inactive_exact as u32) << 21)
            | ((clock.promoted as u32) << 22)
            | ((clock.source_rb_ok as u32) << 23)
            | ((clock.sel1 as u32) << 24)
            | ((clock.enabled_rb_ok as u32) << 25)
            | ((clock.final_ctrl_exact as u32) << 26)
            | ((clock.div1 as u32) << 27)
            | ((clock.pll_exact_lock as u32) << 28)
            | ((clock.pri_ph_bit4 as u32) << 29)
    }

    fn reset_atomics() {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_RIS.store(0, Ordering::Relaxed);
        FIRST_MIS.store(0, Ordering::Relaxed);
        FINAL_RIS.store(0, Ordering::Relaxed);
        FINAL_MIS.store(0, Ordering::Relaxed);
        HANDLER_ROUTE.store(0, Ordering::Relaxed);
        BYTE_RSR.store(0, Ordering::Relaxed);
        STORM.store(0, Ordering::Relaxed);
    }

    fn publish(fields: [u32; 15]) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            for (index, value) in fields.into_iter().enumerate() {
                core::ptr::write_volatile(words.add(index + 1), value);
            }
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn read(offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((UART5_BASE + offset) as *const u32) }
    }

    fn write(offset: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile((UART5_BASE + offset) as *mut u32, value);
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    feature = "pll-sys-core-lock-only",
    not(feature = "rp1-linux-clk-uart-ownership-conflict")
))]
fn publish_pll_sys_core_lock_result(result: PllSysCoreLockResult) {
    const MAGIC: u32 = 0x4b4c_4c50;
    const RESULT_WORDS: usize = 16;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        core::ptr::write_volatile(words.add(1), result.decision as u32);
        core::ptr::write_volatile(words.add(2), result.elapsed_us);
        core::ptr::write_volatile(words.add(3), result.first_cs);
        core::ptr::write_volatile(words.add(4), result.before.cs);
        core::ptr::write_volatile(words.add(5), result.before.pwr);
        core::ptr::write_volatile(words.add(6), result.before.fbdiv_int);
        core::ptr::write_volatile(words.add(7), result.before.fbdiv_frac);
        core::ptr::write_volatile(words.add(8), result.before.prim);
        core::ptr::write_volatile(words.add(9), result.before.sec);
        core::ptr::write_volatile(words.add(10), result.after.cs);
        core::ptr::write_volatile(words.add(11), result.after.pwr);
        core::ptr::write_volatile(words.add(12), result.after.fbdiv_int);
        core::ptr::write_volatile(words.add(13), result.after.fbdiv_frac);
        core::ptr::write_volatile(words.add(14), result.after.prim);
        core::ptr::write_volatile(words.add(15), result.after.sec);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(
    target_arch = "arm",
    feature = "pwm-gpio12-proof",
    not(feature = "pwm0-local-irq-proof")
))]
fn publish_pwm0_proof_result(decision: u32, low: Pwm0Snapshot, high: Pwm0Snapshot) {
    const MAGIC: u32 = 0x384d_5750;
    const RESULT_WORDS: usize = 31;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        core::ptr::write_volatile(words.add(1), decision);
        core::ptr::write_volatile(words.add(2), 50_000_000);
        core::ptr::write_volatile(words.add(3), 5_000_000);
        core::ptr::write_volatile(words.add(4), 1_250_000);
        core::ptr::write_volatile(words.add(5), 50_000);
        core::ptr::write_volatile(words.add(6), 37_500);
        let snapshots = [low, high];
        for (index, snapshot) in snapshots.into_iter().enumerate() {
            let base = 7 + index * 12;
            core::ptr::write_volatile(words.add(base), snapshot.clock_ctrl);
            core::ptr::write_volatile(words.add(base + 1), snapshot.clock_div_int);
            core::ptr::write_volatile(words.add(base + 2), snapshot.clock_div_frac);
            core::ptr::write_volatile(words.add(base + 3), snapshot.clock_sel);
            core::ptr::write_volatile(words.add(base + 4), snapshot.reset_ctrl1);
            core::ptr::write_volatile(words.add(base + 5), snapshot.reset_done1);
            core::ptr::write_volatile(words.add(base + 6), snapshot.gpio12_ctrl);
            core::ptr::write_volatile(words.add(base + 7), snapshot.gpio12_pad);
            core::ptr::write_volatile(words.add(base + 8), snapshot.global_ctrl);
            core::ptr::write_volatile(words.add(base + 9), snapshot.channel_ctrl);
            core::ptr::write_volatile(words.add(base + 10), snapshot.range);
            core::ptr::write_volatile(words.add(base + 11), snapshot.duty);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq-proof"))]
mod pwm0_local_irq_proof {
    use core::sync::atomic::{AtomicU32, Ordering};

    use rp1_hal::prelude::*;

    const MAGIC: u32 = 0x3151_5750; // PWQ1
    const PASS: u32 = 1;
    const FAIL_PREPARE: u32 = 0x101;
    const FAIL_START: u32 = 0x102;
    const FAIL_TIMEOUT: u32 = 0x103;
    const FAIL_FINAL: u32 = 0x104;
    const EXPECTED_COUNT: u32 = 4;
    const IRQ_TIMEOUT_US: u64 = 700_000;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static FIRST_INTR: AtomicU32 = AtomicU32::new(0);
    static FIRST_INTS: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn PWM0_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let (intr, ints) = rp1_hal::pwm::pwm0_raw_irq_status();
        if intr & rp1_hal::pwm::PWM0_CH0_RELOAD_IRQ != 0
            || ints & rp1_hal::pwm::PWM0_CH0_RELOAD_IRQ != 0
        {
            let old = COUNT.load(Ordering::Relaxed);
            COUNT.store(old.wrapping_add(1), Ordering::Relaxed);
            if old == 0 {
                FIRST_INTR.store(intr, Ordering::Relaxed);
                FIRST_INTS.store(ints, Ordering::Relaxed);
                IPSR.store(ipsr, Ordering::Relaxed);
            }
            rp1_hal::pwm::pwm0_ack_reload_interrupt();
        }
    }

    pub fn run(pwm0: &mut Pwm0) -> u32 {
        const CONFIG: Pwm0Config = Pwm0Config::new(50_000, 25_000);

        COUNT.store(0, Ordering::Relaxed);
        FIRST_INTR.store(0, Ordering::Relaxed);
        FIRST_INTS.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);

        rp1_hal::pwm::pwm0_ack_reload_interrupt();
        let route_before = rp1_rt::pwm0_irq_route_snapshot();
        if unsafe { !rp1_rt::prepare_pwm0_irq() } {
            publish(FAIL_PREPARE, route_before, route_before, route_before);
            return FAIL_PREPARE;
        }

        let mut channel = match pwm0.start_gpio12(CONFIG) {
            Ok(channel) => channel,
            Err(error) => {
                let route = rp1_rt::pwm0_irq_route_snapshot();
                publish(
                    FAIL_START | ((error as u32) << 16),
                    route_before,
                    route,
                    route,
                );
                return FAIL_START;
            }
        };

        channel.enable_reload_interrupt();
        unsafe {
            rp1_rt::enable_pwm0_irq();
        }
        let route_enabled = rp1_rt::pwm0_irq_route_snapshot();

        let start = super::raw_timer_us();
        while COUNT.load(Ordering::Relaxed) < EXPECTED_COUNT {
            if super::raw_timer_us().wrapping_sub(start) > IRQ_TIMEOUT_US {
                break;
            }
            core::hint::spin_loop();
        }

        channel.mask_reload_interrupt();
        rp1_hal::pwm::pwm0_ack_reload_interrupt();
        let count_after_mask = COUNT.load(Ordering::Relaxed);
        super::busy_wait_us(4_000);
        let count_stable = COUNT.load(Ordering::Relaxed) == count_after_mask;
        let stop_ok = channel.stop().is_ok();
        rp1_hal::pwm::pwm0_ack_reload_interrupt();
        unsafe {
            rp1_rt::disable_pwm0_irq();
        }
        let (_, final_ints) = rp1_hal::pwm::pwm0_raw_irq_status();
        let route_final = rp1_rt::pwm0_irq_route_snapshot();

        let final_ok = count_after_mask == EXPECTED_COUNT
            && count_stable
            && stop_ok
            && final_ints & rp1_hal::pwm::PWM0_CH0_RELOAD_IRQ == 0
            && route_final.iser0 & (1 << rp1_rt::PWM0_IRQ_NUMBER) == 0
            && route_final.ispr0 & (1 << rp1_rt::PWM0_IRQ_NUMBER) == 0
            && route_final.iabr0 & (1 << rp1_rt::PWM0_IRQ_NUMBER) == 0
            && IPSR.load(Ordering::Relaxed) == rp1_rt::PWM0_VECTOR_INDEX as u32;
        let decision = if count_after_mask < EXPECTED_COUNT {
            FAIL_TIMEOUT
        } else if final_ok {
            PASS
        } else {
            FAIL_FINAL
        };
        publish_with_final(
            decision,
            route_before,
            route_enabled,
            route_final,
            final_ints,
            count_after_mask,
            final_flags(count_stable, stop_ok),
        );
        decision
    }

    fn publish(
        decision: u32,
        before: rp1_rt::Pwm0IrqRouteSnapshot,
        enabled: rp1_rt::Pwm0IrqRouteSnapshot,
        final_route: rp1_rt::Pwm0IrqRouteSnapshot,
    ) {
        publish_with_final(
            decision,
            before,
            enabled,
            final_route,
            0,
            COUNT.load(Ordering::Relaxed),
            0,
        );
    }

    fn publish_with_final(
        decision: u32,
        before: rp1_rt::Pwm0IrqRouteSnapshot,
        enabled: rp1_rt::Pwm0IrqRouteSnapshot,
        final_route: rp1_rt::Pwm0IrqRouteSnapshot,
        final_ints: u32,
        count: u32,
        final_flags: u32,
    ) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            core::ptr::write_volatile(words.add(1), decision);
            core::ptr::write_volatile(words.add(2), rp1_rt::PWM0_IRQ_NUMBER as u32);
            core::ptr::write_volatile(words.add(3), rp1_rt::PWM0_VECTOR_INDEX as u32);
            core::ptr::write_volatile(words.add(4), count);
            core::ptr::write_volatile(words.add(5), IPSR.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(6), FIRST_INTR.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(7), FIRST_INTS.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(8), final_ints);
            core::ptr::write_volatile(words.add(9), before.vtor);
            core::ptr::write_volatile(words.add(10), before.iser0);
            core::ptr::write_volatile(words.add(11), enabled.iser0);
            core::ptr::write_volatile(words.add(12), final_route.iser0);
            core::ptr::write_volatile(words.add(13), final_route.ispr0);
            core::ptr::write_volatile(words.add(14), final_route.iabr0);
            core::ptr::write_volatile(words.add(15), final_flags);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    const fn final_flags(count_stable: bool, stop_ok: bool) -> u32 {
        count_stable as u32 | ((stop_ok as u32) << 1)
    }
}

#[cfg(all(target_arch = "arm", feature = "spi0-local-irq-proof"))]
mod spi0_local_irq_proof {
    use core::sync::atomic::{AtomicU32, Ordering};

    const MAGIC: u32 = u32::from_le_bytes(*b"S0Q1");
    const PASS: u32 = 1;
    const FAIL_PREPARE: u32 = 0x201;
    const FAIL_SPI: u32 = 0x202;
    const FAIL_PRECONDITION: u32 = 0x203;
    const FAIL_TIMEOUT: u32 = 0x204;
    const FAIL_FINAL: u32 = 0x205;
    const FAIL_ARM: u32 = 0x206;
    const TXEI: u32 = 1;
    const KNOWN_IRQ_SOURCES: u32 = 0x3f;
    const SPI0_IRQ_BIT: u32 = 1 << rp1_rt::SPI0_IRQ_NUMBER;
    const _: () = assert!(TXEI == 1);
    const _: () = assert!(KNOWN_IRQ_SOURCES == 0x3f);
    const _: () = assert!(TXEI & KNOWN_IRQ_SOURCES == TXEI);
    const IRQ_TIMEOUT_US: u64 = 100_000;

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_RISR: AtomicU32 = AtomicU32::new(0);
    static FIRST_ISR: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn SPI0_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let snapshot = rp1_hal::spi::spi0_irq_source_snapshot();
        rp1_hal::spi::spi0_mask_tx_empty_irq();
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Relaxed);
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_RISR.store(snapshot.raw_interrupt_status, Ordering::Relaxed);
            FIRST_ISR.store(snapshot.masked_interrupt_status, Ordering::Relaxed);
        }
    }

    pub fn run(spi0: &mut rp1_hal::spi::Spi0) -> u32 {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_RISR.store(0, Ordering::Relaxed);
        FIRST_ISR.store(0, Ordering::Relaxed);

        let route_before = rp1_rt::spi0_irq_route_snapshot();
        if unsafe { !rp1_rt::prepare_spi0_irq() } {
            publish(
                FAIL_PREPARE,
                route_before,
                route_before,
                route_before,
                route_before,
                None,
                None,
                0,
                0,
            );
            return FAIL_PREPARE;
        }

        let prepared = match spi0.prepare_tx_empty_irq() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let route = rp1_rt::spi0_irq_route_snapshot();
                publish(
                    FAIL_SPI | (spi_error_code(error) << 16),
                    route_before,
                    route,
                    route,
                    route,
                    None,
                    None,
                    0,
                    0,
                );
                cleanup();
                return FAIL_SPI;
            }
        };
        if prepared.interrupt_mask != 0
            || prepared.tx_fifo_threshold != 0
            || prepared.enable & 1 == 0
            || prepared.tx_fifo_level != 0
            || prepared.raw_interrupt_status & KNOWN_IRQ_SOURCES != TXEI
            || prepared.masked_interrupt_status & KNOWN_IRQ_SOURCES != 0
        {
            let route = rp1_rt::spi0_irq_route_snapshot();
            publish(
                FAIL_PRECONDITION,
                route_before,
                route,
                route,
                route,
                Some(prepared),
                None,
                0,
                0,
            );
            cleanup();
            return FAIL_PRECONDITION;
        }

        let armed = rp1_hal::spi::spi0_unmask_tx_empty_irq();
        if armed.interrupt_mask & KNOWN_IRQ_SOURCES != TXEI
            || armed.raw_interrupt_status & KNOWN_IRQ_SOURCES != TXEI
            || armed.masked_interrupt_status & KNOWN_IRQ_SOURCES != TXEI
        {
            let route = rp1_rt::spi0_irq_route_snapshot();
            publish(
                FAIL_ARM,
                route,
                route,
                route,
                route,
                Some(armed),
                None,
                0,
                0,
            );
            cleanup();
            return FAIL_ARM;
        }

        unsafe {
            rp1_rt::enable_spi0_irq();
        }
        let route_enabled = rp1_rt::spi0_irq_route_snapshot();

        let start = super::raw_timer_us();
        while COUNT.load(Ordering::Relaxed) == 0 {
            if super::raw_timer_us().wrapping_sub(start) > IRQ_TIMEOUT_US {
                break;
            }
            core::hint::spin_loop();
        }
        let route_wait = rp1_rt::spi0_irq_route_snapshot();
        let primask = primask();

        rp1_hal::spi::spi0_mask_tx_empty_irq();
        let count_after_mask = COUNT.load(Ordering::Relaxed);
        super::busy_wait_us(4_000);
        let count_stable = COUNT.load(Ordering::Relaxed) == count_after_mask;
        let final_snapshot = rp1_hal::spi::spi0_irq_snapshot();
        cleanup();
        let route_final = rp1_rt::spi0_irq_route_snapshot();

        let final_ok = count_after_mask == 1
            && count_stable
            && IPSR.load(Ordering::Relaxed) == rp1_rt::SPI0_VECTOR_INDEX as u32
            && FIRST_RISR.load(Ordering::Relaxed) & KNOWN_IRQ_SOURCES == TXEI
            && FIRST_ISR.load(Ordering::Relaxed) & KNOWN_IRQ_SOURCES == TXEI
            && final_snapshot.raw_interrupt_status & KNOWN_IRQ_SOURCES == TXEI
            && final_snapshot.masked_interrupt_status & KNOWN_IRQ_SOURCES == 0
            && final_snapshot.interrupt_mask == 0
            && route_final.iser0 & SPI0_IRQ_BIT == 0
            && route_final.ispr0 & SPI0_IRQ_BIT == 0
            && route_final.iabr0 & SPI0_IRQ_BIT == 0
            && primask == 0;
        let decision = if count_after_mask == 0 {
            FAIL_TIMEOUT
        } else if final_ok {
            PASS
        } else {
            FAIL_FINAL
        };
        publish(
            decision,
            route_before,
            route_enabled,
            route_wait,
            route_final,
            Some(armed),
            Some(final_snapshot),
            count_stable as u32,
            primask,
        );
        decision
    }

    fn cleanup() {
        rp1_hal::spi::spi0_cleanup_tx_empty_irq();
        unsafe {
            rp1_rt::disable_spi0_irq();
        }
    }

    fn publish(
        decision: u32,
        _before: rp1_rt::Spi0IrqRouteSnapshot,
        enabled: rp1_rt::Spi0IrqRouteSnapshot,
        wait: rp1_rt::Spi0IrqRouteSnapshot,
        final_route: rp1_rt::Spi0IrqRouteSnapshot,
        armed: Option<rp1_hal::spi::Spi0IrqSnapshot>,
        final_snapshot: Option<rp1_hal::spi::Spi0IrqSnapshot>,
        stable: u32,
        primask: u32,
    ) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let armed_state = armed.unwrap_or(rp1_hal::spi::Spi0IrqSnapshot {
            version: 0,
            enable: 0,
            tx_fifo_threshold: 0,
            interrupt_mask: 0,
            raw_interrupt_status: 0,
            masked_interrupt_status: 0,
            tx_fifo_level: 0,
            status: 0,
        });
        let final_state = final_snapshot.unwrap_or(armed_state);
        let final_imr_clear = (final_state.interrupt_mask == 0) as u32;
        let final_iser_clear = (final_route.iser0 & SPI0_IRQ_BIT == 0) as u32;
        let final_ispr_clear = (final_route.ispr0 & SPI0_IRQ_BIT == 0) as u32;
        let final_iabr_clear = (final_route.iabr0 & SPI0_IRQ_BIT == 0) as u32;
        let flags = final_imr_clear
            | (final_iser_clear << 1)
            | (final_ispr_clear << 2)
            | (final_iabr_clear << 3)
            | (stable << 4)
            | ((primask & 1) << 8)
            | ((final_imr_clear & final_iser_clear & final_ispr_clear & final_iabr_clear & stable)
                << 31);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            core::ptr::write_volatile(words.add(1), decision);
            core::ptr::write_volatile(words.add(2), COUNT.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(3), IPSR.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(4), armed_state.interrupt_mask);
            core::ptr::write_volatile(words.add(5), armed_state.raw_interrupt_status);
            core::ptr::write_volatile(words.add(6), armed_state.masked_interrupt_status);
            core::ptr::write_volatile(words.add(7), FIRST_RISR.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(8), FIRST_ISR.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(9), enabled.iser0);
            core::ptr::write_volatile(words.add(10), wait.iser0);
            core::ptr::write_volatile(words.add(11), wait.ispr0);
            core::ptr::write_volatile(words.add(12), wait.iabr0);
            core::ptr::write_volatile(words.add(13), final_state.raw_interrupt_status);
            core::ptr::write_volatile(words.add(14), final_state.masked_interrupt_status);
            core::ptr::write_volatile(words.add(15), flags);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn primask() -> u32 {
        let value: u32;
        unsafe {
            core::arch::asm!("mrs {}, PRIMASK", out(reg) value, options(nomem, nostack, preserves_flags));
        }
        value
    }

    const fn spi_error_code(error: rp1_hal::spi::Spi0Error) -> u32 {
        match error {
            rp1_hal::spi::Spi0Error::Version(_) => 1,
            rp1_hal::spi::Spi0Error::DisableTimeout => 2,
            rp1_hal::spi::Spi0Error::EnableTimeout => 3,
            rp1_hal::spi::Spi0Error::FifoDepthUnknown => 4,
            rp1_hal::spi::Spi0Error::EmptyPayload => 5,
            rp1_hal::spi::Spi0Error::PayloadTooLong { .. } => 6,
            rp1_hal::spi::Spi0Error::TxFifoTimeout => 7,
            rp1_hal::spi::Spi0Error::TransferTimeout => 8,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "spi0-local-irq-bank1-passive-scout"))]
mod spi0_local_irq_bank1_passive_scout {
    const MAGIC: u32 = u32::from_le_bytes(*b"S0P1");
    const PASS: u32 = 1;
    const FAIL_SPI: u32 = 0x222;
    const FAIL_PRECONDITION: u32 = 0x223;
    const FAIL_ARM: u32 = 0x226;
    const FAIL_FINAL: u32 = 0x225;
    const TXEI: u32 = 1;
    const KNOWN_IRQ_SOURCES: u32 = 0x3f;
    const ALLOWED_PREEXISTING_ISPR1_MASK: u32 = 1 << 21;
    const OBSERVE_US: u64 = 4_000;

    pub fn run(spi0: &mut rp1_hal::spi::Spi0) -> u32 {
        let before = rp1_rt::spi0_irq_route_snapshot();
        if !route_idle(before) {
            publish(
                FAIL_PRECONDITION,
                before,
                before,
                before,
                None,
                None,
                None,
                None,
                0,
            );
            return FAIL_PRECONDITION;
        }
        let prepared = match spi0.prepare_tx_empty_irq() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let final_route = cleanup();
                publish(
                    FAIL_SPI | (spi_error_code(error) << 16),
                    before,
                    before,
                    final_route,
                    None,
                    None,
                    None,
                    None,
                    0,
                );
                return FAIL_SPI;
            }
        };
        if prepared.interrupt_mask != 0
            || prepared.raw_interrupt_status & KNOWN_IRQ_SOURCES != TXEI
            || prepared.masked_interrupt_status & KNOWN_IRQ_SOURCES != 0
        {
            let final_route = cleanup();
            publish(
                FAIL_PRECONDITION,
                before,
                before,
                final_route,
                Some(prepared),
                None,
                None,
                None,
                0,
            );
            return FAIL_PRECONDITION;
        }

        let armed = rp1_hal::spi::spi0_unmask_tx_empty_irq();
        let armed_route = rp1_rt::spi0_irq_route_snapshot();
        if armed.interrupt_mask & KNOWN_IRQ_SOURCES != TXEI
            || armed.raw_interrupt_status & KNOWN_IRQ_SOURCES != TXEI
            || armed.masked_interrupt_status & KNOWN_IRQ_SOURCES != TXEI
        {
            let final_route = cleanup();
            publish(
                FAIL_ARM,
                before,
                armed_route,
                final_route,
                Some(prepared),
                Some(armed),
                None,
                None,
                0,
            );
            return FAIL_ARM;
        }

        super::busy_wait_us(OBSERVE_US);
        let observed = rp1_hal::spi::spi0_irq_snapshot();
        let observed_route = rp1_rt::spi0_irq_route_snapshot();
        let final_route = cleanup();
        let final_snapshot = rp1_hal::spi::spi0_irq_snapshot();
        let pass = final_snapshot.interrupt_mask == 0
            && final_snapshot.masked_interrupt_status & KNOWN_IRQ_SOURCES == 0;
        let decision = if pass { PASS } else { FAIL_FINAL };
        publish(
            decision,
            before,
            observed_route,
            final_route,
            Some(prepared),
            Some(armed),
            Some(observed),
            Some(final_snapshot),
            pass as u32,
        );
        decision
    }

    fn cleanup() -> rp1_rt::Spi0IrqRouteSnapshot {
        rp1_hal::spi::spi0_cleanup_tx_empty_irq();
        rp1_rt::spi0_irq_route_snapshot()
    }

    fn route_idle(route: rp1_rt::Spi0IrqRouteSnapshot) -> bool {
        route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & !ALLOWED_PREEXISTING_ISPR1_MASK == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
    }

    fn publish(
        decision: u32,
        before: rp1_rt::Spi0IrqRouteSnapshot,
        during: rp1_rt::Spi0IrqRouteSnapshot,
        final_route: rp1_rt::Spi0IrqRouteSnapshot,
        prepared: Option<rp1_hal::spi::Spi0IrqSnapshot>,
        armed: Option<rp1_hal::spi::Spi0IrqSnapshot>,
        observed: Option<rp1_hal::spi::Spi0IrqSnapshot>,
        final_snapshot: Option<rp1_hal::spi::Spi0IrqSnapshot>,
        flags: u32,
    ) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let prepared = prepared.unwrap_or(empty_spi());
        let armed = armed.unwrap_or(empty_spi());
        let observed = observed.unwrap_or(empty_spi());
        let final_snapshot = final_snapshot.unwrap_or(empty_spi());
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        let iabr_or = before.iabr0
            | before.iabr1
            | during.iabr0
            | during.iabr1
            | final_route.iabr0
            | final_route.iabr1;
        unsafe {
            core::ptr::write_volatile(words, 0);
            core::ptr::write_volatile(
                words.add(1),
                decision | (flags << 16) | ((decision == PASS) as u32) << 31,
            );
            core::ptr::write_volatile(words.add(2), before.iser0);
            core::ptr::write_volatile(words.add(3), before.iser1);
            core::ptr::write_volatile(words.add(4), during.iser0);
            core::ptr::write_volatile(words.add(5), during.iser1);
            core::ptr::write_volatile(words.add(6), final_route.iser0);
            core::ptr::write_volatile(words.add(7), final_route.iser1);
            core::ptr::write_volatile(words.add(8), before.ispr0);
            core::ptr::write_volatile(words.add(9), before.ispr1);
            core::ptr::write_volatile(words.add(10), during.ispr0);
            core::ptr::write_volatile(words.add(11), during.ispr1);
            core::ptr::write_volatile(words.add(12), final_route.ispr0);
            core::ptr::write_volatile(words.add(13), final_route.ispr1);
            core::ptr::write_volatile(
                words.add(14),
                pack_spi(prepared, 0)
                    | pack_spi(armed, 3)
                    | pack_spi(observed, 6)
                    | pack_spi(final_snapshot, 9),
            );
            core::ptr::write_volatile(words.add(15), iabr_or);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn pack_spi(snapshot: rp1_hal::spi::Spi0IrqSnapshot, shift: u32) -> u32 {
        (((snapshot.interrupt_mask & TXEI != 0) as u32)
            | (((snapshot.raw_interrupt_status & TXEI != 0) as u32) << 1)
            | (((snapshot.masked_interrupt_status & TXEI != 0) as u32) << 2))
            << shift
    }

    const fn empty_spi() -> rp1_hal::spi::Spi0IrqSnapshot {
        rp1_hal::spi::Spi0IrqSnapshot {
            version: 0,
            enable: 0,
            tx_fifo_threshold: 0,
            interrupt_mask: 0,
            raw_interrupt_status: 0,
            masked_interrupt_status: 0,
            tx_fifo_level: 0,
            status: 0,
        }
    }

    const fn spi_error_code(error: rp1_hal::spi::Spi0Error) -> u32 {
        match error {
            rp1_hal::spi::Spi0Error::Version(_) => 1,
            rp1_hal::spi::Spi0Error::DisableTimeout => 2,
            rp1_hal::spi::Spi0Error::EnableTimeout => 3,
            rp1_hal::spi::Spi0Error::FifoDepthUnknown => 4,
            rp1_hal::spi::Spi0Error::EmptyPayload => 5,
            rp1_hal::spi::Spi0Error::PayloadTooLong { .. } => 6,
            rp1_hal::spi::Spi0Error::TxFifoTimeout => 7,
            rp1_hal::spi::Spi0Error::TransferTimeout => 8,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq-proof"))]
mod i2c1_local_irq_proof {
    use core::sync::atomic::{AtomicU32, Ordering};

    const MAGIC: u32 = u32::from_le_bytes(*b"I1Q1");
    const PASS: u32 = 1;
    const FAIL_PREPARE: u32 = 0x301;
    const FAIL_I2C: u32 = 0x302;
    const FAIL_TIMEOUT: u32 = 0x304;
    const FAIL_FINAL: u32 = 0x305;
    const FAIL_ARM: u32 = 0x306;
    const ADDRESS: u8 = 0x2d;
    const STOP_DET: u32 = 1 << 9;
    const TX_ABRT: u32 = 1 << 6;
    const I2C1_IRQ_BIT: u32 = 1 << rp1_rt::I2C1_IRQ_NUMBER;
    const IRQ_TIMEOUT_US: u64 = 100_000;
    const PACKET: [u8; 20] = [
        0x44, 0x31, 0x44, 0x52, 0x01, 0x49, 0x01, 0x09, 0xdf, 0x9b, 0x57, 0x13, 0xe0,
        0xac, 0x68, 0x24, 0x31, 0x43, 0x32, 0x49,
    ];

    static COUNT: AtomicU32 = AtomicU32::new(0);
    static IPSR: AtomicU32 = AtomicU32::new(0);
    static FIRST_PACK: AtomicU32 = AtomicU32::new(0);
    static FIRST_ABRT_SOURCE: AtomicU32 = AtomicU32::new(0);

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn I2C1_IRQHandler() {
        let ipsr: u32;
        unsafe {
            core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        }
        let snapshot = rp1_hal::i2c::i2c1_irq_snapshot();
        rp1_hal::i2c::i2c1_mask_stop_det_irq();
        rp1_hal::i2c::i2c1_ack_stop_det_irq(snapshot);
        let old = COUNT.load(Ordering::Relaxed);
        COUNT.store(old.wrapping_add(1), Ordering::Relaxed);
        if old == 0 {
            IPSR.store(ipsr, Ordering::Relaxed);
            FIRST_PACK.store(pack_i2c(snapshot), Ordering::Relaxed);
            FIRST_ABRT_SOURCE.store(snapshot.abort_source, Ordering::Relaxed);
        }
    }

    pub fn publish_setup_error(decision: u32) {
        let route = rp1_rt::i2c1_irq_route_snapshot();
        publish(decision, 0, 0, 0, route, route, 0, 0, 0, 0, 0, 0, 0);
    }

    pub fn run(i2c1: &mut rp1_hal::i2c::I2c1Host) -> u32 {
        COUNT.store(0, Ordering::Relaxed);
        IPSR.store(0, Ordering::Relaxed);
        FIRST_PACK.store(0, Ordering::Relaxed);
        FIRST_ABRT_SOURCE.store(0, Ordering::Relaxed);

        let route_before = rp1_rt::i2c1_irq_route_snapshot();
        if unsafe { !rp1_rt::prepare_i2c1_irq() } {
            publish(
                FAIL_PREPARE,
                0,
                0,
                0,
                route_before,
                route_before,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            );
            return FAIL_PREPARE;
        }

        let armed = match i2c1.arm_stop_det_irq(ADDRESS) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let route = rp1_rt::i2c1_irq_route_snapshot();
                publish(
                    FAIL_I2C | (i2c_error_code(error) << 16),
                    0,
                    0,
                    0,
                    route,
                    route,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                );
                cleanup();
                return FAIL_I2C;
            }
        };
        let armed_pack = pack_i2c(armed);
        let armed_ok = armed.interrupt_mask & STOP_DET != 0
            && armed.interrupt_mask & TX_ABRT == 0
            && armed.raw_interrupt_status & (STOP_DET | TX_ABRT) == 0
            && armed.masked_interrupt_status & (STOP_DET | TX_ABRT) == 0
            && armed.enable_status & 1 != 0;
        if !armed_ok {
            let route = rp1_rt::i2c1_irq_route_snapshot();
            publish(FAIL_ARM, armed_pack, 0, 0, route, route, 0, 0, 0, 0, 0, 0, 0);
            cleanup();
            return FAIL_ARM;
        }

        unsafe {
            rp1_rt::enable_i2c1_irq();
        }
        let route_enabled = rp1_rt::i2c1_irq_route_snapshot();

        let queued = match i2c1.start_write(&PACKET) {
            Ok(queued) => queued,
            Err(error) => {
                let route = rp1_rt::i2c1_irq_route_snapshot();
                publish(
                    FAIL_I2C | (i2c_error_code(error) << 16),
                    armed_pack,
                    FIRST_PACK.load(Ordering::Relaxed),
                    FIRST_ABRT_SOURCE.load(Ordering::Relaxed),
                    route_enabled,
                    route,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                );
                cleanup();
                return FAIL_I2C;
            }
        };
        let witness = (u32::from(ADDRESS) & 0x7f)
            | (u32::from(queued.bytes_queued) << 8)
            | ((queued.last_command & 0x3ff) << 16)
            | (1 << 31);

        let start = super::raw_timer_us();
        loop {
            if COUNT.load(Ordering::Relaxed) != 0 {
                break;
            }
            let snapshot = rp1_hal::i2c::i2c1_irq_snapshot();
            if snapshot.raw_interrupt_status & TX_ABRT != 0 || snapshot.abort_source != 0 {
                break;
            }
            if super::raw_timer_us().wrapping_sub(start) > IRQ_TIMEOUT_US {
                break;
            }
            core::hint::spin_loop();
        }
        let route_wait = rp1_rt::i2c1_irq_route_snapshot();
        let terminal = rp1_hal::i2c::i2c1_irq_snapshot();
        let terminal_pack = pack_i2c(terminal);
        rp1_hal::i2c::i2c1_mask_stop_det_irq();
        let count_at_mask = COUNT.load(Ordering::Relaxed);
        super::busy_wait_us(4_000);
        let final_count = COUNT.load(Ordering::Relaxed);
        let count_stable = final_count == count_at_mask;
        let cleanup_i2c = rp1_hal::i2c::i2c1_cleanup_stop_det_irq();
        let cleanup_i2c_pack = pack_i2c(cleanup_i2c);
        unsafe {
            rp1_rt::disable_i2c1_irq();
        }
        let cleanup_route = rp1_rt::i2c1_irq_route_snapshot();
        let cleanup_route_pack = pack_route(cleanup_route);

        let first_pack = FIRST_PACK.load(Ordering::Relaxed);
        let first_abrt = FIRST_ABRT_SOURCE.load(Ordering::Relaxed);
        let first_stop = final_count != 0 && first_pack & ((1 << 4) | (1 << 8)) == (1 << 4) | (1 << 8);
        let no_abrt = first_pack & ((1 << 5) | (1 << 9)) == 0
            && terminal_pack & ((1 << 5) | (1 << 9)) == 0
            && first_abrt == 0
            && terminal.abort_source == 0;
        let terminal_stop = terminal_pack & ((1 << 4) | (1 << 8)) == (1 << 4) | (1 << 8);
        let cleanup_i2c_ok = cleanup_i2c_pack & ((1 << 4) | (1 << 5) | (1 << 8) | (1 << 9)) == 0
            && cleanup_i2c.enable_status & 1 != 0;
        let cleanup_route_ok = cleanup_route.iser0 & I2C1_IRQ_BIT == 0
            && cleanup_route.ispr0 & I2C1_IRQ_BIT == 0
            && cleanup_route.iabr0 & I2C1_IRQ_BIT == 0
            && cleanup_route.vtor == 0x2000_0000
            && cleanup_route_pack & ((1 << 8) | (1 << 9) | (1 << 10) | (1 << 11)) == 0;
        let wait_primask0 = route_wait.primask & 1 == 0;
        let expected_witness =
            0x8000_0000 | ((u32::from(PACKET[19]) | (1 << 9)) << 16) | (20 << 8) | u32::from(ADDRESS);
        let witness_ok = witness == expected_witness;
        let pass = armed_ok
            && witness_ok
            && first_stop
            && no_abrt
            && cleanup_i2c_ok
            && cleanup_route_ok
            && wait_primask0
            && count_stable
            && final_count == 1
            && IPSR.load(Ordering::Relaxed) == rp1_rt::I2C1_VECTOR_INDEX as u32;
        let decision = if final_count == 0 {
            FAIL_TIMEOUT
        } else if pass {
            PASS
        } else {
            FAIL_FINAL
        };
        let mut flags = 0;
        flags |= (armed_ok as u32) << 0;
        flags |= (witness_ok as u32) << 1;
        flags |= (first_stop as u32) << 2;
        flags |= (no_abrt as u32) << 3;
        flags |= (terminal_stop as u32) << 4;
        flags |= (cleanup_i2c_ok as u32) << 5;
        flags |= (cleanup_route_ok as u32) << 6;
        flags |= (wait_primask0 as u32) << 7;
        flags |= (count_stable as u32) << 8;
        flags |= (pass as u32) << 31;
        publish(
            decision,
            armed_pack,
            first_pack,
            first_abrt,
            route_enabled,
            route_wait,
            terminal_pack,
            terminal.abort_source,
            witness,
            cleanup_i2c_pack,
            cleanup_route_pack,
            count_at_mask,
            flags,
        );
        decision
    }

    fn cleanup() {
        rp1_hal::i2c::i2c1_cleanup_stop_det_irq();
        unsafe {
            rp1_rt::disable_i2c1_irq();
        }
    }

    fn publish(
        decision: u32,
        armed_i2c_pack: u32,
        first_i2c_pack: u32,
        first_abrt_source: u32,
        route_enabled: rp1_rt::I2c1IrqRouteSnapshot,
        route_wait: rp1_rt::I2c1IrqRouteSnapshot,
        terminal_i2c_pack: u32,
        terminal_abrt_source: u32,
        transfer_witness_pack: u32,
        cleanup_i2c_pack: u32,
        cleanup_route_pack: u32,
        count_at_mask: u32,
        flags: u32,
    ) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        unsafe {
            core::ptr::write_volatile(words, 0);
            core::ptr::write_volatile(words.add(1), decision);
            core::ptr::write_volatile(words.add(2), COUNT.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(3), IPSR.load(Ordering::Relaxed));
            core::ptr::write_volatile(words.add(4), armed_i2c_pack);
            core::ptr::write_volatile(words.add(5), first_i2c_pack);
            core::ptr::write_volatile(words.add(6), first_abrt_source);
            core::ptr::write_volatile(words.add(7), pack_route(route_enabled));
            core::ptr::write_volatile(words.add(8), pack_route(route_wait));
            core::ptr::write_volatile(words.add(9), terminal_i2c_pack);
            core::ptr::write_volatile(words.add(10), terminal_abrt_source);
            core::ptr::write_volatile(words.add(11), transfer_witness_pack);
            core::ptr::write_volatile(words.add(12), cleanup_i2c_pack);
            core::ptr::write_volatile(words.add(13), cleanup_route_pack);
            core::ptr::write_volatile(words.add(14), count_at_mask);
            core::ptr::write_volatile(words.add(15), flags);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn pack_i2c(snapshot: rp1_hal::i2c::I2c1IrqSnapshot) -> u32 {
        let known = STOP_DET | TX_ABRT;
        ((snapshot.interrupt_mask & STOP_DET != 0) as u32)
            | (((snapshot.interrupt_mask & TX_ABRT != 0) as u32) << 1)
            | (((snapshot.raw_interrupt_status & STOP_DET != 0) as u32) << 4)
            | (((snapshot.raw_interrupt_status & TX_ABRT != 0) as u32) << 5)
            | (((snapshot.masked_interrupt_status & STOP_DET != 0) as u32) << 8)
            | (((snapshot.masked_interrupt_status & TX_ABRT != 0) as u32) << 9)
            | (((snapshot.enable_status & 1 != 0) as u32) << 12)
            | (((snapshot.interrupt_mask & !known != 0) as u32) << 16)
            | (((snapshot.raw_interrupt_status & !known != 0) as u32) << 17)
            | (((snapshot.masked_interrupt_status & !known != 0) as u32) << 18)
    }

    fn pack_route(route: rp1_rt::I2c1IrqRouteSnapshot) -> u32 {
        ((route.iser0 & I2C1_IRQ_BIT != 0) as u32)
            | (((route.ispr0 & I2C1_IRQ_BIT != 0) as u32) << 1)
            | (((route.iabr0 & I2C1_IRQ_BIT != 0) as u32) << 2)
            | (((route.primask & 1 != 0) as u32) << 3)
            | (((route.vtor == 0x2000_0000) as u32) << 4)
            | (((route.iser0 & !I2C1_IRQ_BIT != 0) as u32) << 8)
            | (((route.iser1 != 0) as u32) << 9)
            | (((route.ispr0 & !I2C1_IRQ_BIT != 0) as u32) << 10)
            | (((route.iabr0 & !I2C1_IRQ_BIT != 0) as u32) << 11)
    }

    const fn i2c_error_code(error: rp1_hal::i2c::I2c1Error) -> u32 {
        match error {
            rp1_hal::i2c::I2c1Error::ComponentType(_) => 1,
            rp1_hal::i2c::I2c1Error::FifoTooShallow(_) => 2,
            rp1_hal::i2c::I2c1Error::DisableTimeout => 3,
            rp1_hal::i2c::I2c1Error::EnableTimeout => 4,
            rp1_hal::i2c::I2c1Error::InvalidAddress(_) => 5,
            rp1_hal::i2c::I2c1Error::EmptyPayload => 6,
            rp1_hal::i2c::I2c1Error::PayloadTooLong { .. } => 7,
            rp1_hal::i2c::I2c1Error::TxFifoTimeout => 8,
            rp1_hal::i2c::I2c1Error::TxAbort(_) => 9,
            rp1_hal::i2c::I2c1Error::StopTimeout => 10,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq-bank1-passive-scout"))]
mod i2c1_local_irq_bank1_passive_scout {
    const MAGIC: u32 = u32::from_le_bytes(*b"I1P1");
    const PASS: u32 = 1;
    const FAIL_I2C: u32 = 0x322;
    const FAIL_PRECONDITION: u32 = 0x323;
    const FAIL_ARM: u32 = 0x326;
    const FAIL_TIMEOUT: u32 = 0x324;
    const FAIL_FINAL: u32 = 0x325;
    const ADDRESS: u8 = 0x2d;
    const STOP_DET: u32 = 1 << 9;
    const TX_ABRT: u32 = 1 << 6;
    const ALLOWED_PREEXISTING_ISPR1_MASK: u32 = 1 << 21;
    const IRQ_TIMEOUT_US: u64 = 100_000;
    const PACKET: [u8; 20] = [
        0x44, 0x31, 0x44, 0x52, 0x01, 0x49, 0x01, 0x09, 0xdf, 0x9b, 0x57, 0x13, 0xe0, 0xac, 0x68,
        0x24, 0x31, 0x43, 0x32, 0x49,
    ];

    pub fn publish_setup_error(decision: u32) {
        let route = rp1_rt::i2c1_irq_route_snapshot();
        publish(decision, route, route, route, None, None, None, 0, 0);
    }

    pub fn run(i2c1: &mut rp1_hal::i2c::I2c1Host) -> u32 {
        let before = rp1_rt::i2c1_irq_route_snapshot();
        if !route_idle(before) {
            publish(
                FAIL_PRECONDITION,
                before,
                before,
                before,
                None,
                None,
                None,
                0,
                0,
            );
            return FAIL_PRECONDITION;
        }
        let expected_witness = 0x8000_0000 | (20 << 8) | u32::from(ADDRESS);
        let armed = match i2c1.arm_stop_det_irq(ADDRESS) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let final_route = cleanup();
                publish(
                    FAIL_I2C | (i2c_error_code(error) << 16),
                    before,
                    before,
                    final_route,
                    None,
                    None,
                    None,
                    0,
                    0,
                );
                return FAIL_I2C;
            }
        };
        if armed.interrupt_mask & STOP_DET == 0
            || armed.raw_interrupt_status & (STOP_DET | TX_ABRT) != 0
            || armed.masked_interrupt_status & (STOP_DET | TX_ABRT) != 0
            || armed.enable_status & 1 == 0
        {
            let final_route = cleanup();
            publish(
                FAIL_ARM,
                before,
                before,
                final_route,
                Some(armed),
                None,
                None,
                0,
                0,
            );
            return FAIL_ARM;
        }

        let queued = match i2c1.start_write(&PACKET) {
            Ok(queued) => queued,
            Err(error) => {
                let final_route = cleanup();
                publish(
                    FAIL_I2C | (i2c_error_code(error) << 16),
                    before,
                    before,
                    final_route,
                    Some(armed),
                    None,
                    None,
                    0,
                    0,
                );
                return FAIL_I2C;
            }
        };
        let witness = 0x8000_0000 | (u32::from(queued.bytes_queued) << 8) | u32::from(ADDRESS);

        let start = super::raw_timer_us();
        let observed = loop {
            let snapshot = rp1_hal::i2c::i2c1_irq_snapshot();
            if snapshot.raw_interrupt_status & (STOP_DET | TX_ABRT) != 0
                || snapshot.abort_source != 0
            {
                break snapshot;
            }
            if super::raw_timer_us().wrapping_sub(start) > IRQ_TIMEOUT_US {
                break snapshot;
            }
            core::hint::spin_loop();
        };
        let observed_route = rp1_rt::i2c1_irq_route_snapshot();
        let final_snapshot = rp1_hal::i2c::i2c1_cleanup_stop_det_irq();
        let final_route = rp1_rt::i2c1_irq_route_snapshot();
        let saw_stop = observed.raw_interrupt_status & STOP_DET != 0
            || observed.masked_interrupt_status & STOP_DET != 0;
        let final_clean = final_snapshot.interrupt_mask & STOP_DET == 0
            && final_snapshot.raw_interrupt_status & (STOP_DET | TX_ABRT) == 0
            && final_snapshot.masked_interrupt_status & (STOP_DET | TX_ABRT) == 0;
        let pass = saw_stop && final_clean && witness == expected_witness;
        let decision = if !saw_stop {
            FAIL_TIMEOUT
        } else if pass {
            PASS
        } else {
            FAIL_FINAL
        };
        publish(
            decision,
            before,
            observed_route,
            final_route,
            Some(armed),
            Some(observed),
            Some(final_snapshot),
            witness,
            pass as u32,
        );
        decision
    }

    fn cleanup() -> rp1_rt::I2c1IrqRouteSnapshot {
        rp1_hal::i2c::i2c1_cleanup_stop_det_irq();
        rp1_rt::i2c1_irq_route_snapshot()
    }

    fn route_idle(route: rp1_rt::I2c1IrqRouteSnapshot) -> bool {
        route.iser0 == 0
            && route.iser1 == 0
            && route.ispr0 == 0
            && route.ispr1 & !ALLOWED_PREEXISTING_ISPR1_MASK == 0
            && route.iabr0 == 0
            && route.iabr1 == 0
    }

    fn publish(
        decision: u32,
        before: rp1_rt::I2c1IrqRouteSnapshot,
        during: rp1_rt::I2c1IrqRouteSnapshot,
        final_route: rp1_rt::I2c1IrqRouteSnapshot,
        armed: Option<rp1_hal::i2c::I2c1IrqSnapshot>,
        observed: Option<rp1_hal::i2c::I2c1IrqSnapshot>,
        final_snapshot: Option<rp1_hal::i2c::I2c1IrqSnapshot>,
        witness: u32,
        flags: u32,
    ) {
        const WORDS: usize = 16;
        const _: () = assert!(WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
        let armed = armed.unwrap_or(empty_i2c());
        let observed = observed.unwrap_or(empty_i2c());
        let final_snapshot = final_snapshot.unwrap_or(empty_i2c());
        let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
        let iabr_or = before.iabr0
            | before.iabr1
            | during.iabr0
            | during.iabr1
            | final_route.iabr0
            | final_route.iabr1;
        unsafe {
            core::ptr::write_volatile(words, 0);
            core::ptr::write_volatile(
                words.add(1),
                decision | (flags << 16) | ((decision == PASS) as u32) << 31,
            );
            core::ptr::write_volatile(words.add(2), before.iser0);
            core::ptr::write_volatile(words.add(3), before.iser1);
            core::ptr::write_volatile(words.add(4), during.iser0);
            core::ptr::write_volatile(words.add(5), during.iser1);
            core::ptr::write_volatile(words.add(6), final_route.iser0);
            core::ptr::write_volatile(words.add(7), final_route.iser1);
            core::ptr::write_volatile(words.add(8), before.ispr0);
            core::ptr::write_volatile(words.add(9), before.ispr1);
            core::ptr::write_volatile(words.add(10), during.ispr0);
            core::ptr::write_volatile(words.add(11), during.ispr1);
            core::ptr::write_volatile(words.add(12), final_route.ispr0);
            core::ptr::write_volatile(words.add(13), final_route.ispr1);
            core::ptr::write_volatile(
                words.add(14),
                pack_i2c(armed, 0)
                    | pack_i2c(observed, 5)
                    | pack_i2c(final_snapshot, 10)
                    | (((witness == (0x8000_0000 | (20 << 8) | u32::from(ADDRESS))) as u32) << 15),
            );
            core::ptr::write_volatile(words.add(15), iabr_or);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            core::ptr::write_volatile(words, MAGIC);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }

    fn pack_i2c(snapshot: rp1_hal::i2c::I2c1IrqSnapshot, shift: u32) -> u32 {
        (((snapshot.interrupt_mask & STOP_DET != 0) as u32)
            | (((snapshot.raw_interrupt_status & STOP_DET != 0
                || snapshot.masked_interrupt_status & STOP_DET != 0) as u32)
                << 1)
            | (((snapshot.raw_interrupt_status & TX_ABRT != 0
                || snapshot.masked_interrupt_status & TX_ABRT != 0) as u32)
                << 2)
            | (((snapshot.enable_status & 1 != 0) as u32) << 3)
            | (((snapshot.abort_source != 0) as u32) << 4))
            << shift
    }

    const fn empty_i2c() -> rp1_hal::i2c::I2c1IrqSnapshot {
        rp1_hal::i2c::I2c1IrqSnapshot {
            interrupt_mask: 0,
            raw_interrupt_status: 0,
            masked_interrupt_status: 0,
            abort_source: 0,
            enable_status: 0,
        }
    }

    const fn i2c_error_code(error: rp1_hal::i2c::I2c1Error) -> u32 {
        match error {
            rp1_hal::i2c::I2c1Error::ComponentType(_) => 1,
            rp1_hal::i2c::I2c1Error::FifoTooShallow(_) => 2,
            rp1_hal::i2c::I2c1Error::DisableTimeout => 3,
            rp1_hal::i2c::I2c1Error::EnableTimeout => 4,
            rp1_hal::i2c::I2c1Error::InvalidAddress(_) => 5,
            rp1_hal::i2c::I2c1Error::EmptyPayload => 6,
            rp1_hal::i2c::I2c1Error::PayloadTooLong { .. } => 7,
            rp1_hal::i2c::I2c1Error::TxFifoTimeout => 8,
            rp1_hal::i2c::I2c1Error::TxAbort(_) => 9,
            rp1_hal::i2c::I2c1Error::StopTimeout => 10,
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    feature = "uart0-tx-polling-only",
    not(feature = "uart0-rx-irq")
))]
fn publish_uart0_io_readback(written: usize) {
    const MAGIC: u32 = 0x4f49_3055;
    const CLOCK_CTRL: *const u32 = 0x4001_8054 as *const u32;
    const CLOCK_DIV: *const u32 = 0x4001_8058 as *const u32;
    const GPIO14_CTRL: *const u32 = 0x400d_0074 as *const u32;
    const GPIO15_CTRL: *const u32 = 0x400d_007c as *const u32;
    const PAD14: *const u32 = 0x400f_003c as *const u32;
    const PAD15: *const u32 = 0x400f_0040 as *const u32;
    const GPIO_FUNCSEL_MASK: u32 = 0x1f;
    const GPIO_OVERRIDE_MASK: u32 = 0x0003_f000;
    const PAD_OD: u32 = 1 << 7;
    const PAD_IE: u32 = 1 << 6;
    const PAD_SCHMITT: u32 = 1 << 1;
    const PAD_PULL_MASK: u32 = 0b11 << 2;
    const PAD_PULL_UP: u32 = 2 << 2;

    let payload = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;

    unsafe {
        let clock_ctrl = core::ptr::read_volatile(CLOCK_CTRL);
        let clock_div = core::ptr::read_volatile(CLOCK_DIV);
        let gpio14_ctrl = core::ptr::read_volatile(GPIO14_CTRL);
        let gpio15_ctrl = core::ptr::read_volatile(GPIO15_CTRL);
        let pad14 = core::ptr::read_volatile(PAD14);
        let pad15 = core::ptr::read_volatile(PAD15);
        let mut contract = 0u32;

        contract |= ((gpio14_ctrl & GPIO_FUNCSEL_MASK) == 4) as u32;
        contract |= (((gpio14_ctrl & GPIO_OVERRIDE_MASK) == 0) as u32) << 1;
        contract |= (((gpio15_ctrl & GPIO_FUNCSEL_MASK) == 4) as u32) << 2;
        contract |= (((gpio15_ctrl & GPIO_OVERRIDE_MASK) == 0) as u32) << 3;
        contract |= (((pad14 & PAD_OD) == 0) as u32) << 4;
        contract |= (((pad14 & PAD_PULL_MASK) == 0) as u32) << 5;
        contract |= (((pad15 & PAD_IE) != 0) as u32) << 6;
        contract |= (((pad15 & PAD_SCHMITT) != 0) as u32) << 7;
        contract |= (((pad15 & PAD_PULL_MASK) == PAD_PULL_UP) as u32) << 8;
        contract |= ((clock_ctrl == 0x840) as u32) << 9;
        contract |= ((clock_div == 1) as u32) << 10;

        core::ptr::write_volatile(payload, 0);
        core::ptr::write_volatile(payload.add(1), clock_ctrl);
        core::ptr::write_volatile(payload.add(2), clock_div);
        core::ptr::write_volatile(
            payload.add(3),
            (contract & 0xffff) | ((written as u32 & 0xffff) << 16),
        );
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(payload, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(
    target_arch = "arm",
    feature = "uart0-polled-rx",
    not(feature = "uart0-rx-irq")
))]
fn publish_uart0_rx_result(decision: u32, received: usize, bytes: &[u8]) {
    const MAGIC: u32 = 0x5852_3055;
    let payload = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;

    unsafe {
        core::ptr::write_volatile(payload, 0);
        core::ptr::write_volatile(payload.add(1), decision);
        core::ptr::write_volatile(payload.add(2), received as u32);
        for word_index in 0..5 {
            let mut word = 0u32;
            for byte_index in 0..4 {
                let index = word_index * 4 + byte_index;
                if index < bytes.len() {
                    word |= u32::from(bytes[index]) << (byte_index * 8);
                }
            }
            core::ptr::write_volatile(payload.add(3 + word_index), word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(payload, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn UART0_IRQHandler() {
    unsafe {
        rp1_rt::record_uart0_irq_entry();
        Uart0Tx::service_rx_interrupt();
    }
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
fn publish_uart0_irq_result(
    decision: u32,
    irq: rp1_hal::uart::Uart0IrqSnapshot,
    route_before: rp1_rt::Uart0IrqRouteSnapshot,
    route_enabled: rp1_rt::Uart0IrqRouteSnapshot,
    route_final: rp1_rt::Uart0IrqRouteSnapshot,
) {
    const MAGIC: u32 = 0x5149_3055;
    let payload = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    let route_flags = u32::from(route_before.vtor == 0x2000_0000)
        | (u32::from(route_before.iser0 & !(1 << 25) == 0 && route_before.iser1 == 0) << 1)
        | (u32::from(route_enabled.iser0 & (1 << 25) != 0) << 2)
        | (u32::from(route_enabled.primask == 0) << 3)
        | (u32::from(route_final.iser0 & (1 << 25) == 0) << 4);

    unsafe {
        core::ptr::write_volatile(payload, 0);
        core::ptr::write_volatile(payload.add(1), decision);
        core::ptr::write_volatile(
            payload.add(2),
            (irq.byte_count & 0xff) | ((irq.irq_count & 0xff) << 8) | ((irq.ipsr & 0xff) << 16),
        );
        core::ptr::write_volatile(
            payload.add(3),
            (irq.first_ris & 0xffff) | ((irq.first_mis & 0xffff) << 16),
        );
        core::ptr::write_volatile(
            payload.add(4),
            (irq.final_ris & 0xffff) | ((irq.final_mis & 0xffff) << 16),
        );
        core::ptr::write_volatile(
            payload.add(5),
            (irq.rsr & 0xffff) | ((irq.final_imsc & 0xffff) << 16),
        );
        core::ptr::write_volatile(payload.add(6), route_before.vtor);
        core::ptr::write_volatile(payload.add(7), route_flags);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(payload, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "cortex-m3-option-proof"))]
fn cortex_m3_option_hardware_proof() -> [u32; 15] {
    const CPUID: *const u32 = 0xe000_ed00 as *const u32;
    const ICTR: *const u32 = 0xe000_e004 as *const u32;
    const ACTLR: *const u32 = 0xe000_e008 as *const u32;
    const AIRCR: *const u32 = 0xe000_ed0c as *const u32;
    const ID_PFR0: *const u32 = 0xe000_ed40 as *const u32;
    const ID_PFR1: *const u32 = 0xe000_ed44 as *const u32;
    const ID_DFR0: *const u32 = 0xe000_ed48 as *const u32;
    const SYST_CALIB: *const u32 = 0xe000_e01c as *const u32;
    const MPU_TYPE: *const u32 = 0xe000_ed90 as *const u32;
    const MPU_CTRL: *const u32 = 0xe000_ed94 as *const u32;
    const MPU_RNR: *mut u32 = 0xe000_ed98 as *mut u32;
    const MPU_RASR: *const u32 = 0xe000_eda0 as *const u32;
    const DEMCR: *mut u32 = 0xe000_edfc as *mut u32;
    const DWT_CTRL: *mut u32 = 0xe000_1000 as *mut u32;
    const DWT_CYCCNT: *mut u32 = 0xe000_1004 as *mut u32;
    const FP_CTRL: *const u32 = 0xe000_2000 as *const u32;
    const NVIC_ISER: *const u32 = 0xe000_e100 as *const u32;
    const NVIC_ISPR: *const u32 = 0xe000_e200 as *const u32;
    const NVIC_IABR: *const u32 = 0xe000_e300 as *const u32;
    const NVIC_IPR: *mut u8 = 0xe000_e400 as *mut u8;
    const DEMCR_TRCENA: u32 = 1 << 24;
    const DWT_CYCCNTENA: u32 = 1;
    const FLAG_PRIORITY_CANDIDATE_SAFE: u32 = 1 << 0;
    const FLAG_PRIORITY_WRITE_OBSERVED: u32 = 1 << 1;
    const FLAG_PRIORITY_RESTORED: u32 = 1 << 2;
    const FLAG_MPU_SELECTOR_RESTORED: u32 = 1 << 3;
    const FLAG_DWT_CYCCNT_ADVANCED: u32 = 1 << 4;
    const FLAG_DIVIDE_EXECUTED: u32 = 1 << 5;
    const FLAG_EXCLUSIVE_MONITOR: u32 = 1 << 6;
    const FLAG_LITTLE_ENDIAN: u32 = 1 << 7;
    const FLAG_MPU_PRESENT: u32 = 1 << 8;
    const FLAG_SYSTICK_REFERENCE_PRESENT: u32 = 1 << 9;
    const FLAG_SYSTICK_CALIBRATION_EXACT: u32 = 1 << 10;
    const FLAG_DWT_CYCCNT_PRESENT: u32 = 1 << 11;
    const FLAG_DWT_PROFILING_PRESENT: u32 = 1 << 12;
    const FLAG_DWT_TRACE_PACKETS_PRESENT: u32 = 1 << 13;
    const FLAG_DWT_EXTERNAL_MATCH_PRESENT: u32 = 1 << 14;
    const FLAG_FPB_PRESENT: u32 = 1 << 15;
    const FLAG_NVIC_COVERS_IRQ60: u32 = 1 << 16;
    const FLAG_PRIORITY_MATCHES_REGDB: u32 = 1 << 17;
    const FLAG_MPU_REGION_COUNT_BOUNDED: u32 = 1 << 18;
    const FLAG_ALL_MPU_REGIONS_DISABLED: u32 = 1 << 19;
    const FLAG_DEBUG_STATE_RESTORED: u32 = 1 << 20;
    const FLAG_EXCLUSIVE_PLAIN_STORE: u32 = 1 << 21;
    const FLAG_EXCLUSIVE_LDREX_MATCH: u32 = 1 << 22;
    const FLAG_EXCLUSIVE_STREX_SUCCESS: u32 = 1 << 23;

    let cpuid = unsafe { core::ptr::read_volatile(CPUID) };
    let ictr = unsafe { core::ptr::read_volatile(ICTR) };
    let actlr = unsafe { core::ptr::read_volatile(ACTLR) };
    let aircr = unsafe { core::ptr::read_volatile(AIRCR) };
    let id_pfr0 = unsafe { core::ptr::read_volatile(ID_PFR0) };
    let id_pfr1 = unsafe { core::ptr::read_volatile(ID_PFR1) };
    let id_dfr0 = unsafe { core::ptr::read_volatile(ID_DFR0) };
    let systick_calib = unsafe { core::ptr::read_volatile(SYST_CALIB) };
    let mpu_type = unsafe { core::ptr::read_volatile(MPU_TYPE) };
    let mpu_ctrl = unsafe { core::ptr::read_volatile(MPU_CTRL) };
    let dwt_ctrl = unsafe { core::ptr::read_volatile(DWT_CTRL) };
    let fp_ctrl = unsafe { core::ptr::read_volatile(FP_CTRL) };
    let mut flags = 0u32;

    if aircr & (1 << 15) == 0 {
        flags |= FLAG_LITTLE_ENDIAN;
    }
    let external_irq_lines = ((ictr & 0x0f) + 1) * 32;
    if external_irq_lines > 60 {
        flags |= FLAG_NVIC_COVERS_IRQ60;
    }
    if systick_calib & (1 << 31) == 0 {
        flags |= FLAG_SYSTICK_REFERENCE_PRESENT;
    }
    if systick_calib & (1 << 30) == 0 && systick_calib & 0x00ff_ffff != 0 {
        flags |= FLAG_SYSTICK_CALIBRATION_EXACT;
    }
    if dwt_ctrl & (1 << 25) == 0 {
        flags |= FLAG_DWT_CYCCNT_PRESENT;
    }
    if dwt_ctrl & (1 << 24) == 0 {
        flags |= FLAG_DWT_PROFILING_PRESENT;
    }
    if dwt_ctrl & (1 << 27) == 0 {
        flags |= FLAG_DWT_TRACE_PACKETS_PRESENT;
    }
    if dwt_ctrl & (1 << 26) == 0 {
        flags |= FLAG_DWT_EXTERNAL_MATCH_PRESENT;
    }
    let fp_code_comparators = ((fp_ctrl >> 4) & 0x0f) | (((fp_ctrl >> 12) & 0x07) << 4);
    if fp_code_comparators != 0 {
        flags |= FLAG_FPB_PRESENT;
    }

    let mpu_region_count = (mpu_type >> 8) & 0xff;
    if mpu_region_count != 0 {
        flags |= FLAG_MPU_PRESENT;
    }
    if mpu_region_count <= 8 {
        flags |= FLAG_MPU_REGION_COUNT_BOUNDED;
    }
    let original_mpu_rnr = unsafe { core::ptr::read_volatile(MPU_RNR) };
    let mut mpu_enabled_mask = 0u32;
    for region in 0..core::cmp::min(mpu_region_count, 8) {
        unsafe {
            core::ptr::write_volatile(MPU_RNR, region);
        }
        let rasr = unsafe { core::ptr::read_volatile(MPU_RASR) };
        if rasr & 1 != 0 {
            mpu_enabled_mask |= 1 << region;
        }
    }
    unsafe {
        core::ptr::write_volatile(MPU_RNR, original_mpu_rnr);
    }
    if unsafe { core::ptr::read_volatile(MPU_RNR) } == original_mpu_rnr {
        flags |= FLAG_MPU_SELECTOR_RESTORED;
    }
    if mpu_enabled_mask == 0 {
        flags |= FLAG_ALL_MPU_REGIONS_DISABLED;
    }

    let mut priority_irq = 0xffu32;
    let mut priority_original = 0u8;
    let mut priority_mask = 0u8;
    let mut priority_restored = 0u8;
    let mut priority_zero = 0xffu8;
    let primask: u32;
    unsafe {
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            saved = out(reg) primask,
            options(nostack, preserves_flags)
        );
    }
    let scan_limit = core::cmp::min(external_irq_lines, 61);
    for irq in (0..scan_limit).rev() {
        let bank = (irq / 32) as usize;
        let bit = 1u32 << (irq % 32);
        let enabled = unsafe { core::ptr::read_volatile(NVIC_ISER.add(bank)) } & bit != 0;
        let pending = unsafe { core::ptr::read_volatile(NVIC_ISPR.add(bank)) } & bit != 0;
        let active = unsafe { core::ptr::read_volatile(NVIC_IABR.add(bank)) } & bit != 0;
        if enabled || pending || active {
            continue;
        }

        let priority = unsafe { NVIC_IPR.add(irq as usize) };
        let original = unsafe { core::ptr::read_volatile(priority) };
        unsafe {
            core::ptr::write_volatile(priority, 0xff);
            core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
        }
        let mask = unsafe { core::ptr::read_volatile(priority) };
        unsafe {
            core::ptr::write_volatile(priority, 0);
            core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
        }
        let zero = unsafe { core::ptr::read_volatile(priority) };
        unsafe {
            core::ptr::write_volatile(priority, original);
            core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
        }
        let restored = unsafe { core::ptr::read_volatile(priority) };
        if mask == 0 {
            continue;
        }
        priority_irq = irq;
        priority_original = original;
        priority_mask = mask;
        priority_zero = zero;
        priority_restored = restored;
        flags |= FLAG_PRIORITY_CANDIDATE_SAFE;
        break;
    }
    unsafe {
        core::arch::asm!(
            "msr PRIMASK, {saved}",
            saved = in(reg) primask,
            options(nostack, preserves_flags)
        );
    }
    let priority_bits = priority_mask.count_ones();
    if priority_mask != 0 && priority_zero == 0 {
        flags |= FLAG_PRIORITY_WRITE_OBSERVED;
    }
    if priority_irq != 0xff && priority_restored == priority_original {
        flags |= FLAG_PRIORITY_RESTORED;
    }
    if priority_bits == 3 {
        flags |= FLAG_PRIORITY_MATCHES_REGDB;
    }

    let original_demcr = unsafe { core::ptr::read_volatile(DEMCR) };
    let original_dwt_ctrl = unsafe { core::ptr::read_volatile(DWT_CTRL) };
    let original_cyccnt = unsafe { core::ptr::read_volatile(DWT_CYCCNT) };
    let mut cycle_delta = 0u32;
    if original_dwt_ctrl & (1 << 25) == 0 {
        unsafe {
            core::ptr::write_volatile(DEMCR, original_demcr | DEMCR_TRCENA);
            core::ptr::write_volatile(DWT_CTRL, original_dwt_ctrl & !DWT_CYCCNTENA);
            core::ptr::write_volatile(DWT_CYCCNT, 0);
            core::ptr::write_volatile(DWT_CTRL, original_dwt_ctrl | DWT_CYCCNTENA);
            core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
            for _ in 0..256 {
                core::arch::asm!("nop", options(nomem, nostack, preserves_flags));
            }
            cycle_delta = core::ptr::read_volatile(DWT_CYCCNT);
            core::ptr::write_volatile(DWT_CTRL, original_dwt_ctrl & !DWT_CYCCNTENA);
            core::ptr::write_volatile(DWT_CYCCNT, original_cyccnt);
            core::ptr::write_volatile(DWT_CTRL, original_dwt_ctrl);
            core::ptr::write_volatile(DEMCR, original_demcr);
            core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
        }
        if cycle_delta != 0 {
            flags |= FLAG_DWT_CYCCNT_ADVANCED;
        }
    }
    let restored_demcr = unsafe { core::ptr::read_volatile(DEMCR) };
    let restored_dwt_ctrl = unsafe { core::ptr::read_volatile(DWT_CTRL) };
    let restored_cyccnt = unsafe { core::ptr::read_volatile(DWT_CYCCNT) };
    if restored_demcr == original_demcr
        && restored_dwt_ctrl == original_dwt_ctrl
        && (original_dwt_ctrl & DWT_CYCCNTENA != 0 || restored_cyccnt == original_cyccnt)
    {
        flags |= FLAG_DEBUG_STATE_RESTORED;
    }

    let quotient: u32;
    unsafe {
        core::arch::asm!(
            "udiv {result}, {dividend}, {divisor}",
            result = out(reg) quotient,
            dividend = in(reg) 100u32,
            divisor = in(reg) 7u32,
            options(nomem, nostack, preserves_flags)
        );
    }
    if quotient == 14 {
        flags |= FLAG_DIVIDE_EXECUTED;
    }

    static EXCLUSIVE_WORD: core::sync::atomic::AtomicU32 =
        core::sync::atomic::AtomicU32::new(0x4558_434c);
    let exclusive_ptr = EXCLUSIVE_WORD.as_ptr();
    let exclusive_original = unsafe { core::ptr::read_volatile(exclusive_ptr) };
    let exclusive_test = exclusive_original ^ 0x0000_ffff;
    unsafe {
        core::ptr::write_volatile(exclusive_ptr, exclusive_test);
    }
    let plain_observed = unsafe { core::ptr::read_volatile(exclusive_ptr) };
    unsafe {
        core::ptr::write_volatile(exclusive_ptr, exclusive_original);
    }
    if plain_observed == exclusive_test
        && unsafe { core::ptr::read_volatile(exclusive_ptr) } == exclusive_original
    {
        flags |= FLAG_EXCLUSIVE_PLAIN_STORE;
    }

    let exclusive_primask: u32;
    let exclusive_loaded: u32;
    let strex_status: u32;
    unsafe {
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            "dmb sy",
            "ldrex {loaded}, [{address}]",
            "strex {status}, {value}, [{address}]",
            "dmb sy",
            saved = out(reg) exclusive_primask,
            loaded = out(reg) exclusive_loaded,
            status = out(reg) strex_status,
            address = in(reg) exclusive_ptr,
            value = in(reg) exclusive_test,
            options(nostack, preserves_flags)
        );
    }
    let exclusive_observed = unsafe { core::ptr::read_volatile(exclusive_ptr) };
    unsafe {
        core::arch::asm!("clrex", options(nomem, nostack, preserves_flags));
        core::ptr::write_volatile(exclusive_ptr, exclusive_original);
    }
    let exclusive_restored = unsafe { core::ptr::read_volatile(exclusive_ptr) };
    unsafe {
        core::arch::asm!(
            "msr PRIMASK, {saved}",
            saved = in(reg) exclusive_primask,
            options(nostack, preserves_flags)
        );
    }
    if exclusive_loaded == exclusive_original {
        flags |= FLAG_EXCLUSIVE_LDREX_MATCH;
    }
    if strex_status == 0 {
        flags |= FLAG_EXCLUSIVE_STREX_SUCCESS;
    }
    if exclusive_loaded == exclusive_original
        && strex_status == 0
        && exclusive_observed == exclusive_test
        && exclusive_restored == exclusive_original
    {
        flags |= FLAG_EXCLUSIVE_MONITOR;
    }

    let priority_probe = priority_irq
        | ((priority_original as u32) << 8)
        | ((priority_mask as u32) << 16)
        | ((priority_restored as u32) << 24);
    let summary = (priority_bits & 0x0f)
        | ((mpu_enabled_mask & 0xff) << 4)
        | ((quotient & 0xff) << 12)
        | (core::cmp::min(cycle_delta, 0x0fff) << 20);

    [
        (16 << 16) | 1,
        flags,
        cpuid,
        ictr,
        actlr,
        mpu_type,
        mpu_ctrl,
        systick_calib,
        dwt_ctrl,
        fp_ctrl,
        id_pfr0,
        id_pfr1,
        id_dfr0,
        priority_probe,
        summary,
    ]
}

#[cfg(all(target_arch = "arm", feature = "cortex-m3-option-proof"))]
fn publish_cortex_m3_option_hardware_proof(fields: [u32; 15]) {
    const MAGIC: u32 = 0x3133_4d43; // CM31
    const RESULT_WORDS: usize = 16;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(words, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "boot-rom-readonly-proof",
        feature = "boot-rom-boundary-proof",
        feature = "proc1-boot-rom-proof"
    )
))]
#[inline(always)]
unsafe fn boot_rom_read_word(address: u32) -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!(
            "ldr {value}, [{address}]",
            value = out(reg) value,
            address = in(reg) address,
            options(nostack, readonly, preserves_flags)
        );
    }
    value
}

#[cfg(all(
    target_arch = "arm",
    feature = "boot-rom-readonly-proof",
    not(feature = "boot-rom-dump-proof"),
    not(feature = "boot-rom-boundary-proof")
))]
fn publish_boot_rom_readonly_proof() {
    const MAGIC: u32 = 0x314d_4f52; // ROM1
    const FORMAT: u32 = (16 << 16) | 1;
    const IN_PROGRESS: u32 = 0x4252_0001;
    const ROM_VECTOR_WORDS: usize = 8;
    const HASH_WORDS: usize = 64;
    let output = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    let shared = 0x2000_0000 as *const u32;

    unsafe {
        for index in 0..16 {
            core::ptr::write_volatile(output.add(index), 0);
        }
        core::ptr::write_volatile(output, MAGIC);
        core::ptr::write_volatile(output.add(1), FORMAT);
        core::ptr::write_volatile(
            output.add(2),
            core::ptr::read_volatile(0xe000_ed00 as *const u32),
        );
        core::ptr::write_volatile(
            output.add(3),
            core::ptr::read_volatile(0xe000_ed08 as *const u32),
        );
        core::ptr::write_volatile(output.add(15), IN_PROGRESS);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let mut vectors = [0u32; ROM_VECTOR_WORDS];
        for (index, value) in vectors.iter_mut().enumerate() {
            *value = boot_rom_read_word((index * core::mem::size_of::<u32>()) as u32);
            core::ptr::write_volatile(output.add(index + 4), *value);
        }

        let shared_sp = core::ptr::read_volatile(shared);
        let shared_reset = core::ptr::read_volatile(shared.add(1));
        core::ptr::write_volatile(output.add(12), shared_sp);
        core::ptr::write_volatile(output.add(13), shared_reset);

        let mut hash = 0x811c_9dc5u32;
        for index in 0..HASH_WORDS {
            hash ^= boot_rom_read_word((index * core::mem::size_of::<u32>()) as u32);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        core::ptr::write_volatile(output.add(14), hash);

        let mut flags = 1u32;
        if vectors[0] != shared_sp || vectors[1] != shared_reset {
            flags |= 1 << 1;
        }
        if (0x2000_0000..0x2001_0000).contains(&vectors[0]) {
            flags |= 1 << 2;
        }
        if vectors[1] & 1 != 0 {
            flags |= 1 << 3;
        }
        if (vectors[1] & !1) < 0x2000_0000 {
            flags |= 1 << 4;
        }
        core::ptr::write_volatile(output.add(15), flags);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "boot-rom-dump-proof"))]
fn publish_boot_rom_dump() {
    const MAGIC: u32 = 0x3144_4d52; // RMD1
    const FORMAT: u32 = (16 << 16) | 1;
    const ROM_SOURCE: u32 = 0x0000_0000;
    const DUMP_DESTINATION: u32 = 0x2000_6400;
    const DUMP_LENGTH: usize = 0x8000;
    const WORDS: usize = DUMP_LENGTH / core::mem::size_of::<u32>();
    let destination = DUMP_DESTINATION as usize as *mut u32;
    let output = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;

    let mut hash = 0x811c_9dc5u32;
    unsafe {
        for index in 0..WORDS {
            let value = boot_rom_read_word(ROM_SOURCE + (index as u32 * 4));
            core::ptr::write_volatile(destination.add(index), value);
            hash ^= value;
            hash = hash.wrapping_mul(0x0100_0193);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let fields = [
            MAGIC,
            FORMAT,
            ROM_SOURCE,
            DUMP_DESTINATION,
            DUMP_LENGTH as u32,
            hash,
            core::ptr::read_volatile(destination),
            core::ptr::read_volatile(destination.add(1)),
            core::ptr::read_volatile(destination.add(0x1ffc / 4)),
            core::ptr::read_volatile(destination.add(0x3ffc / 4)),
            core::ptr::read_volatile(destination.add(0x5ffc / 4)),
            core::ptr::read_volatile(destination.add(0x7ffc / 4)),
            core::ptr::read_volatile(0xe000_ed00 as *const u32),
            core::ptr::read_volatile(0xe000_ed08 as *const u32),
            WORDS as u32,
            1,
        ];
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(output.add(index), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "boot-rom-boundary-proof"))]
fn publish_boot_rom_boundary_proof() {
    const MAGIC: u32 = 0x3142_4d52; // RMB1
    const FORMAT: u32 = (16 << 16) | 1;
    const MIRROR_OFFSET: u32 = 0x8000;
    const WORDS: usize = 0x8000 / core::mem::size_of::<u32>();
    let output = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    let shared = 0x2000_0000 as *const u32;

    unsafe {
        let mut primary_hash = 0x811c_9dc5u32;
        let mut mirror_hash = 0x811c_9dc5u32;
        let mut mismatch_count = 0u32;
        let mut first_mismatch = u32::MAX;

        for index in 0..WORDS {
            let offset = (index * core::mem::size_of::<u32>()) as u32;
            let primary = boot_rom_read_word(offset);
            let mirror = boot_rom_read_word(MIRROR_OFFSET + offset);
            primary_hash = (primary_hash ^ primary).wrapping_mul(0x0100_0193);
            mirror_hash = (mirror_hash ^ mirror).wrapping_mul(0x0100_0193);
            if primary != mirror {
                mismatch_count = mismatch_count.wrapping_add(1);
                if first_mismatch == u32::MAX {
                    first_mismatch = offset;
                }
            }
        }

        let fields = [
            MAGIC,
            FORMAT,
            boot_rom_read_word(0x0000),
            boot_rom_read_word(0x7ffc),
            boot_rom_read_word(0x8000),
            boot_rom_read_word(0xfffc),
            primary_hash,
            mirror_hash,
            mismatch_count,
            first_mismatch,
            core::ptr::read_volatile(0xe000_ed00 as *const u32),
            core::ptr::read_volatile(0xe000_ed08 as *const u32),
            core::ptr::read_volatile(shared),
            core::ptr::read_volatile(shared.add(1)),
            WORDS as u32,
            1,
        ];
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(output.add(index), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
const PROC1_PROOF_WORDS: usize = 16;

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
const PROC1_PROOF_MAGIC: u32 = 0x3143_5052; // RPC1

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
const PROC1_PROOF_DONE: u32 = 0x3145_4e44; // DNE1

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
const PROC1_PROOF_TIMEOUT: u32 = 0x5455_4f54; // TOUT

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
const PROC1_PROOF_RESET_TIMEOUT: u32 = 0x554f_5452; // RTOU

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
static mut PROC1_PROOF_STACK: [u64; 256] = [0; 256];

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
static mut PROC1_PROOF_RESULT: [u32; PROC1_PROOF_WORDS] = [0; PROC1_PROOF_WORDS];

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
fn proc1_proof_stack_top() -> u32 {
    let base = core::ptr::addr_of!(PROC1_PROOF_STACK) as usize;
    (base + core::mem::size_of::<[u64; 256]>()) as u32
}

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Proc1BootRomProof(core_id: u32, _arg1: u32, _arg2: u32) {
    const CORE_ID: *const u32 = 0xe00f_f01c as *const u32;
    const CPUID: *const u32 = 0xe000_ed00 as *const u32;
    const VTOR: *const u32 = 0xe000_ed08 as *const u32;
    const PROC1_ENTRY_SCRATCH: *const u32 = 0x4015_4014 as *const u32;
    const WORDS: usize = 0x8000 / core::mem::size_of::<u32>();

    let output = core::ptr::addr_of_mut!(PROC1_PROOF_RESULT).cast::<u32>();
    unsafe {
        core::ptr::write_volatile(output.add(15), 0);
        core::ptr::write_volatile(output, PROC1_PROOF_MAGIC);

        let mut primary_hash = 0x811c_9dc5u32;
        let mut mirror_hash = 0x811c_9dc5u32;
        let mut mismatch_count = 0u32;
        for index in 0..WORDS {
            let offset = (index * core::mem::size_of::<u32>()) as u32;
            let primary = boot_rom_read_word(offset);
            let mirror = boot_rom_read_word(0x8000 + offset);
            primary_hash = (primary_hash ^ primary).wrapping_mul(0x0100_0193);
            mirror_hash = (mirror_hash ^ mirror).wrapping_mul(0x0100_0193);
            if primary != mirror {
                mismatch_count = mismatch_count.wrapping_add(1);
            }
        }

        let fields = [
            PROC1_PROOF_MAGIC,
            (PROC1_PROOF_WORDS as u32) << 16 | 1,
            core_id,
            core::ptr::read_volatile(CORE_ID),
            core::ptr::read_volatile(CPUID),
            core::ptr::read_volatile(VTOR),
            boot_rom_read_word(0x0000),
            boot_rom_read_word(0x7ffc),
            boot_rom_read_word(0x8000),
            boot_rom_read_word(0xfffc),
            primary_hash,
            mirror_hash,
            mismatch_count,
            core::ptr::read_volatile(PROC1_ENTRY_SCRATCH),
            proc1_proof_stack_top(),
            PROC1_PROOF_DONE,
        ];
        for (index, value) in fields.into_iter().enumerate().skip(1) {
            core::ptr::write_volatile(output.add(index), value);
        }
        core::arch::asm!("dsb sy", "sev", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "proc1-boot-rom-proof"))]
fn publish_proc1_boot_rom_proof() {
    const RESET_CTRL0: *mut u32 = 0x4001_4000 as *mut u32;
    const RESET_DONE0: *const u32 = 0x4001_4018 as *const u32;
    const PROC1_RESET_MASK: u32 = 1 << 31;
    const START_MAGIC: *mut u32 = 0x4015_400c as *mut u32;
    const PROC1_ENTRY: *mut u32 = 0x4015_4014 as *mut u32;
    const PROC1_STACK: *mut u32 = 0x4015_401c as *mut u32;
    const START_MAGIC_VALUE: u32 = 0xb007_c0de;
    const PROC1_ENTRY_XOR: u32 = 0x4ff8_3f2d;
    const RESET_TIMEOUT_US: u64 = 100_000;
    const TIMEOUT_US: u64 = 100_000;
    const _: () =
        assert!(PROC1_PROOF_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let result = core::ptr::addr_of_mut!(PROC1_PROOF_RESULT).cast::<u32>();
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    let stack_top = proc1_proof_stack_top();
    let entry = (Proc1BootRomProof as *const () as usize as u32) | 1;
    unsafe {
        for index in 0..PROC1_PROOF_WORDS {
            core::ptr::write_volatile(result.add(index), 0);
        }
        core::ptr::write_volatile(START_MAGIC, START_MAGIC_VALUE);
        core::ptr::write_volatile(PROC1_STACK, stack_top);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(PROC1_ENTRY, entry ^ PROC1_ENTRY_XOR);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let reset_ctrl = core::ptr::read_volatile(RESET_CTRL0);
        core::ptr::write_volatile(RESET_CTRL0, reset_ctrl & !PROC1_RESET_MASK);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let reset_start = raw_timer_us();
        while core::ptr::read_volatile(RESET_DONE0) & PROC1_RESET_MASK == 0
            && raw_timer_us().wrapping_sub(reset_start) <= RESET_TIMEOUT_US
        {
            core::hint::spin_loop();
        }

        if core::ptr::read_volatile(RESET_DONE0) & PROC1_RESET_MASK == 0 {
            core::ptr::write_volatile(result, PROC1_PROOF_MAGIC);
            core::ptr::write_volatile(result.add(1), (PROC1_PROOF_WORDS as u32) << 16 | 1);
            core::ptr::write_volatile(result.add(13), core::ptr::read_volatile(PROC1_ENTRY));
            core::ptr::write_volatile(result.add(14), stack_top);
            core::ptr::write_volatile(result.add(15), PROC1_PROOF_RESET_TIMEOUT);
        } else {
            core::arch::asm!("dsb sy", "sev", options(nostack, preserves_flags));

            let start = raw_timer_us();
            while core::ptr::read_volatile(result.add(15)) != PROC1_PROOF_DONE
                && raw_timer_us().wrapping_sub(start) <= TIMEOUT_US
            {
                core::hint::spin_loop();
            }
            if core::ptr::read_volatile(result.add(15)) != PROC1_PROOF_DONE {
                core::ptr::write_volatile(result, PROC1_PROOF_MAGIC);
                core::ptr::write_volatile(result.add(1), (PROC1_PROOF_WORDS as u32) << 16 | 1);
                core::ptr::write_volatile(result.add(13), core::ptr::read_volatile(PROC1_ENTRY));
                core::ptr::write_volatile(result.add(14), stack_top);
                core::ptr::write_volatile(result.add(15), PROC1_PROOF_TIMEOUT);
            }
        }

        for index in 0..PROC1_PROOF_WORDS {
            core::ptr::write_volatile(
                mailbox.add(index),
                core::ptr::read_volatile(result.add(index)),
            );
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_WORDS: usize = 33;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_MAILBOX_WORDS: usize = 16;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_MAGIC: u32 = 0x314d_4344; // DCM1

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_DONE: u32 = 0x3145_4e44; // DNE1

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_RESET_TIMEOUT: u32 = 0x5452_4344; // DCRT

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CALLBACK_TIMEOUT: u32 = 0x5443_4344; // DCCT

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PLAIN_TIMEOUT: u32 = 0x5450_4344; // DCPT

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PROC1_TIMEOUT: u32 = 0x5431_4344; // DC1T

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CROSS_READY_TIMEOUT: u32 = 0x5452_5843; // CXRT

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CROSS_EVENT_TIMEOUT: u32 = 0x5445_5843; // CXET

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CMD_PLAIN: u32 = 0x4e41_4c50; // PLAN

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CMD_PROC1_EXCLUSIVE: u32 = 0x3158_4345; // ECX1

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CMD_CROSS: u32 = 0x5353_4f52; // ROSS

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_ACK_CALLBACK: u32 = 0x5944_5231; // 1RDY

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_ACK_PLAIN: u32 = 0x4b4f_4c50; // PLOK

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_ACK_PROC1_EXCLUSIVE: u32 = 0x4b4f_3158; // X1OK

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_ACK_CROSS_READY: u32 = 0x5944_5258; // XRDY

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_ACK_CROSS_DONE: u32 = 0x454e_4458; // XDNE

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CROSS_WRITE_DONE: u32 = 0x4554_4f4d; // MOTE

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PLAIN_VALUE: u32 = 0x504c_4149;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PLAIN_XOR: u32 = 0xa5a5_5a5a;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PROC1_ORIGINAL: u32 = 0x3145_584f;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PROC1_REPLACEMENT: u32 = 0x3153_584f;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PROC0_ORIGINAL: u32 = 0x3045_584f;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_PROC0_REPLACEMENT: u32 = 0x3053_584f;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CLREX_ORIGINAL: u32 = 0x434c_524f;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CLREX_REPLACEMENT: u32 = 0x434c_5253;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CROSS_ORIGINAL: u32 = 0x4352_4f53;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CROSS_PROC0_VALUE: u32 = 0x4350_3053;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
const DUAL_CORE_CROSS_PROC1_VALUE: u32 = 0x4350_3153;

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
mod dual_core_index {
    pub const FORMAT: usize = 1;
    pub const COMMAND: usize = 2;
    pub const ACK: usize = 3;
    pub const PROC1_ARG: usize = 4;
    pub const PROC1_CORE_ID: usize = 5;
    pub const PLAIN_WORD: usize = 6;
    pub const PLAIN_SEEN: usize = 7;
    pub const PLAIN_REPLY: usize = 8;
    pub const PROC1_WORD: usize = 9;
    pub const PROC1_LOADED: usize = 10;
    pub const PROC1_STREX: usize = 11;
    pub const PROC1_FINAL: usize = 12;
    pub const PROC0_WORD: usize = 13;
    pub const PROC0_LOADED: usize = 14;
    pub const PROC0_STREX: usize = 15;
    pub const PROC0_FINAL: usize = 16;
    pub const CLREX_WORD: usize = 17;
    pub const CLREX_LOADED: usize = 18;
    pub const CLREX_STREX: usize = 19;
    pub const CLREX_FINAL: usize = 20;
    pub const CROSS_WORD: usize = 21;
    pub const CROSS_WRITE_DONE: usize = 22;
    pub const CROSS_LOADED: usize = 23;
    pub const CROSS_STREX: usize = 24;
    pub const CROSS_FINAL: usize = 25;
    pub const CROSS_REMAINING: usize = 26;
    pub const RESET_CTRL_BEFORE: usize = 27;
    pub const RESET_DONE_AFTER: usize = 28;
    pub const ENTRY_AFTER: usize = 29;
    pub const STACK_TOP: usize = 30;
    pub const FLAGS: usize = 31;
    pub const COMPLETION: usize = 32;
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
#[repr(C, align(32))]
struct DualCoreProofBlock([u32; DUAL_CORE_WORDS]);

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
static mut DUAL_CORE_BLOCK: DualCoreProofBlock = DualCoreProofBlock([0; DUAL_CORE_WORDS]);

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
static mut DUAL_CORE_STACK: [u64; 256] = [0; 256];

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
fn dual_core_block() -> *mut u32 {
    core::ptr::addr_of_mut!(DUAL_CORE_BLOCK).cast::<u32>()
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_atomic(
    block: *mut u32,
    index: usize,
) -> &'static core::sync::atomic::AtomicU32 {
    unsafe { &*block.add(index).cast::<core::sync::atomic::AtomicU32>() }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_atomic_load(block: *mut u32, index: usize) -> u32 {
    unsafe { dual_core_atomic(block, index) }.load(core::sync::atomic::Ordering::Acquire)
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_atomic_store(block: *mut u32, index: usize, value: u32) {
    unsafe { dual_core_atomic(block, index) }.store(value, core::sync::atomic::Ordering::Release);
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
fn dual_core_stack_top() -> u32 {
    let base = core::ptr::addr_of!(DUAL_CORE_STACK) as usize;
    (base + core::mem::size_of::<[u64; 256]>()) as u32
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_wait_atomic(
    block: *mut u32,
    index: usize,
    expected: u32,
    timeout_us: u64,
) -> bool {
    let start = raw_timer_us();
    while unsafe { dual_core_atomic_load(block, index) } != expected
        && raw_timer_us().wrapping_sub(start) <= timeout_us
    {
        core::hint::spin_loop();
    }
    unsafe { dual_core_atomic_load(block, index) == expected }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_wait_mask(
    address: *const u32,
    mask: u32,
    expected: u32,
    timeout_us: u64,
) -> bool {
    let start = raw_timer_us();
    while unsafe { core::ptr::read_volatile(address) } & mask != expected
        && raw_timer_us().wrapping_sub(start) <= timeout_us
    {
        core::hint::spin_loop();
    }
    unsafe { core::ptr::read_volatile(address) & mask == expected }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_wait_command(block: *mut u32, expected: u32) {
    while unsafe { dual_core_atomic_load(block, dual_core_index::COMMAND) } != expected {
        core::hint::spin_loop();
    }
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_exclusive_roundtrip(address: *mut u32, value: u32) -> (u32, u32) {
    let loaded: u32;
    let status: u32;
    unsafe {
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            "dmb sy",
            "ldrex {loaded}, [{address}]",
            "strex {status}, {value}, [{address}]",
            "dmb sy",
            "msr PRIMASK, {saved}",
            saved = out(reg) _,
            loaded = out(reg) loaded,
            status = out(reg) status,
            address = in(reg) address,
            value = in(reg) value,
            options(nostack)
        );
    }
    (loaded, status)
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_clrex_control(address: *mut u32, value: u32) -> (u32, u32) {
    let loaded: u32;
    let status: u32;
    unsafe {
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            "dmb sy",
            "ldrex {loaded}, [{address}]",
            "clrex",
            "strex {status}, {value}, [{address}]",
            "dmb sy",
            "msr PRIMASK, {saved}",
            saved = out(reg) _,
            loaded = out(reg) loaded,
            status = out(reg) status,
            address = in(reg) address,
            value = in(reg) value,
            options(nostack)
        );
    }
    (loaded, status)
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Proc1DualCoreMemoryProof(core_id: u32, _arg1: u32, _arg2: u32) {
    const CORE_ID: *const u32 = 0xe00f_f01c as *const u32;
    let block = dual_core_block();
    let saved_primask: u32;
    unsafe {
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            saved = out(reg) saved_primask,
            options(nostack, preserves_flags)
        );
        core::ptr::write_volatile(block.add(dual_core_index::PROC1_ARG), core_id);
        core::ptr::write_volatile(
            block.add(dual_core_index::PROC1_CORE_ID),
            core::ptr::read_volatile(CORE_ID),
        );
        dual_core_atomic_store(block, dual_core_index::ACK, DUAL_CORE_ACK_CALLBACK);
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));

        dual_core_wait_command(block, DUAL_CORE_CMD_PLAIN);
        let plain = core::ptr::read_volatile(block.add(dual_core_index::PLAIN_WORD));
        core::ptr::write_volatile(block.add(dual_core_index::PLAIN_SEEN), plain);
        core::ptr::write_volatile(
            block.add(dual_core_index::PLAIN_REPLY),
            plain ^ DUAL_CORE_PLAIN_XOR,
        );
        dual_core_atomic_store(block, dual_core_index::ACK, DUAL_CORE_ACK_PLAIN);
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));

        dual_core_wait_command(block, DUAL_CORE_CMD_PROC1_EXCLUSIVE);
        let proc1_word = block.add(dual_core_index::PROC1_WORD);
        let (loaded, status) =
            dual_core_exclusive_roundtrip(proc1_word, DUAL_CORE_PROC1_REPLACEMENT);
        core::ptr::write_volatile(block.add(dual_core_index::PROC1_LOADED), loaded);
        core::ptr::write_volatile(block.add(dual_core_index::PROC1_STREX), status);
        core::ptr::write_volatile(
            block.add(dual_core_index::PROC1_FINAL),
            core::ptr::read_volatile(proc1_word),
        );
        dual_core_atomic_store(block, dual_core_index::ACK, DUAL_CORE_ACK_PROC1_EXCLUSIVE);
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));

        dual_core_wait_command(block, DUAL_CORE_CMD_CROSS);
        core::arch::asm!("sev", "wfe", options(nostack, preserves_flags));
        dual_core_atomic_store(block, dual_core_index::ACK, DUAL_CORE_ACK_CROSS_READY);
        core::arch::asm!("dmb sy", "wfe", options(nostack, preserves_flags));
        dual_core_atomic_store(
            block,
            dual_core_index::CROSS_WORD,
            DUAL_CORE_CROSS_PROC1_VALUE,
        );
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
        dual_core_atomic_store(
            block,
            dual_core_index::CROSS_WRITE_DONE,
            DUAL_CORE_CROSS_WRITE_DONE,
        );
        dual_core_atomic_store(block, dual_core_index::ACK, DUAL_CORE_ACK_CROSS_DONE);
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));
        core::arch::asm!(
            "msr PRIMASK, {saved}",
            saved = in(reg) saved_primask,
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
unsafe fn dual_core_publish(block: *mut u32) {
    const _: () = assert!(
        DUAL_CORE_MAILBOX_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE
    );
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        let status_pack = (core::ptr::read_volatile(block.add(dual_core_index::PROC1_STREX))
            & 0xff)
            | ((core::ptr::read_volatile(block.add(dual_core_index::PROC0_STREX)) & 0xff) << 8)
            | ((core::ptr::read_volatile(block.add(dual_core_index::CLREX_STREX)) & 0xff) << 16)
            | ((core::ptr::read_volatile(block.add(dual_core_index::CROSS_STREX)) & 0xff) << 24);
        let fields = [
            DUAL_CORE_MAGIC,
            (DUAL_CORE_MAILBOX_WORDS as u32) << 16 | 1,
            core::ptr::read_volatile(block.add(dual_core_index::FLAGS)),
            core::ptr::read_volatile(block.add(dual_core_index::COMPLETION)),
            (core::ptr::read_volatile(block.add(dual_core_index::PROC1_ARG)) & 0xffff)
                | ((core::ptr::read_volatile(block.add(dual_core_index::PROC1_CORE_ID)) & 0xffff)
                    << 16),
            core::ptr::read_volatile(block.add(dual_core_index::PLAIN_SEEN)),
            core::ptr::read_volatile(block.add(dual_core_index::PLAIN_REPLY)),
            core::ptr::read_volatile(block.add(dual_core_index::PROC1_LOADED)),
            core::ptr::read_volatile(block.add(dual_core_index::PROC1_FINAL)),
            core::ptr::read_volatile(block.add(dual_core_index::PROC0_LOADED)),
            core::ptr::read_volatile(block.add(dual_core_index::PROC0_FINAL)),
            core::ptr::read_volatile(block.add(dual_core_index::CLREX_LOADED)),
            core::ptr::read_volatile(block.add(dual_core_index::CLREX_FINAL)),
            core::ptr::read_volatile(block.add(dual_core_index::CROSS_LOADED)),
            core::ptr::read_volatile(block.add(dual_core_index::CROSS_FINAL)),
            status_pack,
        ];
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate().skip(1) {
            core::ptr::write_volatile(mailbox.add(index), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, DUAL_CORE_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "dual-core-memory-proof"))]
fn publish_dual_core_memory_proof() {
    const RESET_CTRL0: *mut u32 = 0x4001_4000 as *mut u32;
    const RESET_DONE0: *const u32 = 0x4001_4018 as *const u32;
    const PROC1_RESET_MASK: u32 = 1 << 31;
    const START_MAGIC: *mut u32 = 0x4015_400c as *mut u32;
    const PROC1_ENTRY: *mut u32 = 0x4015_4014 as *mut u32;
    const PROC1_STACK: *mut u32 = 0x4015_401c as *mut u32;
    const START_MAGIC_VALUE: u32 = 0xb007_c0de;
    const PROC1_ENTRY_XOR: u32 = 0x4ff8_3f2d;
    const TIMEOUT_US: u64 = 100_000;
    const CROSS_LOOP_LIMIT: u32 = 10_000_000;

    let block = dual_core_block();
    let stack_top = dual_core_stack_top();
    let entry = (Proc1DualCoreMemoryProof as *const () as usize as u32) | 1;
    unsafe {
        for index in 0..DUAL_CORE_WORDS {
            core::ptr::write_volatile(block.add(index), 0);
        }
        core::ptr::write_volatile(
            block.add(dual_core_index::FORMAT),
            (DUAL_CORE_WORDS as u32) << 16 | 1,
        );
        core::ptr::write_volatile(block.add(dual_core_index::STACK_TOP), stack_top);
        core::ptr::write_volatile(START_MAGIC, START_MAGIC_VALUE);
        core::ptr::write_volatile(PROC1_STACK, stack_top);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(PROC1_ENTRY, entry ^ PROC1_ENTRY_XOR);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let reset_ctrl = core::ptr::read_volatile(RESET_CTRL0);
        core::ptr::write_volatile(block.add(dual_core_index::RESET_CTRL_BEFORE), reset_ctrl);
        core::ptr::write_volatile(RESET_CTRL0, reset_ctrl & !PROC1_RESET_MASK);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        if !dual_core_wait_mask(RESET_DONE0, PROC1_RESET_MASK, PROC1_RESET_MASK, TIMEOUT_US) {
            core::ptr::write_volatile(
                block.add(dual_core_index::RESET_DONE_AFTER),
                core::ptr::read_volatile(RESET_DONE0),
            );
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_RESET_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }
        core::ptr::write_volatile(
            block.add(dual_core_index::RESET_DONE_AFTER),
            core::ptr::read_volatile(RESET_DONE0),
        );
        core::arch::asm!("dsb sy", "sev", options(nostack, preserves_flags));

        if !dual_core_wait_atomic(
            block,
            dual_core_index::ACK,
            DUAL_CORE_ACK_CALLBACK,
            TIMEOUT_US,
        ) {
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_CALLBACK_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }

        core::ptr::write_volatile(
            block.add(dual_core_index::PLAIN_WORD),
            DUAL_CORE_PLAIN_VALUE,
        );
        dual_core_atomic_store(block, dual_core_index::COMMAND, DUAL_CORE_CMD_PLAIN);
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));
        if !dual_core_wait_atomic(block, dual_core_index::ACK, DUAL_CORE_ACK_PLAIN, TIMEOUT_US) {
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_PLAIN_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }

        core::ptr::write_volatile(
            block.add(dual_core_index::PROC1_WORD),
            DUAL_CORE_PROC1_ORIGINAL,
        );
        dual_core_atomic_store(
            block,
            dual_core_index::COMMAND,
            DUAL_CORE_CMD_PROC1_EXCLUSIVE,
        );
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));
        if !dual_core_wait_atomic(
            block,
            dual_core_index::ACK,
            DUAL_CORE_ACK_PROC1_EXCLUSIVE,
            TIMEOUT_US,
        ) {
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_PROC1_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }

        let proc0_word = block.add(dual_core_index::PROC0_WORD);
        core::ptr::write_volatile(proc0_word, DUAL_CORE_PROC0_ORIGINAL);
        let (proc0_loaded, proc0_status) =
            dual_core_exclusive_roundtrip(proc0_word, DUAL_CORE_PROC0_REPLACEMENT);
        core::ptr::write_volatile(block.add(dual_core_index::PROC0_LOADED), proc0_loaded);
        core::ptr::write_volatile(block.add(dual_core_index::PROC0_STREX), proc0_status);
        core::ptr::write_volatile(
            block.add(dual_core_index::PROC0_FINAL),
            core::ptr::read_volatile(proc0_word),
        );

        let clrex_word = block.add(dual_core_index::CLREX_WORD);
        core::ptr::write_volatile(clrex_word, DUAL_CORE_CLREX_ORIGINAL);
        let (clrex_loaded, clrex_status) =
            dual_core_clrex_control(clrex_word, DUAL_CORE_CLREX_REPLACEMENT);
        core::ptr::write_volatile(block.add(dual_core_index::CLREX_LOADED), clrex_loaded);
        core::ptr::write_volatile(block.add(dual_core_index::CLREX_STREX), clrex_status);
        core::ptr::write_volatile(
            block.add(dual_core_index::CLREX_FINAL),
            core::ptr::read_volatile(clrex_word),
        );

        dual_core_atomic_store(block, dual_core_index::CROSS_WORD, DUAL_CORE_CROSS_ORIGINAL);
        dual_core_atomic_store(block, dual_core_index::CROSS_WRITE_DONE, 0);
        dual_core_atomic_store(block, dual_core_index::ACK, 0);
        dual_core_atomic_store(block, dual_core_index::COMMAND, DUAL_CORE_CMD_CROSS);
        core::arch::asm!("dmb sy", "sev", options(nostack, preserves_flags));
        if !dual_core_wait_atomic(
            block,
            dual_core_index::ACK,
            DUAL_CORE_ACK_CROSS_READY,
            TIMEOUT_US,
        ) {
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_CROSS_READY_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }

        let saved_primask: u32;
        let cross_loaded: u32;
        let cross_status: u32;
        let cross_seen: u32;
        let cross_remaining: u32;
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            "dmb sy",
            "ldrex {loaded}, [{address}]",
            "sev",
            "1:",
            "ldr {seen}, [{done}]",
            "cmp {seen}, {done_value}",
            "beq 2f",
            "subs {remaining}, {remaining}, #1",
            "bne 1b",
            "clrex",
            "mov {status}, #255",
            "b 3f",
            "2:",
            "strex {status}, {value}, [{address}]",
            "3:",
            "dmb sy",
            "msr PRIMASK, {saved}",
            saved = out(reg) saved_primask,
            loaded = out(reg) cross_loaded,
            status = out(reg) cross_status,
            seen = out(reg) cross_seen,
            remaining = inout(reg) CROSS_LOOP_LIMIT => cross_remaining,
            address = in(reg) block.add(dual_core_index::CROSS_WORD),
            done = in(reg) block.add(dual_core_index::CROSS_WRITE_DONE),
            done_value = in(reg) DUAL_CORE_CROSS_WRITE_DONE,
            value = in(reg) DUAL_CORE_CROSS_PROC0_VALUE,
            options(nostack)
        );
        let _ = saved_primask;
        core::ptr::write_volatile(block.add(dual_core_index::CROSS_LOADED), cross_loaded);
        core::ptr::write_volatile(block.add(dual_core_index::CROSS_STREX), cross_status);
        core::ptr::write_volatile(
            block.add(dual_core_index::CROSS_FINAL),
            dual_core_atomic_load(block, dual_core_index::CROSS_WORD),
        );
        core::ptr::write_volatile(block.add(dual_core_index::CROSS_REMAINING), cross_remaining);
        core::ptr::write_volatile(
            block.add(dual_core_index::ENTRY_AFTER),
            core::ptr::read_volatile(PROC1_ENTRY),
        );

        if cross_seen != DUAL_CORE_CROSS_WRITE_DONE {
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_CROSS_EVENT_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }
        if !dual_core_wait_atomic(
            block,
            dual_core_index::ACK,
            DUAL_CORE_ACK_CROSS_DONE,
            TIMEOUT_US,
        ) {
            core::ptr::write_volatile(
                block.add(dual_core_index::COMPLETION),
                DUAL_CORE_CROSS_EVENT_TIMEOUT,
            );
            dual_core_publish(block);
            return;
        }

        let mut flags = 0u32;
        if core::ptr::read_volatile(block.add(dual_core_index::PROC1_ARG)) == 1
            && core::ptr::read_volatile(block.add(dual_core_index::PROC1_CORE_ID)) == 1
        {
            flags |= 1 << 0;
        }
        if core::ptr::read_volatile(block.add(dual_core_index::PLAIN_SEEN)) == DUAL_CORE_PLAIN_VALUE
            && core::ptr::read_volatile(block.add(dual_core_index::PLAIN_REPLY))
                == (DUAL_CORE_PLAIN_VALUE ^ DUAL_CORE_PLAIN_XOR)
        {
            flags |= 1 << 1;
        }
        if core::ptr::read_volatile(block.add(dual_core_index::PROC1_LOADED))
            == DUAL_CORE_PROC1_ORIGINAL
            && core::ptr::read_volatile(block.add(dual_core_index::PROC1_STREX)) == 0
            && core::ptr::read_volatile(block.add(dual_core_index::PROC1_FINAL))
                == DUAL_CORE_PROC1_REPLACEMENT
        {
            flags |= 1 << 2;
        }
        if proc0_loaded == DUAL_CORE_PROC0_ORIGINAL
            && proc0_status == 0
            && core::ptr::read_volatile(block.add(dual_core_index::PROC0_FINAL))
                == DUAL_CORE_PROC0_REPLACEMENT
        {
            flags |= 1 << 3;
        }
        if clrex_loaded == DUAL_CORE_CLREX_ORIGINAL
            && clrex_status != 0
            && core::ptr::read_volatile(block.add(dual_core_index::CLREX_FINAL))
                == DUAL_CORE_CLREX_ORIGINAL
        {
            flags |= 1 << 4;
        }
        if dual_core_atomic_load(block, dual_core_index::ACK) == DUAL_CORE_ACK_CROSS_DONE
            && dual_core_atomic_load(block, dual_core_index::CROSS_WRITE_DONE)
                == DUAL_CORE_CROSS_WRITE_DONE
        {
            flags |= 1 << 5;
        }
        if cross_loaded == DUAL_CORE_CROSS_ORIGINAL
            && cross_status != 0
            && core::ptr::read_volatile(block.add(dual_core_index::CROSS_FINAL))
                == DUAL_CORE_CROSS_PROC1_VALUE
        {
            flags |= 1 << 6;
        }
        core::ptr::write_volatile(block.add(dual_core_index::FLAGS), flags);
        core::ptr::write_volatile(block.add(dual_core_index::COMPLETION), DUAL_CORE_DONE);
        dual_core_publish(block);
    }
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_WORDS: usize = 28;

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_MAILBOX_WORDS: usize = 16;

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_MAGIC: u32 = 0x314d_504c; // LPM1

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_DONE: u32 = 0x5353_4150; // PASS

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_ALIASED: u32 = 0x4149_4c41; // ALIA

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_RESET_TIMEOUT: u32 = 0x5452_4d4c; // LMRT

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_CALLBACK_TIMEOUT: u32 = 0x5443_4d4c; // LMCT

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_PHASE_A_TIMEOUT: u32 = 0x5441_4d4c; // LMAT

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_PHASE_B_TIMEOUT: u32 = 0x5442_4d4c; // LMBT

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_CMD_A: u32 = 0x4143_4f4c; // LOCA

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_CMD_B: u32 = 0x4243_4f4c; // LOCB

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_ACK_READY: u32 = 0x5944_524c; // LRDY

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_ACK_A: u32 = 0x414b_4f4c; // LOKA

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_ACK_B: u32 = 0x424b_4f4c; // LOKB

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_ISRAM_ADDR: *mut u32 = 0x1000_1ff0 as *mut u32;

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC_LOCAL_DSRAM_ADDR: *mut u32 = 0x1000_3ff0 as *mut u32;

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC0_ISRAM_A: u32 = 0x5349_3050; // P0IS

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC0_DSRAM_A: u32 = 0x5344_3050; // P0DS

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC0_ISRAM_B: u32 = 0x6949_3050; // P0Ii

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC0_DSRAM_B: u32 = 0x6444_3050; // P0Dd

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC1_ISRAM_A: u32 = 0x5349_3150; // P1IS

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
const PROC1_DSRAM_A: u32 = 0x5344_3150; // P1DS

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
mod proc_local_index {
    pub const FORMAT: usize = 1;
    pub const COMMAND: usize = 2;
    pub const ACK: usize = 3;
    pub const PROC1_ARG: usize = 4;
    pub const PROC1_CORE_ID: usize = 5;
    pub const RESET_CTRL_BEFORE: usize = 6;
    pub const RESET_DONE_AFTER: usize = 7;
    pub const STACK_TOP: usize = 8;
    pub const PROC0_ISRAM_SAVED: usize = 9;
    pub const PROC0_DSRAM_SAVED: usize = 10;
    pub const PROC1_ISRAM_SAVED: usize = 11;
    pub const PROC1_DSRAM_SAVED: usize = 12;
    pub const PROC1_ISRAM_AFTER_A: usize = 13;
    pub const PROC1_DSRAM_AFTER_A: usize = 14;
    pub const PROC0_ISRAM_AFTER_A: usize = 15;
    pub const PROC0_DSRAM_AFTER_A: usize = 16;
    pub const PROC1_ISRAM_AFTER_B: usize = 17;
    pub const PROC1_DSRAM_AFTER_B: usize = 18;
    pub const PROC0_ISRAM_AFTER_B: usize = 19;
    pub const PROC0_DSRAM_AFTER_B: usize = 20;
    pub const PROC1_ISRAM_RESTORED: usize = 21;
    pub const PROC1_DSRAM_RESTORED: usize = 22;
    pub const PROC0_ISRAM_RESTORED: usize = 23;
    pub const PROC0_DSRAM_RESTORED: usize = 24;
    pub const FLAGS: usize = 25;
    pub const COMPLETION: usize = 26;
    pub const ENTRY_AFTER: usize = 27;
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
#[repr(C, align(32))]
struct ProcLocalProofBlock([u32; PROC_LOCAL_WORDS]);

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
static mut PROC_LOCAL_BLOCK: ProcLocalProofBlock = ProcLocalProofBlock([0; PROC_LOCAL_WORDS]);

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
static mut PROC_LOCAL_STACK: [u64; 256] = [0; 256];

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
fn proc_local_block() -> *mut u32 {
    core::ptr::addr_of_mut!(PROC_LOCAL_BLOCK).cast::<u32>()
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
fn proc_local_stack_top() -> u32 {
    let base = core::ptr::addr_of!(PROC_LOCAL_STACK) as usize;
    (base + core::mem::size_of::<[u64; 256]>()) as u32
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
unsafe fn proc_local_load_acquire(block: *mut u32, index: usize) -> u32 {
    let value = unsafe { core::ptr::read_volatile(block.add(index)) };
    unsafe { core::arch::asm!("dmb sy", options(nostack, preserves_flags)) };
    value
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
unsafe fn proc_local_store_release(block: *mut u32, index: usize, value: u32) {
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(block.add(index), value);
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
unsafe fn proc_local_wait_value(
    block: *mut u32,
    index: usize,
    expected: u32,
    timeout_us: u64,
) -> bool {
    let start = raw_timer_us();
    while unsafe { proc_local_load_acquire(block, index) } != expected
        && raw_timer_us().wrapping_sub(start) <= timeout_us
    {
        core::hint::spin_loop();
    }
    unsafe { proc_local_load_acquire(block, index) == expected }
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
unsafe fn proc_local_wait_mask(
    address: *const u32,
    mask: u32,
    expected: u32,
    timeout_us: u64,
) -> bool {
    let start = raw_timer_us();
    while unsafe { core::ptr::read_volatile(address) } & mask != expected
        && raw_timer_us().wrapping_sub(start) <= timeout_us
    {
        core::hint::spin_loop();
    }
    unsafe { core::ptr::read_volatile(address) & mask == expected }
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Proc1LocalMemoryProof(core_id: u32, _arg1: u32, _arg2: u32) {
    const CORE_ID: *const u32 = 0xe00f_f01c as *const u32;
    const TIMEOUT_US: u64 = 100_000;
    let block = proc_local_block();
    let saved_primask: u32;

    unsafe {
        core::arch::asm!(
            "mrs {saved}, PRIMASK",
            "cpsid i",
            saved = out(reg) saved_primask,
            options(nostack, preserves_flags)
        );
        let isram_saved = core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR);
        let dsram_saved = core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR);
        core::ptr::write_volatile(block.add(proc_local_index::PROC1_ARG), core_id);
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_CORE_ID),
            core::ptr::read_volatile(CORE_ID),
        );
        core::ptr::write_volatile(block.add(proc_local_index::PROC1_ISRAM_SAVED), isram_saved);
        core::ptr::write_volatile(block.add(proc_local_index::PROC1_DSRAM_SAVED), dsram_saved);
        proc_local_store_release(block, proc_local_index::ACK, PROC_LOCAL_ACK_READY);
        core::arch::asm!("sev", options(nostack, preserves_flags));

        if !proc_local_wait_value(
            block,
            proc_local_index::COMMAND,
            PROC_LOCAL_CMD_A,
            TIMEOUT_US,
        ) {
            proc_local_store_release(
                block,
                proc_local_index::COMPLETION,
                PROC_LOCAL_PHASE_A_TIMEOUT,
            );
            core::arch::asm!("sev", options(nostack, preserves_flags));
            core::arch::asm!(
                "msr PRIMASK, {saved}",
                saved = in(reg) saved_primask,
                options(nostack, preserves_flags)
            );
            return;
        }

        core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, PROC1_ISRAM_A);
        core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, PROC1_DSRAM_A);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_ISRAM_AFTER_A),
            core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_DSRAM_AFTER_A),
            core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR),
        );
        proc_local_store_release(block, proc_local_index::ACK, PROC_LOCAL_ACK_A);
        core::arch::asm!("sev", options(nostack, preserves_flags));

        if !proc_local_wait_value(
            block,
            proc_local_index::COMMAND,
            PROC_LOCAL_CMD_B,
            TIMEOUT_US,
        ) {
            core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, isram_saved);
            core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, dsram_saved);
            proc_local_store_release(
                block,
                proc_local_index::COMPLETION,
                PROC_LOCAL_PHASE_B_TIMEOUT,
            );
            core::arch::asm!("sev", options(nostack, preserves_flags));
            core::arch::asm!(
                "msr PRIMASK, {saved}",
                saved = in(reg) saved_primask,
                options(nostack, preserves_flags)
            );
            return;
        }

        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_ISRAM_AFTER_B),
            core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_DSRAM_AFTER_B),
            core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR),
        );
        core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, isram_saved);
        core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, dsram_saved);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_ISRAM_RESTORED),
            core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC1_DSRAM_RESTORED),
            core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR),
        );
        proc_local_store_release(block, proc_local_index::ACK, PROC_LOCAL_ACK_B);
        core::arch::asm!("sev", options(nostack, preserves_flags));
        core::arch::asm!(
            "msr PRIMASK, {saved}",
            saved = in(reg) saved_primask,
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
unsafe fn proc_local_publish(block: *mut u32) {
    const _: () = assert!(
        PROC_LOCAL_MAILBOX_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE
    );
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        let mut restore_mask = 0u32;
        if core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_RESTORED))
            == core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_SAVED))
        {
            restore_mask |= 1 << 0;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_RESTORED))
            == core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_SAVED))
        {
            restore_mask |= 1 << 1;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_RESTORED))
            == core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_SAVED))
        {
            restore_mask |= 1 << 2;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_RESTORED))
            == core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_SAVED))
        {
            restore_mask |= 1 << 3;
        }
        let fields = [
            core::ptr::read_volatile(block),
            (PROC_LOCAL_MAILBOX_WORDS as u32) << 16 | 1,
            core::ptr::read_volatile(block.add(proc_local_index::FLAGS)),
            core::ptr::read_volatile(block.add(proc_local_index::COMPLETION)),
            (core::ptr::read_volatile(block.add(proc_local_index::PROC1_ARG)) & 0xffff)
                | ((core::ptr::read_volatile(block.add(proc_local_index::PROC1_CORE_ID)) & 0xffff)
                    << 16),
            core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_AFTER_A)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_AFTER_A)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_AFTER_A)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_AFTER_A)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_AFTER_B)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_AFTER_B)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_AFTER_B)),
            core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_AFTER_B)),
            restore_mask,
            core::ptr::read_volatile(block.add(proc_local_index::RESET_DONE_AFTER)),
            core::ptr::read_volatile(block.add(proc_local_index::ENTRY_AFTER)),
        ];
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "proc-local-memory-proof"))]
fn publish_proc_local_memory_proof() {
    const RESET_CTRL0: *mut u32 = 0x4001_4000 as *mut u32;
    const RESET_DONE0: *const u32 = 0x4001_4018 as *const u32;
    const PROC1_RESET_MASK: u32 = 1 << 31;
    const START_MAGIC: *mut u32 = 0x4015_400c as *mut u32;
    const PROC1_ENTRY: *mut u32 = 0x4015_4014 as *mut u32;
    const PROC1_STACK: *mut u32 = 0x4015_401c as *mut u32;
    const START_MAGIC_VALUE: u32 = 0xb007_c0de;
    const PROC1_ENTRY_XOR: u32 = 0x4ff8_3f2d;
    const TIMEOUT_US: u64 = 100_000;

    let block = proc_local_block();
    let stack_top = proc_local_stack_top();
    let entry = (Proc1LocalMemoryProof as *const () as usize as u32) | 1;
    unsafe {
        for index in 0..PROC_LOCAL_WORDS {
            core::ptr::write_volatile(block.add(index), 0);
        }
        core::ptr::write_volatile(block, PROC_LOCAL_MAGIC);
        core::ptr::write_volatile(
            block.add(proc_local_index::FORMAT),
            (PROC_LOCAL_WORDS as u32) << 16 | 1,
        );
        core::ptr::write_volatile(block.add(proc_local_index::STACK_TOP), stack_top);
        core::ptr::write_volatile(START_MAGIC, START_MAGIC_VALUE);
        core::ptr::write_volatile(PROC1_STACK, stack_top);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(PROC1_ENTRY, entry ^ PROC1_ENTRY_XOR);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let reset_ctrl = core::ptr::read_volatile(RESET_CTRL0);
        core::ptr::write_volatile(block.add(proc_local_index::RESET_CTRL_BEFORE), reset_ctrl);
        core::ptr::write_volatile(RESET_CTRL0, reset_ctrl & !PROC1_RESET_MASK);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        if !proc_local_wait_mask(RESET_DONE0, PROC1_RESET_MASK, PROC1_RESET_MASK, TIMEOUT_US) {
            core::ptr::write_volatile(
                block.add(proc_local_index::RESET_DONE_AFTER),
                core::ptr::read_volatile(RESET_DONE0),
            );
            core::ptr::write_volatile(
                block.add(proc_local_index::COMPLETION),
                PROC_LOCAL_RESET_TIMEOUT,
            );
            proc_local_publish(block);
            return;
        }
        core::ptr::write_volatile(
            block.add(proc_local_index::RESET_DONE_AFTER),
            core::ptr::read_volatile(RESET_DONE0),
        );
        core::arch::asm!("sev", options(nostack, preserves_flags));

        if !proc_local_wait_value(
            block,
            proc_local_index::ACK,
            PROC_LOCAL_ACK_READY,
            TIMEOUT_US,
        ) {
            core::ptr::write_volatile(
                block.add(proc_local_index::COMPLETION),
                PROC_LOCAL_CALLBACK_TIMEOUT,
            );
            proc_local_publish(block);
            return;
        }

        let proc0_isram_saved = core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR);
        let proc0_dsram_saved = core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR);
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_ISRAM_SAVED),
            proc0_isram_saved,
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_DSRAM_SAVED),
            proc0_dsram_saved,
        );
        core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, PROC0_ISRAM_A);
        core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, PROC0_DSRAM_A);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        proc_local_store_release(block, proc_local_index::COMMAND, PROC_LOCAL_CMD_A);
        core::arch::asm!("sev", options(nostack, preserves_flags));
        if !proc_local_wait_value(block, proc_local_index::ACK, PROC_LOCAL_ACK_A, TIMEOUT_US) {
            core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, proc0_isram_saved);
            core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, proc0_dsram_saved);
            core::ptr::write_volatile(
                block.add(proc_local_index::COMPLETION),
                PROC_LOCAL_PHASE_A_TIMEOUT,
            );
            proc_local_publish(block);
            return;
        }

        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_ISRAM_AFTER_A),
            core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_DSRAM_AFTER_A),
            core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR),
        );
        core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, PROC0_ISRAM_B);
        core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, PROC0_DSRAM_B);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        proc_local_store_release(block, proc_local_index::COMMAND, PROC_LOCAL_CMD_B);
        core::arch::asm!("sev", options(nostack, preserves_flags));
        if !proc_local_wait_value(block, proc_local_index::ACK, PROC_LOCAL_ACK_B, TIMEOUT_US) {
            core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, proc0_isram_saved);
            core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, proc0_dsram_saved);
            core::ptr::write_volatile(
                block.add(proc_local_index::COMPLETION),
                PROC_LOCAL_PHASE_B_TIMEOUT,
            );
            proc_local_publish(block);
            return;
        }

        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_ISRAM_AFTER_B),
            core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_DSRAM_AFTER_B),
            core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR),
        );
        core::ptr::write_volatile(PROC_LOCAL_ISRAM_ADDR, proc0_isram_saved);
        core::ptr::write_volatile(PROC_LOCAL_DSRAM_ADDR, proc0_dsram_saved);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_ISRAM_RESTORED),
            core::ptr::read_volatile(PROC_LOCAL_ISRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::PROC0_DSRAM_RESTORED),
            core::ptr::read_volatile(PROC_LOCAL_DSRAM_ADDR),
        );
        core::ptr::write_volatile(
            block.add(proc_local_index::ENTRY_AFTER),
            core::ptr::read_volatile(PROC1_ENTRY),
        );

        let mut flags = 0u32;
        if core::ptr::read_volatile(block.add(proc_local_index::PROC1_ARG)) == 1
            && core::ptr::read_volatile(block.add(proc_local_index::PROC1_CORE_ID)) == 1
        {
            flags |= 1 << 0;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_AFTER_A))
            == PROC1_ISRAM_A
            && core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_AFTER_A))
                == PROC1_DSRAM_A
        {
            flags |= 1 << 1;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_AFTER_A))
            == PROC0_ISRAM_A
            && core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_AFTER_A))
                == PROC0_DSRAM_A
        {
            flags |= 1 << 2;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_AFTER_B))
            == PROC1_ISRAM_A
            && core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_AFTER_B))
                == PROC1_DSRAM_A
        {
            flags |= 1 << 3;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_AFTER_B))
            == PROC0_ISRAM_B
            && core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_AFTER_B))
                == PROC0_DSRAM_B
        {
            flags |= 1 << 4;
        }
        if core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_RESTORED))
            == core::ptr::read_volatile(block.add(proc_local_index::PROC1_ISRAM_SAVED))
            && core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_RESTORED))
                == core::ptr::read_volatile(block.add(proc_local_index::PROC1_DSRAM_SAVED))
            && core::ptr::read_volatile(block.add(proc_local_index::PROC0_ISRAM_RESTORED))
                == proc0_isram_saved
            && core::ptr::read_volatile(block.add(proc_local_index::PROC0_DSRAM_RESTORED))
                == proc0_dsram_saved
        {
            flags |= 1 << 5;
        }
        core::ptr::write_volatile(block.add(proc_local_index::FLAGS), flags);
        core::ptr::write_volatile(
            block.add(proc_local_index::COMPLETION),
            if flags == 0x3f {
                PROC_LOCAL_DONE
            } else {
                PROC_LOCAL_ALIASED
            },
        );
        proc_local_publish(block);
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery-proof"))]
fn publish_expected_fault_recovery_proof() -> bool {
    const RESULT_MAGIC: u32 = 0x3152_4645; // EFR1
    const RESULT_WORDS: usize = 16;
    const EXPECTED_KIND_UDF: u32 = 1;
    const EXPECTED_RECOVERED: u32 = 0x5643_4552; // RECV
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let snapshot = unsafe { rp1_rt::run_expected_udf_recovery() };
    let mut flags = 0u32;
    if snapshot.active == 0 {
        flags |= 1 << 0;
    }
    if snapshot.sequence == 1 {
        flags |= 1 << 1;
    }
    if snapshot.kind == EXPECTED_KIND_UDF {
        flags |= 1 << 2;
    }
    if snapshot.exception == 3 || snapshot.exception == 6 {
        flags |= 1 << 3;
    }
    if snapshot.cfsr & (1 << 16) != 0 {
        flags |= 1 << 4;
    }
    if snapshot.exception == 6 || snapshot.hfsr & (1 << 30) != 0 {
        flags |= 1 << 5;
    }
    if snapshot.probe_pc == snapshot.stacked_pc {
        flags |= 1 << 6;
    }
    if snapshot.resume_pc != snapshot.probe_pc
        && (0x2000_0000..0x2001_0000).contains(&(snapshot.resume_pc as usize))
    {
        flags |= 1 << 7;
    }
    if snapshot.completion == EXPECTED_RECOVERED {
        flags |= 1 << 8;
    }
    if snapshot.handler_count == 1 {
        flags |= 1 << 9;
    }
    if snapshot.recovered() {
        flags |= 1 << 10;
    }

    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        flags,
        snapshot.sequence,
        snapshot.kind,
        snapshot.exception,
        snapshot.exc_return,
        snapshot.probe_pc,
        snapshot.resume_pc,
        snapshot.stacked_pc,
        snapshot.stacked_lr,
        snapshot.stacked_xpsr,
        snapshot.cfsr,
        snapshot.hfsr,
        snapshot.handler_count,
        snapshot.completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    snapshot.recovered()
}

#[cfg(all(target_arch = "arm", feature = "rp1-axi-dmac-identity-readonly-proof"))]
fn publish_rp1_axi_dmac_identity_readonly_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"DMID");
    const RESULT_WORDS: usize = 16;
    const ADDRESSES: [u32; 8] = [
        0x4001_8014, // CLK_SYS_CTRL
        0x4001_8018, // CLK_SYS_DIV_INT
        0x4001_8020, // CLK_SYS_SEL
        0x4001_8044, // CLK_DMA_CTRL
        0x4001_8048, // CLK_DMA_DIV_INT
        0x4001_8050, // CLK_DMA_SEL
        0x4018_8000, // DMAC_ID
        0x4018_8008, // DMAC_COMPVER
    ];
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let mut values = [0u32; ADDRESSES.len()];
    let mut success_mask = 0u32;
    let mut fault_mask = 0u32;
    let mut unexpected_mask = 0u32;
    let mut identity_diagnostics = [[0u32; 5]; 2];

    for (index, address) in ADDRESSES.into_iter().enumerate() {
        let snapshot = unsafe { rp1_rt::run_expected_data_read(address as usize as *const u32) };
        values[index] = snapshot.access_result;
        if snapshot.data_read_succeeded() {
            success_mask |= 1 << index;
        } else if snapshot.recovered_data_read_fault(address) {
            fault_mask |= 1 << index;
        } else {
            unexpected_mask |= 1 << index;
        }
        if index >= 6 {
            identity_diagnostics[index - 6] = [
                snapshot.exception,
                snapshot.cfsr,
                snapshot.hfsr,
                snapshot.bfar,
                snapshot.faulted,
            ];
        }
    }

    let outcomes = success_mask | fault_mask << 8 | unexpected_mask << 16;
    let identity_status = (identity_diagnostics[0][0] & 0xff)
        | (identity_diagnostics[1][0] & 0xff) << 8
        | ((identity_diagnostics[0][2] >> 30) & 0x3) << 16
        | ((identity_diagnostics[1][2] >> 30) & 0x3) << 18
        | (identity_diagnostics[0][4] & 0x1) << 20
        | (identity_diagnostics[1][4] & 0x1) << 21;
    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        outcomes,
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        values[5],
        values[6],
        values[7],
        identity_diagnostics[0][1],
        identity_diagnostics[0][3],
        identity_diagnostics[1][1],
        identity_diagnostics[1][3],
        identity_status,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    if success_mask == (1 << ADDRESSES.len()) - 1 {
        u32::from_le_bytes(*b"READ")
    } else if fault_mask != 0 && unexpected_mask == 0 {
        u32::from_le_bytes(*b"PFLT")
    } else {
        u32::from_le_bytes(*b"UNXP")
    }
}

#[cfg(all(
    target_arch = "arm",
    feature = "rp1-axi-dmac-reset-state-readonly-proof"
))]
fn publish_rp1_axi_dmac_reset_state_readonly_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"DMRS");
    const RESULT_WORDS: usize = 16;
    const ADDRESSES: [u32; 11] = [
        0x4001_4000, // RESET_CTRL0
        0x4001_4004, // RESET_CTRL1
        0x4001_4008, // RESET_CTRL2
        0x4001_4018, // RESET_DONE0
        0x4001_401c, // RESET_DONE1
        0x4001_4020, // RESET_DONE2
        0x4018_8000, // DMAC_ID
        0x4018_8008, // DMAC_COMPVER
        0x4018_8010, // DMAC_CFG
        0x4018_8058, // DMAC_RESET
        0x4001_8044, // CLK_DMA_CTRL
    ];
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let mut values = [0u32; ADDRESSES.len()];
    let mut success_mask = 0u32;
    let mut fault_mask = 0u32;
    let mut unexpected_mask = 0u32;
    for (index, address) in ADDRESSES.into_iter().enumerate() {
        let snapshot = unsafe { rp1_rt::run_expected_data_read(address as usize as *const u32) };
        values[index] = snapshot.access_result;
        if snapshot.data_read_succeeded() {
            success_mask |= 1 << index;
        } else if snapshot.recovered_data_read_fault(address) {
            fault_mask |= 1 << index;
        } else {
            unexpected_mask |= 1 << index;
        }
    }

    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        success_mask,
        fault_mask,
        unexpected_mask,
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        values[5],
        values[6],
        values[7],
        values[8],
        values[9],
        values[10],
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    if success_mask == (1 << ADDRESSES.len()) - 1 {
        u32::from_le_bytes(*b"READ")
    } else if fault_mask != 0 && unexpected_mask == 0 {
        u32::from_le_bytes(*b"PFLT")
    } else {
        u32::from_le_bytes(*b"UNXP")
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "rp1-axi-dmac-active-identity-proof",
        feature = "rp1-axi-dmac-internal-reset-identity-proof",
        feature = "rp1-axi-dmac-global-enable-identity-proof"
    )
))]
fn publish_rp1_axi_dmac_active_identity_proof(
    pll_sys: PllSysCoreLockResult,
    internal_reset: bool,
    enable_dmac: bool,
) -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"DMAA");
    const RESULT_WORDS: usize = 16;
    const PLL_SYS_PRIM: *mut u32 = 0x4002_0010 as *mut u32;
    const CLK_DMA_CTRL: *mut u32 = 0x4001_8044 as *mut u32;
    const CLK_DMA_DIV_INT: *const u32 = 0x4001_8048 as *const u32;
    const CLK_DMA_SEL: *const u32 = 0x4001_8050 as *const u32;
    const DMAC_ID: *const u32 = 0x4018_8000 as *const u32;
    const DMAC_COMPVER: *const u32 = 0x4018_8008 as *const u32;
    const DMAC_CFG: *mut u32 = 0x4018_8010 as *mut u32;
    const DMAC_CHEN: *const u32 = 0x4018_8018 as *const u32;
    const DMAC_RESET: *mut u32 = 0x4018_8058 as *mut u32;
    const DMAC_CFG_ENABLE: u32 = 1;
    const PRIM_DISABLED_100MHZ_PARENT: u32 = 0x0005_1000;
    const PRIM_ENABLED_100MHZ_PARENT: u32 = 0x0005_1010;
    const CLK_ENABLE: u32 = 1 << 11;
    const CLK_ENABLED_STATUS: u32 = 1 << 28;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let internal_reset = internal_reset || enable_dmac;
    let pre_ctrl = unsafe { core::ptr::read_volatile(CLK_DMA_CTRL) };
    let pre_div = unsafe { core::ptr::read_volatile(CLK_DMA_DIV_INT) };
    let pre_sel = unsafe { core::ptr::read_volatile(CLK_DMA_SEL) };
    let pre_prim = unsafe { core::ptr::read_volatile(PLL_SYS_PRIM) };
    let mut status = 0u32;
    let mut active_prim = pre_prim;
    let mut active_ctrl = pre_ctrl;
    let mut id_value = 0u32;
    let mut compver_value = 0u32;
    let mut id_cfsr = 0u32;
    let mut compver_cfsr = 0u32;
    let mut reset_before = 0xffu32;
    let mut reset_after = 0xffu32;
    let mut reset_polls = 0u32;
    let mut cfg_before = u32::MAX;
    let mut cfg_active = u32::MAX;
    let mut cfg_after_reset = u32::MAX;

    let core_exact = pll_sys.decision == PllSysCoreLockDecision::Locked
        && pll_sys.after.cs == 0x8000_0001
        && pll_sys.after.pwr == 0x0000_0004
        && pll_sys.after.fbdiv_int == 20
        && pll_sys.after.fbdiv_frac == 0
        && pll_sys.after.prim == 0x0007_7000
        && pll_sys.after.sec == 0x8001_0000;
    if core_exact {
        status |= 1 << 0;
    }

    let precondition_exact =
        core_exact && pre_ctrl == 0 && pre_div == 1 && pre_sel == 1 && pre_prim == 0x0007_7000;
    if precondition_exact {
        status |= 1 << 1;
        unsafe {
            core::ptr::write_volatile(PLL_SYS_PRIM, PRIM_DISABLED_100MHZ_PARENT);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            active_prim = core::ptr::read_volatile(PLL_SYS_PRIM);
        }
        if active_prim == PRIM_DISABLED_100MHZ_PARENT {
            status |= 1 << 2;
            unsafe {
                core::ptr::write_volatile(PLL_SYS_PRIM, PRIM_ENABLED_100MHZ_PARENT);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                active_prim = core::ptr::read_volatile(PLL_SYS_PRIM);
            }
        }
        if active_prim == PRIM_ENABLED_100MHZ_PARENT {
            status |= 1 << 3;
            unsafe {
                core::ptr::write_volatile(CLK_DMA_CTRL, pre_ctrl | CLK_ENABLE);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                active_ctrl = core::ptr::read_volatile(CLK_DMA_CTRL);
            }
        }
        if active_ctrl == (pre_ctrl | CLK_ENABLE | CLK_ENABLED_STATUS) {
            status |= 1 << 4;
        }
        if enable_dmac && status & (1 << 4) != 0 {
            let chen_before;
            unsafe {
                cfg_before = core::ptr::read_volatile(DMAC_CFG);
                chen_before = core::ptr::read_volatile(DMAC_CHEN);
            }
            if cfg_before == 0 && chen_before == 0 {
                status |= 1 << 14;
                unsafe {
                    core::ptr::write_volatile(DMAC_CFG, cfg_before | DMAC_CFG_ENABLE);
                    core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                    cfg_active = core::ptr::read_volatile(DMAC_CFG);
                }
                if cfg_active == DMAC_CFG_ENABLE {
                    status |= 1 << 15;
                }
            }
        }
        if status & (1 << 4) != 0 && internal_reset && (!enable_dmac || status & (1 << 15) != 0) {
            reset_before = unsafe { core::ptr::read_volatile(DMAC_RESET) };
            if reset_before == 0 {
                status |= 1 << 12;
                unsafe {
                    core::ptr::write_volatile(DMAC_RESET, 1);
                    core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                    reset_after = core::ptr::read_volatile(DMAC_RESET);
                }
                while reset_after != 0 && reset_polls < 1_000 {
                    reset_polls += 1;
                    core::hint::spin_loop();
                    reset_after = unsafe { core::ptr::read_volatile(DMAC_RESET) };
                }
                if reset_after == 0 {
                    status |= 1 << 13;
                }
            }
        }
        if enable_dmac && status & (3 << 12) == (3 << 12) {
            cfg_after_reset = unsafe { core::ptr::read_volatile(DMAC_CFG) };
            if cfg_after_reset == DMAC_CFG_ENABLE {
                status |= 1 << 17;
            }
        }
        let identity_ready = status & (1 << 4) != 0
            && (!internal_reset || status & (3 << 12) == (3 << 12))
            && (!enable_dmac || status & (1 << 17) != 0);
        if identity_ready {
            let id = unsafe { rp1_rt::run_expected_data_read(DMAC_ID) };
            let compver = unsafe { rp1_rt::run_expected_data_read(DMAC_COMPVER) };
            id_value = id.access_result;
            compver_value = compver.access_result;
            id_cfsr = id.cfsr;
            compver_cfsr = compver.cfsr;
            if id.data_read_succeeded() {
                status |= 1 << 5;
            } else if id.recovered_data_read_fault(DMAC_ID as u32) {
                status |= 1 << 7;
            }
            if compver.data_read_succeeded() {
                status |= 1 << 6;
            } else if compver.recovered_data_read_fault(DMAC_COMPVER as u32) {
                status |= 1 << 8;
            }
        }
        if enable_dmac && status & (1 << 14) != 0 {
            let cfg_restored;
            unsafe {
                core::ptr::write_volatile(DMAC_CFG, cfg_before);
                core::arch::asm!("dsb sy", options(nostack, preserves_flags));
                cfg_restored = core::ptr::read_volatile(DMAC_CFG);
            }
            if cfg_restored == cfg_before {
                status |= 1 << 16;
            }
        }
    }

    let (restored_ctrl, restored_prim) = if precondition_exact {
        unsafe {
            core::ptr::write_volatile(CLK_DMA_CTRL, pre_ctrl);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        let ctrl = unsafe { core::ptr::read_volatile(CLK_DMA_CTRL) };
        if ctrl == pre_ctrl {
            status |= 1 << 9;
        }
        unsafe {
            core::ptr::write_volatile(PLL_SYS_PRIM, pre_prim);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        let prim = unsafe { core::ptr::read_volatile(PLL_SYS_PRIM) };
        if prim == pre_prim {
            status |= 1 << 10;
        }
        if status & (3 << 9) == (3 << 9) {
            status |= 1 << 11;
        }
        (ctrl, prim)
    } else {
        unsafe {
            (
                core::ptr::read_volatile(CLK_DMA_CTRL),
                core::ptr::read_volatile(PLL_SYS_PRIM),
            )
        }
    };

    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        status,
        pre_ctrl,
        pre_div,
        pre_sel,
        pll_sys.after.cs,
        active_prim,
        active_ctrl,
        id_value,
        compver_value,
        id_cfsr,
        compver_cfsr,
        if enable_dmac {
            cfg_before
        } else {
            restored_ctrl
        },
        if enable_dmac {
            cfg_active
        } else {
            restored_prim
        },
        if enable_dmac {
            cfg_after_reset
        } else if internal_reset {
            (reset_after & 0xff) << 24 | (reset_before & 0xff) << 16 | reset_polls.min(0xffff)
        } else {
            pll_sys.elapsed_us
        },
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(
            mailbox,
            if enable_dmac {
                u32::from_le_bytes(*b"DMAG")
            } else if internal_reset {
                u32::from_le_bytes(*b"DMAR")
            } else {
                RESULT_MAGIC
            },
        );
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    let restored = status & (3 << 9) == (3 << 9);
    let reset_exact = !internal_reset || status & (3 << 12) == (3 << 12);
    let cfg_exact = !enable_dmac || status & (15 << 14) == (15 << 14);
    if precondition_exact && restored && reset_exact && cfg_exact && status & (3 << 5) == (3 << 5) {
        if id_value != 0 || compver_value != 0 {
            u32::from_le_bytes(*b"DATA")
        } else {
            u32::from_le_bytes(*b"ZERO")
        }
    } else if precondition_exact && restored && reset_exact && cfg_exact && status & (3 << 7) != 0 {
        u32::from_le_bytes(*b"PFLT")
    } else {
        u32::from_le_bytes(*b"FAIL")
    }
}

#[cfg(all(target_arch = "arm", feature = "shared-sram-bitband-proof"))]
static mut SHARED_SRAM_BITBAND_TEST_WORD: u32 = 0;

#[cfg(all(target_arch = "arm", feature = "shared-sram-bitband-proof"))]
fn publish_shared_sram_bitband_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"BBA1");
    const RESULT_WORDS: usize = 16;
    const TEST_PATTERN: u32 = 0xa55a_5aa4;
    const RESULT_PASS: u32 = u32::from_le_bytes(*b"PASS");
    const RESULT_NOT_SUPPORTED: u32 = u32::from_le_bytes(*b"NSUP");
    const RESULT_NOT_BITBAND: u32 = u32::from_le_bytes(*b"NBIT");
    const RESULT_PARTIAL: u32 = u32::from_le_bytes(*b"PART");
    const RESULT_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let base = core::ptr::addr_of_mut!(SHARED_SRAM_BITBAND_TEST_WORD);
    let base_address = base as usize as u32;
    let alias_address =
        0x2200_0000u32.wrapping_add(base_address.wrapping_sub(0x2000_0000).wrapping_mul(32));
    let alias = alias_address as usize as *mut u32;
    let original = unsafe { core::ptr::read_volatile(base) };
    let mut flags = 0u32;
    let mut read_zero = u32::MAX;
    let mut after_set = u32::MAX;
    let mut read_one = u32::MAX;
    let mut after_clear = u32::MAX;
    let mut fault_info = 0u32;
    let mut fault_cfsr = 0u32;
    let mut fault_bfar = 0u32;
    let mut completion = RESULT_FAIL;
    let mut proceed = true;

    if (0x2000_0000..0x2001_0000).contains(&base_address) {
        flags |= 1 << 0;
    }
    if (0x2200_0000..0x2400_0000).contains(&alias_address) {
        flags |= 1 << 1;
    }

    unsafe {
        core::ptr::write_volatile(base, TEST_PATTERN);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    if unsafe { core::ptr::read_volatile(base) } == TEST_PATTERN {
        flags |= 1 << 2;
    } else {
        proceed = false;
    }

    if proceed {
        let snapshot = unsafe { rp1_rt::run_expected_data_read(alias.cast_const()) };
        if snapshot.data_read_succeeded() {
            read_zero = snapshot.access_result;
            flags |= 1 << 11;
            if read_zero == 0 {
                flags |= 1 << 3;
            } else {
                completion = RESULT_NOT_BITBAND;
                proceed = false;
            }
        } else if snapshot.recovered_data_read_fault(alias_address) {
            flags |= 1 << 10;
            fault_info = 1
                | (snapshot.kind << 8)
                | (snapshot.exception << 16)
                | (snapshot.handler_count << 24);
            fault_cfsr = snapshot.cfsr;
            fault_bfar = snapshot.bfar;
            completion = RESULT_NOT_SUPPORTED;
            proceed = false;
        } else {
            proceed = false;
        }
    }

    if proceed {
        let snapshot = unsafe { rp1_rt::run_expected_data_write(alias, 1) };
        if snapshot.data_write_succeeded() {
            flags |= 1 << 4;
        } else if snapshot.recovered_data_write_fault(alias_address) {
            flags |= 1 << 10;
            fault_info = 2
                | (snapshot.kind << 8)
                | (snapshot.exception << 16)
                | (snapshot.handler_count << 24);
            fault_cfsr = snapshot.cfsr;
            fault_bfar = snapshot.bfar;
            completion = RESULT_PARTIAL;
            proceed = false;
        } else {
            proceed = false;
        }
    }

    if proceed {
        after_set = unsafe { core::ptr::read_volatile(base) };
        if after_set == TEST_PATTERN | 1 {
            flags |= 1 << 5;
        } else {
            proceed = false;
        }
    }

    if proceed {
        let snapshot = unsafe { rp1_rt::run_expected_data_read(alias.cast_const()) };
        if snapshot.data_read_succeeded() && snapshot.access_result == 1 {
            read_one = snapshot.access_result;
            flags |= 1 << 6;
        } else if snapshot.recovered_data_read_fault(alias_address) {
            flags |= 1 << 10;
            fault_info = 3
                | (snapshot.kind << 8)
                | (snapshot.exception << 16)
                | (snapshot.handler_count << 24);
            fault_cfsr = snapshot.cfsr;
            fault_bfar = snapshot.bfar;
            completion = RESULT_PARTIAL;
            proceed = false;
        } else {
            proceed = false;
        }
    }

    if proceed {
        let snapshot = unsafe { rp1_rt::run_expected_data_write(alias, 0) };
        if snapshot.data_write_succeeded() {
            flags |= 1 << 7;
        } else if snapshot.recovered_data_write_fault(alias_address) {
            flags |= 1 << 10;
            fault_info = 4
                | (snapshot.kind << 8)
                | (snapshot.exception << 16)
                | (snapshot.handler_count << 24);
            fault_cfsr = snapshot.cfsr;
            fault_bfar = snapshot.bfar;
            completion = RESULT_PARTIAL;
            proceed = false;
        } else {
            proceed = false;
        }
    }

    if proceed {
        after_clear = unsafe { core::ptr::read_volatile(base) };
        if after_clear == TEST_PATTERN {
            flags |= 1 << 8;
            completion = RESULT_PASS;
        }
    }

    unsafe {
        core::ptr::write_volatile(base, original);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let restored = unsafe { core::ptr::read_volatile(base) };
    if restored == original {
        flags |= 1 << 9;
    } else {
        completion = RESULT_FAIL;
    }

    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        flags,
        base_address,
        alias_address,
        original,
        TEST_PATTERN,
        read_zero,
        after_set,
        read_one,
        after_clear,
        restored,
        fault_info,
        fault_cfsr,
        fault_bfar,
        completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    completion
}

#[cfg(all(target_arch = "arm", feature = "internal-memory-boundary-read-proof"))]
fn publish_internal_memory_boundary_read_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"IMB1");
    const RESULT_WORDS: usize = 16;
    const TARGET_ADDRESS: u32 = 0x2001_0000;
    const RESULT_READABLE: u32 = u32::from_le_bytes(*b"READ");
    const RESULT_PRECISE_FAULT: u32 = u32::from_le_bytes(*b"PFLT");
    const RESULT_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let snapshot = unsafe { rp1_rt::run_expected_data_read(TARGET_ADDRESS as *const u32) };
    let mut flags = 1u32;
    if snapshot.target_address == TARGET_ADDRESS {
        flags |= 1 << 1;
    }

    let completion = if snapshot.data_read_succeeded() && snapshot.target_address == TARGET_ADDRESS
    {
        flags |= (1 << 2) | (1 << 4);
        RESULT_READABLE
    } else if snapshot.recovered_data_read_fault(TARGET_ADDRESS) {
        flags |= (1 << 3) | (1 << 4);
        RESULT_PRECISE_FAULT
    } else {
        RESULT_FAIL
    };

    let fault_info = snapshot.kind
        | (snapshot.exception << 8)
        | (snapshot.handler_count << 16)
        | (snapshot.faulted << 24);
    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        flags,
        TARGET_ADDRESS,
        snapshot.access_result,
        fault_info,
        snapshot.sequence,
        snapshot.probe_pc,
        snapshot.resume_pc,
        snapshot.stacked_pc,
        snapshot.cfsr,
        snapshot.hfsr,
        snapshot.bfar,
        snapshot.mmfar,
        snapshot.target_address,
        completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    completion
}

#[cfg(all(target_arch = "arm", feature = "shared-sram-64k-mirror-readonly-proof"))]
fn publish_shared_sram_64k_mirror_readonly_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"M64K");
    const RESULT_WORDS: usize = 16;
    const BASE: u32 = 0x2000_0000;
    const MIRROR: u32 = 0x2001_0000;
    const OFFSETS: [u32; 4] = [0, 0x100, 0x2000, 0x4000];
    const RESULT_PASS: u32 = u32::from_le_bytes(*b"PASS");
    const RESULT_DIFFERENT: u32 = u32::from_le_bytes(*b"DIFF");
    const RESULT_PRECISE_FAULT: u32 = u32::from_le_bytes(*b"PFLT");
    const RESULT_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let mut flags = 1u32;
    let mut base_values = [u32::MAX; 4];
    let mut mirror_values = [u32::MAX; 4];
    let mut fault: Option<(u32, rp1_rt::ExpectedFaultSnapshot, bool)> = None;

    for (index, offset) in OFFSETS.into_iter().enumerate() {
        let address = BASE + offset;
        let snapshot = unsafe { rp1_rt::run_expected_data_read(address as *const u32) };
        if snapshot.data_read_succeeded() && snapshot.target_address == address {
            base_values[index] = snapshot.access_result;
        } else {
            let exact = snapshot.recovered_data_read_fault(address);
            fault = Some((index as u32 + 1, snapshot, exact));
            break;
        }
    }
    if fault.is_none() {
        flags |= 1 << 1;
        for (index, offset) in OFFSETS.into_iter().enumerate() {
            let address = MIRROR + offset;
            let snapshot = unsafe { rp1_rt::run_expected_data_read(address as *const u32) };
            if snapshot.data_read_succeeded() && snapshot.target_address == address {
                mirror_values[index] = snapshot.access_result;
            } else {
                let exact = snapshot.recovered_data_read_fault(address);
                fault = Some((index as u32 + 5, snapshot, exact));
                break;
            }
        }
    }
    if fault.is_none() {
        flags |= 1 << 2;
    }

    let mut payload = [u32::MAX; 8];
    let mut fault_info = 0u32;
    let completion = if let Some((stage, snapshot, exact)) = fault {
        fault_info = stage
            | (snapshot.kind << 8)
            | (snapshot.exception << 16)
            | (snapshot.handler_count << 24);
        payload = [
            snapshot.target_address,
            snapshot.cfsr,
            snapshot.hfsr,
            snapshot.bfar,
            snapshot.mmfar,
            snapshot.probe_pc,
            snapshot.resume_pc,
            snapshot.stacked_pc,
        ];
        if exact {
            flags |= 1 << 8;
            RESULT_PRECISE_FAULT
        } else {
            RESULT_FAIL
        }
    } else {
        for index in 0..4 {
            payload[index * 2] = base_values[index];
            payload[index * 2 + 1] = mirror_values[index];
        }
        let all_equal = base_values == mirror_values;
        let diverse = base_values.iter().any(|value| *value != base_values[0]);
        if all_equal {
            flags |= 1 << 3;
        }
        if diverse {
            flags |= 1 << 4;
        }
        if all_equal && diverse {
            RESULT_PASS
        } else {
            RESULT_DIFFERENT
        }
    };

    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        flags,
        OFFSETS.len() as u32,
        BASE,
        MIRROR,
        payload[0],
        payload[1],
        payload[2],
        payload[3],
        payload[4],
        payload[5],
        payload[6],
        payload[7],
        fault_info,
        completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    completion
}

#[cfg(all(
    target_arch = "arm",
    feature = "shared-sram-alias-window-extent-readonly-proof"
))]
fn publish_shared_sram_alias_window_extent_readonly_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"AEXT");
    const RESULT_WORDS: usize = 16;
    const BASE: u32 = 0x2000_0100;
    const ADDRESS_BITS: [u32; 9] = [16, 17, 18, 19, 20, 21, 22, 23, 24];
    const STATUS_EQUAL: u32 = 0;
    const STATUS_DIFFERENT: u32 = 1;
    const STATUS_PRECISE_FAULT: u32 = 2;
    const STATUS_UNEXPECTED: u32 = 3;
    const BASE_STATUS_SHIFT: u32 = 28;
    const SEQUENCE_COMPLETE: u32 = 1 << 30;
    const RESULT_DONE: u32 = u32::from_le_bytes(*b"DONE");
    const RESULT_PRECISE_FAULT: u32 = u32::from_le_bytes(*b"PFLT");
    const RESULT_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let base_snapshot = unsafe { rp1_rt::run_expected_data_read(BASE as *const u32) };
    let base_succeeded =
        base_snapshot.data_read_succeeded() && base_snapshot.target_address == BASE;
    let base_status = if base_succeeded {
        STATUS_EQUAL
    } else if base_snapshot.recovered_data_read_fault(BASE) {
        STATUS_PRECISE_FAULT
    } else {
        STATUS_UNEXPECTED
    };
    let base_value = if base_succeeded {
        base_snapshot.access_result
    } else {
        u32::MAX
    };

    let mut status_word = base_status << BASE_STATUS_SHIFT;
    let mut candidate_values = [u32::MAX; 9];
    let mut attempted = 0usize;
    let mut unexpected = base_status == STATUS_UNEXPECTED;
    if base_succeeded {
        for (index, address_bit) in ADDRESS_BITS.into_iter().enumerate() {
            let address = BASE | (1 << address_bit);
            let snapshot = unsafe { rp1_rt::run_expected_data_read(address as *const u32) };
            let status = if snapshot.data_read_succeeded() && snapshot.target_address == address {
                candidate_values[index] = snapshot.access_result;
                if snapshot.access_result == base_value {
                    STATUS_EQUAL
                } else {
                    STATUS_DIFFERENT
                }
            } else if snapshot.recovered_data_read_fault(address) {
                STATUS_PRECISE_FAULT
            } else {
                unexpected = true;
                STATUS_UNEXPECTED
            };
            status_word |= status << (index * 2);
            attempted += 1;
            if unexpected {
                break;
            }
        }
    }
    if attempted == ADDRESS_BITS.len() {
        status_word |= SEQUENCE_COMPLETE;
    }

    let completion = if unexpected {
        RESULT_FAIL
    } else if base_status == STATUS_PRECISE_FAULT {
        RESULT_PRECISE_FAULT
    } else if attempted == ADDRESS_BITS.len() {
        RESULT_DONE
    } else {
        RESULT_FAIL
    };
    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        status_word,
        ADDRESS_BITS.len() as u32,
        BASE,
        base_value,
        candidate_values[0],
        candidate_values[1],
        candidate_values[2],
        candidate_values[3],
        candidate_values[4],
        candidate_values[5],
        candidate_values[6],
        candidate_values[7],
        candidate_values[8],
        completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    completion
}

#[cfg(all(
    target_arch = "arm",
    feature = "shared-sram-system-region-alias-readonly-proof"
))]
fn publish_shared_sram_system_region_alias_readonly_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"SREG");
    const RESULT_WORDS: usize = 16;
    const BASE: u32 = 0x2000_0100;
    const CANDIDATES: [u32; 5] = [
        0x2200_0100,
        0x2400_0100,
        0x2800_0100,
        0x3000_0100,
        0x3fff_0100,
    ];
    const STATUS_EQUAL: u32 = 0;
    const STATUS_DIFFERENT: u32 = 1;
    const STATUS_PRECISE_FAULT: u32 = 2;
    const STATUS_UNEXPECTED: u32 = 3;
    const BASE_STATUS_SHIFT: u32 = 28;
    const SEQUENCE_COMPLETE: u32 = 1 << 30;
    const RESULT_DONE: u32 = u32::from_le_bytes(*b"DONE");
    const RESULT_PRECISE_FAULT: u32 = u32::from_le_bytes(*b"PFLT");
    const RESULT_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let base_snapshot = unsafe { rp1_rt::run_expected_data_read(BASE as *const u32) };
    let base_succeeded =
        base_snapshot.data_read_succeeded() && base_snapshot.target_address == BASE;
    let base_status = if base_succeeded {
        STATUS_EQUAL
    } else if base_snapshot.recovered_data_read_fault(BASE) {
        STATUS_PRECISE_FAULT
    } else {
        STATUS_UNEXPECTED
    };
    let base_value = if base_succeeded {
        base_snapshot.access_result
    } else {
        u32::MAX
    };

    let mut status_word = base_status << BASE_STATUS_SHIFT;
    let mut candidate_values = [u32::MAX; 5];
    let mut fault_words = [0u32; 4];
    let mut attempted = 0usize;
    let mut unexpected = base_status == STATUS_UNEXPECTED;
    if !base_succeeded {
        fault_words = [
            base_snapshot.target_address,
            base_snapshot.cfsr,
            base_snapshot.hfsr,
            base_snapshot.bfar,
        ];
    } else {
        for (index, address) in CANDIDATES.into_iter().enumerate() {
            let snapshot = unsafe { rp1_rt::run_expected_data_read(address as *const u32) };
            let status = if snapshot.data_read_succeeded() && snapshot.target_address == address {
                candidate_values[index] = snapshot.access_result;
                if snapshot.access_result == base_value {
                    STATUS_EQUAL
                } else {
                    STATUS_DIFFERENT
                }
            } else if snapshot.recovered_data_read_fault(address) {
                if fault_words[0] == 0 {
                    fault_words = [address, snapshot.cfsr, snapshot.hfsr, snapshot.bfar];
                }
                STATUS_PRECISE_FAULT
            } else {
                fault_words = [
                    snapshot.target_address,
                    snapshot.cfsr,
                    snapshot.hfsr,
                    snapshot.bfar,
                ];
                unexpected = true;
                STATUS_UNEXPECTED
            };
            status_word |= status << (index * 2);
            attempted += 1;
            if unexpected {
                break;
            }
        }
    }
    if attempted == CANDIDATES.len() {
        status_word |= SEQUENCE_COMPLETE;
    }

    let completion = if unexpected {
        RESULT_FAIL
    } else if base_status == STATUS_PRECISE_FAULT {
        RESULT_PRECISE_FAULT
    } else if attempted == CANDIDATES.len() {
        RESULT_DONE
    } else {
        RESULT_FAIL
    };
    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        status_word,
        CANDIDATES.len() as u32,
        BASE,
        base_value,
        candidate_values[0],
        candidate_values[1],
        candidate_values[2],
        candidate_values[3],
        candidate_values[4],
        fault_words[0],
        fault_words[1],
        fault_words[2],
        fault_words[3],
        completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    completion
}

#[cfg(all(target_arch = "arm", feature = "mpu-fault-enforcement-proof"))]
#[repr(C, align(32))]
struct MpuFaultProbeCell {
    words: [u32; 8],
}

#[cfg(all(target_arch = "arm", feature = "mpu-fault-enforcement-proof"))]
static mut MPU_FAULT_PROBE_CELL: MpuFaultProbeCell = MpuFaultProbeCell { words: [0; 8] };

#[cfg(all(target_arch = "arm", feature = "mpu-fault-enforcement-proof"))]
fn publish_mpu_fault_enforcement_proof() -> u32 {
    const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"MPUF");
    const RESULT_WORDS: usize = 16;
    const RESULT_PASS: u32 = u32::from_le_bytes(*b"PASS");
    const RESULT_SKIP: u32 = u32::from_le_bytes(*b"SKIP");
    const RESULT_FAIL: u32 = u32::from_le_bytes(*b"FAIL");
    const SCRATCH_VALUE: u32 = 0x4d50_5537;
    const MPU_REGION: u32 = 7;
    const MPU_RASR_NO_ACCESS_32B: u32 = 0x100c_0009;
    const MPU_CTRL_ENABLE_PRIVDEFENA: u32 = (1 << 0) | (1 << 2);
    const SHCSR_MEMFAULTENA: u32 = 1 << 16;
    const MPU_TYPE: *const u32 = 0xe000_ed90 as *const u32;
    const MPU_CTRL: *mut u32 = 0xe000_ed94 as *mut u32;
    const MPU_RNR: *mut u32 = 0xe000_ed98 as *mut u32;
    const MPU_RBAR: *mut u32 = 0xe000_ed9c as *mut u32;
    const MPU_RASR: *mut u32 = 0xe000_eda0 as *mut u32;
    const SCB_SHCSR: *mut u32 = 0xe000_ed24 as *mut u32;
    const _: () =
        assert!(RESULT_WORDS * core::mem::size_of::<u32>() <= rp1_hal::debug::MAILBOX_SIZE);

    let scratch = unsafe { core::ptr::addr_of_mut!(MPU_FAULT_PROBE_CELL.words).cast::<u32>() };
    let target = scratch as usize as u32;
    let control: u32;
    unsafe {
        core::arch::asm!("mrs {}, CONTROL", out(reg) control, options(nomem, nostack, preserves_flags));
        core::ptr::write_volatile(scratch, SCRATCH_VALUE);
    }
    let value_before = unsafe { core::ptr::read_volatile(scratch) };
    let mpu_type = unsafe { core::ptr::read_volatile(MPU_TYPE) };
    let ctrl_before = unsafe { core::ptr::read_volatile(MPU_CTRL) };
    let shcsr_before = unsafe { core::ptr::read_volatile(SCB_SHCSR) };
    let rnr_before = unsafe { core::ptr::read_volatile(MPU_RNR) };
    unsafe {
        core::ptr::write_volatile(MPU_RNR, MPU_REGION);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let rbar_before = unsafe { core::ptr::read_volatile(MPU_RBAR) };
    let rasr_before = unsafe { core::ptr::read_volatile(MPU_RASR) };
    unsafe {
        core::ptr::write_volatile(MPU_RNR, rnr_before);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    let type_ok = mpu_type & 1 == 0 && (mpu_type >> 8) & 0xff >= 8;
    let ctrl_ok = ctrl_before & 1 == 0;
    let region_ok = rasr_before & 1 == 0;
    let context_ok = control & 1 == 0
        && target & 0x1f == 0
        && (0x2000_0000..0x2001_0000).contains(&(target as usize))
        && value_before == SCRATCH_VALUE;
    let preconditions = type_ok && ctrl_ok && region_ok && context_ok;

    let mut flags = u32::from(type_ok)
        | u32::from(ctrl_ok) << 1
        | u32::from(region_ok) << 2
        | u32::from(context_ok) << 3;
    let mut exception = 0u32;
    let mut handler_count = 0u32;
    let mut cfsr = 0u32;
    let mut hfsr = 0u32;
    let mut mmfar = 0u32;
    let mut exact_recovery = false;

    if preconditions {
        unsafe {
            core::ptr::write_volatile(MPU_RNR, MPU_REGION);
            core::ptr::write_volatile(MPU_RBAR, target);
            core::ptr::write_volatile(MPU_RASR, MPU_RASR_NO_ACCESS_32B);
            core::ptr::write_volatile(SCB_SHCSR, shcsr_before | SHCSR_MEMFAULTENA);
            core::ptr::write_volatile(MPU_CTRL, MPU_CTRL_ENABLE_PRIVDEFENA);
            core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        }
        let configured = unsafe {
            core::ptr::read_volatile(MPU_RNR) == MPU_REGION
                && core::ptr::read_volatile(MPU_RBAR) & !0x1f == target
                && core::ptr::read_volatile(MPU_RASR) == MPU_RASR_NO_ACCESS_32B
                && core::ptr::read_volatile(MPU_CTRL) & MPU_CTRL_ENABLE_PRIVDEFENA
                    == MPU_CTRL_ENABLE_PRIVDEFENA
                && core::ptr::read_volatile(SCB_SHCSR) & SHCSR_MEMFAULTENA != 0
        };
        if configured {
            flags |= 1 << 4;
            let snapshot = unsafe { rp1_rt::run_expected_data_read(scratch) };
            flags |= 1 << 5;
            exception = snapshot.exception;
            handler_count = snapshot.handler_count;
            cfsr = snapshot.cfsr;
            hfsr = snapshot.hfsr;
            mmfar = snapshot.mmfar;
            exact_recovery = snapshot.recovered_memmanage_data_read_fault(target);
            flags |= u32::from(exact_recovery) << 6;
            flags |= u32::from(exception == 4) << 7;
            flags |= u32::from(cfsr & (1 << 1) != 0) << 8;
            flags |= u32::from(cfsr & (1 << 7) != 0) << 9;
            flags |= u32::from(mmfar == target) << 10;
            flags |= u32::from(hfsr == 0) << 11;
            flags |= u32::from(handler_count == 1 && snapshot.faulted == 1) << 12;
            flags |= u32::from(
                snapshot.probe_pc == snapshot.stacked_pc && snapshot.probe_pc != snapshot.resume_pc,
            ) << 13;
        }

        unsafe {
            core::ptr::write_volatile(MPU_CTRL, ctrl_before);
            core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
            core::ptr::write_volatile(MPU_RNR, MPU_REGION);
            core::ptr::write_volatile(MPU_RBAR, rbar_before);
            core::ptr::write_volatile(MPU_RASR, rasr_before);
        }
        let region_restored = unsafe {
            core::ptr::read_volatile(MPU_RBAR) == rbar_before
                && core::ptr::read_volatile(MPU_RASR) == rasr_before
        };
        unsafe {
            core::ptr::write_volatile(MPU_RNR, rnr_before);
        }
        let rnr_restored = unsafe { core::ptr::read_volatile(MPU_RNR) == rnr_before };
        let shcsr_current = unsafe { core::ptr::read_volatile(SCB_SHCSR) };
        let shcsr_restore =
            (shcsr_current & !SHCSR_MEMFAULTENA) | (shcsr_before & SHCSR_MEMFAULTENA);
        unsafe {
            core::ptr::write_volatile(SCB_SHCSR, shcsr_restore);
            core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        }
        flags |= u32::from(unsafe { core::ptr::read_volatile(MPU_CTRL) } == ctrl_before) << 14;
        flags |= u32::from(region_restored && rnr_restored) << 15;
        flags |= u32::from(
            unsafe { core::ptr::read_volatile(SCB_SHCSR) } & SHCSR_MEMFAULTENA
                == shcsr_before & SHCSR_MEMFAULTENA,
        ) << 16;
    }

    let value_after = unsafe { core::ptr::read_volatile(scratch) };
    flags |= u32::from(value_after == SCRATCH_VALUE) << 17;
    let restored = flags & ((1 << 14) | (1 << 15) | (1 << 16) | (1 << 17))
        == (1 << 14) | (1 << 15) | (1 << 16) | (1 << 17);
    let completion = if !preconditions {
        RESULT_SKIP
    } else if exact_recovery && restored {
        flags |= 1 << 18;
        RESULT_PASS
    } else {
        RESULT_FAIL
    };

    let fields = [
        (RESULT_WORDS as u32) << 16 | 1,
        flags,
        mpu_type,
        ctrl_before,
        shcsr_before,
        (rnr_before & 0xff)
            | (control & 0xff) << 8
            | (exception & 0xff) << 16
            | (handler_count & 0xff) << 24,
        rbar_before,
        rasr_before,
        target,
        value_before,
        cfsr,
        hfsr,
        mmfar,
        value_after,
        completion,
    ];
    let mailbox = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        core::ptr::write_volatile(mailbox, 0);
        for (index, value) in fields.into_iter().enumerate() {
            core::ptr::write_volatile(mailbox.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(mailbox, RESULT_MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    completion
}

#[cfg(all(target_arch = "arm", not(feature = "state3-composite-boundary")))]
fn emit_readback_frames(pin: &mut ConfiguredPin<22, Output>, uart0: &Uart0Tx) {
    const SNAPSHOT_IDS: [usize; 8] = [1, 3, 4, 5, 6, 7, 9, 10];

    // A unique calibration/header pulse precedes 72 fixed-order byte pulses.
    pulse_width(pin, 320);
    for id in SNAPSHOT_IDS {
        for byte in uart0.readback_snapshot(id).encoded_bytes() {
            pulse_width(pin, byte as u32 + 1);
        }
    }
}

#[cfg(target_arch = "arm")]
#[rp1_hal::main]
fn main(mut p: Peripherals) -> ! {
    let mut gpio22 = p.gpio.pin::<22>().into_output();
    pulse_group(&mut gpio22, 1);
    pulse_group(&mut gpio22, 2);

    #[cfg(feature = "gpio22-start-proof")]
    quiet_stop();

    #[cfg(feature = "rp1-clock-independence-proof")]
    clock_independence::initialize();

    #[cfg(feature = "pll-sys-core-lock-only")]
    match release_pll_sys_reset_bit29() {
        Ok(()) => pulse_width(&mut gpio22, 72),
        Err(_) => {
            pulse_width(&mut gpio22, 104);
            quiet_stop();
        }
    }

    #[cfg(feature = "endpoint-clock-only")]
    if enable_endpoint_clock_bit26(&mut gpio22).is_err() {
        loop {
            endpoint_clock_phase(&mut gpio22, 11); // ECLKF: fail-stop.
            delay_blink();
        }
    }

    #[cfg(feature = "state3-composite-boundary")]
    {
        let pre_state1 = pre_state1_reset_clock_boundary();
        let mut result = if !pre_state1.ready {
            State3Result {
                decision: State3Decision::PreState1ReadbackMismatch,
                pre_state1_observation: pre_state1.observation,
                perstn_wait_us: 0,
                perstn_observation: 0,
                boundary_us: 0,
                cperstn_to_first_poll_us: 0,
            }
        } else {
            let state1_2 = state1_2_perstn_qualified_boundary();
            if state1_2.qualified {
                state3_composite_boundary(state1_2)
            } else {
                State3Result {
                    decision: State3Decision::PerstnTimeout,
                    pre_state1_observation: pre_state1.observation,
                    perstn_wait_us: state1_2.wait_us,
                    perstn_observation: state1_2.observation,
                    boundary_us: 0,
                    cperstn_to_first_poll_us: 0,
                }
            }
        };
        result.pre_state1_observation = pre_state1.observation;

        #[cfg(feature = "state5-composite-boundary")]
        if result.decision == State3Decision::CoreAlive {
            let state5 = state5_composite_boundary();
            #[cfg(all(
                feature = "bar2-readonly-handshake",
                not(feature = "uart0-tx-polling-only"),
                not(feature = "rp1-linux-clk-uart-ownership-conflict")
            ))]
            if state5.decision == State5Decision::LinkUp {
                publish_bar2_readonly_identity(0);
            }
            #[cfg(not(feature = "uart-reset-irq-map-proof"))]
            emit_state5_result_frame(&mut gpio22, result, state5);
            #[cfg(feature = "rp1-clock-independence-proof")]
            if state5.decision == State5Decision::LinkUp
                && !clock_independence::acknowledge_and_wait_for_go(&p.raw_timer)
            {
                pulse_width(&mut gpio22, 1104);
                quiet_stop();
            }
            #[cfg(all(
                feature = "inbound-monitor-block-proof",
                not(feature = "rp1-bar1-hole-write-effect-proof")
            ))]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 1056);
                inbound_monitor::run();
            }
            #[cfg(feature = "cortex-m3-option-proof")]
            if state5.decision == State5Decision::LinkUp {
                let fields = cortex_m3_option_hardware_proof();
                publish_cortex_m3_option_hardware_proof(fields);
                pulse_width(&mut gpio22, 544);
                quiet_stop();
            }
            #[cfg(all(
                feature = "boot-rom-readonly-proof",
                not(feature = "boot-rom-dump-proof")
            ))]
            if state5.decision == State5Decision::LinkUp {
                publish_boot_rom_readonly_proof();
                pulse_width(&mut gpio22, 560);
                quiet_stop();
            }
            #[cfg(feature = "boot-rom-dump-proof")]
            if state5.decision == State5Decision::LinkUp {
                publish_boot_rom_dump();
                pulse_width(&mut gpio22, 576);
                quiet_stop();
            }
            #[cfg(feature = "boot-rom-boundary-proof")]
            if state5.decision == State5Decision::LinkUp {
                publish_boot_rom_boundary_proof();
                pulse_width(&mut gpio22, 592);
                quiet_stop();
            }
            #[cfg(feature = "proc1-boot-rom-proof")]
            if state5.decision == State5Decision::LinkUp {
                publish_proc1_boot_rom_proof();
                pulse_width(&mut gpio22, 608);
                quiet_stop();
            }
            #[cfg(feature = "dual-core-memory-proof")]
            if state5.decision == State5Decision::LinkUp {
                publish_dual_core_memory_proof();
                pulse_width(&mut gpio22, 624);
                quiet_stop();
            }
            #[cfg(feature = "proc-local-memory-proof")]
            if state5.decision == State5Decision::LinkUp {
                publish_proc_local_memory_proof();
                pulse_width(&mut gpio22, 640);
                quiet_stop();
            }
            #[cfg(feature = "expected-fault-recovery-proof")]
            if state5.decision == State5Decision::LinkUp {
                let recovered = publish_expected_fault_recovery_proof();
                pulse_width(&mut gpio22, if recovered { 656 } else { 688 });
                quiet_stop();
            }
            #[cfg(feature = "rp1-axi-dmac-identity-readonly-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_rp1_axi_dmac_identity_readonly_proof();
                let marker = if result == u32::from_le_bytes(*b"READ") {
                    1040
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    1200
                } else {
                    1360
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "rp1-axi-dmac-reset-state-readonly-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_rp1_axi_dmac_reset_state_readonly_proof();
                let marker = if result == u32::from_le_bytes(*b"READ") {
                    1040
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    1200
                } else {
                    1360
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "rp1-axi-dmac-active-identity-proof")]
            if state5.decision == State5Decision::LinkUp {
                let pll_sys = pll_sys_core_lock_transition();
                let result = publish_rp1_axi_dmac_active_identity_proof(pll_sys, false, false);
                let marker = if result == u32::from_le_bytes(*b"DATA") {
                    1056
                } else if result == u32::from_le_bytes(*b"ZERO") {
                    1216
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    1376
                } else {
                    1536
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "rp1-axi-dmac-internal-reset-identity-proof")]
            if state5.decision == State5Decision::LinkUp {
                let pll_sys = pll_sys_core_lock_transition();
                let result = publish_rp1_axi_dmac_active_identity_proof(pll_sys, true, false);
                let marker = if result == u32::from_le_bytes(*b"DATA") {
                    1056
                } else if result == u32::from_le_bytes(*b"ZERO") {
                    1216
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    1376
                } else {
                    1536
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "rp1-axi-dmac-global-enable-identity-proof")]
            if state5.decision == State5Decision::LinkUp {
                let pll_sys = pll_sys_core_lock_transition();
                let result = publish_rp1_axi_dmac_active_identity_proof(pll_sys, true, true);
                let marker = if result == u32::from_le_bytes(*b"DATA") {
                    1056
                } else if result == u32::from_le_bytes(*b"ZERO") {
                    1216
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    1376
                } else {
                    1536
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "shared-sram-bitband-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_shared_sram_bitband_proof();
                let marker = if result == u32::from_le_bytes(*b"PASS") {
                    704
                } else if result == u32::from_le_bytes(*b"NSUP")
                    || result == u32::from_le_bytes(*b"NBIT")
                {
                    720
                } else {
                    752
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "internal-memory-boundary-read-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_internal_memory_boundary_read_proof();
                let marker = if result == u32::from_le_bytes(*b"READ")
                    || result == u32::from_le_bytes(*b"PFLT")
                {
                    768
                } else {
                    800
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "shared-sram-64k-mirror-readonly-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_shared_sram_64k_mirror_readonly_proof();
                let marker = if result == u32::from_le_bytes(*b"PASS") {
                    816
                } else if result == u32::from_le_bytes(*b"DIFF") {
                    832
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    848
                } else {
                    880
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "shared-sram-alias-window-extent-readonly-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_shared_sram_alias_window_extent_readonly_proof();
                let marker = if result == u32::from_le_bytes(*b"DONE") {
                    896
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    912
                } else {
                    928
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "shared-sram-system-region-alias-readonly-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_shared_sram_system_region_alias_readonly_proof();
                let marker = if result == u32::from_le_bytes(*b"DONE") {
                    944
                } else if result == u32::from_le_bytes(*b"PFLT") {
                    960
                } else {
                    976
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "mpu-fault-enforcement-proof")]
            if state5.decision == State5Decision::LinkUp {
                let result = publish_mpu_fault_enforcement_proof();
                let marker = if result == u32::from_le_bytes(*b"PASS") {
                    992
                } else if result == u32::from_le_bytes(*b"SKIP") {
                    1008
                } else {
                    1024
                };
                pulse_width(&mut gpio22, marker);
                quiet_stop();
            }
            #[cfg(feature = "watchdog-proof")]
            if state5.decision == State5Decision::LinkUp {
                let watchdog = watchdog_hardware_proof();
                publish_watchdog_proof(watchdog);
                pulse_width(&mut gpio22, 432);
                quiet_stop();
            }
            #[cfg(feature = "watchdog-scratch-proof")]
            if state5.decision == State5Decision::LinkUp {
                let scratch = watchdog_scratch_hardware_proof();
                publish_watchdog_scratch_proof(scratch);
                pulse_width(&mut gpio22, 448);
                quiet_stop();
            }
            #[cfg(feature = "watchdog-expiry-reason-proof")]
            {
                // The host request/readback is the BAR2 reachability gate. The bounded
                // 24 ms state-5 sample may time out before the host enumerates the endpoint.
                pulse_width(&mut gpio22, 608);
                let result = watchdog_expiry_reason::run_or_arm();
                watchdog_expiry_reason::publish(result);
                pulse_width(&mut gpio22, 624);
                quiet_stop();
            }
            #[cfg(feature = "rp1-adc-one-shot-proof")]
            {
                pulse_width(&mut gpio22, 360);
                let result = adc_one_shot::run();
                adc_one_shot::publish(result);
                pulse_width(&mut gpio22, 656);
                quiet_stop();
            }
            #[cfg(feature = "rp1-i2s-readonly-prerequisite-snapshot")]
            {
                pulse_width(&mut gpio22, 368);
                let result = i2s_readonly::run();
                i2s_readonly::publish(result);
                pulse_width(&mut gpio22, 664);
                quiet_stop();
            }
            #[cfg(feature = "timer-register-proof")]
            if state5.decision == State5Decision::LinkUp {
                let timer = timer_register_hardware_proof();
                publish_timer_register_proof(timer);
                pulse_width(&mut gpio22, 464);
                quiet_stop();
            }
            #[cfg(feature = "timer-writable-time-proof")]
            if state5.decision == State5Decision::LinkUp {
                let timer = timer_writable_time_hardware_proof();
                publish_timer_writable_time_proof(timer);
                pulse_width(&mut gpio22, 480);
                quiet_stop();
            }
            #[cfg(feature = "timer0-inte-ints-proof")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let timer = timer0_inte_ints_proof();
                publish_timer0_inte_ints_proof(timer);
                pulse_width(&mut gpio22, if timer[0] == 1 { 368 } else { 504 });
                quiet_stop();
            }
            #[cfg(feature = "timer0-alarm0-local-irq26-candidate")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let decision = timer0_alarm0_local_irq26_candidate::run_and_publish();
                pulse_width(&mut gpio22, if decision == 1 { 421 } else { 557 });
                quiet_stop();
            }
            #[cfg(feature = "raw-timer-proof")]
            if state5.decision == State5Decision::LinkUp {
                let immediate_start = p.raw_timer.now();
                let immediate_delta_us = p.raw_timer.elapsed_since(immediate_start);

                gpio22.set_high();
                let delay_start = p.raw_timer.now();
                p.raw_timer.delay_us(1_000);
                let delay_delta_us = p.raw_timer.elapsed_since(delay_start);
                gpio22.set_low();

                publish_raw_timer_proof(immediate_delta_us, delay_delta_us);
                pulse_width(&mut gpio22, 416);
                quiet_stop();
            }
            #[cfg(feature = "pll-sys-core-lock-only")]
            if state5.decision == State5Decision::LinkUp {
                let pll_sys = pll_sys_core_lock_transition();
                #[cfg(not(feature = "rp1-linux-clk-uart-ownership-conflict"))]
                publish_pll_sys_core_lock_result(pll_sys);
                let pri_ph = if pll_sys.decision == PllSysCoreLockDecision::Locked {
                    enable_pll_sys_pri_ph_bit4()
                } else {
                    Err(PllSysPriPhError::ParentNotLocked)
                };
                match pri_ph {
                    Ok(()) => {
                        #[cfg(not(feature = "uart-reset-irq-map-proof"))]
                        pulse_width(&mut gpio22, 88);
                        #[cfg(feature = "uart0-reset-only")]
                        match release_uart0_reset_bank1_bit26() {
                            Ok(()) => pulse_width(&mut gpio22, 89),
                            Err(_) => {
                                pulse_width(&mut gpio22, 121);
                                quiet_stop();
                            }
                        }
                    }
                    Err(_) => {
                        pulse_width(&mut gpio22, 120);
                        quiet_stop();
                    }
                }
            }
            #[cfg(feature = "rp1-bar1-hole-write-effect-proof")]
            if state5.decision == State5Decision::LinkUp {
                let _uart0 = p.uart0.init_tx_115200_clock_ready();
                inbound_monitor::run();
            }
            #[cfg(feature = "rp1-linux-clk-uart-ownership-conflict")]
            if state5.decision == State5Decision::LinkUp {
                linux_clk_uart_ownership::run(&mut gpio22, &p.raw_timer, p.uart0);
            }
            #[cfg(feature = "rp1-clock-independence-proof")]
            if state5.decision == State5Decision::LinkUp {
                let spi_reset = release_spi0_reset_bank1_bit10();
                if !clock_independence::record_spi_reset(spi_reset, &p.raw_timer) {
                    pulse_width(&mut gpio22, 1120);
                    quiet_stop();
                }
                let uart0 = p.uart0;
                let spi0 = p.spi0;
                clock_independence::run_autonomous(
                    &mut gpio22,
                    &mut p.gpio,
                    &p.raw_timer,
                    uart0,
                    &mut p.pwm0,
                    spi0,
                );
                quiet_stop();
            }
            #[cfg(feature = "uart-reset-irq-map-proof")]
            if state5.decision == State5Decision::LinkUp {
                let fields = uart_reset_irq_map_hardware_proof(&mut p.resets);
                publish_uart_reset_irq_map_hardware_proof(fields);
                if fields[0] & 0x300 == 0x300 {
                    pulse_width(&mut gpio22, 496);
                } else {
                    pulse_width(&mut gpio22, 528);
                }
                quiet_stop();
            }
            #[cfg(feature = "uart1-local-nvic42-delivery")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let decision = uart1_local_nvic42_delivery::run_and_publish(&mut p.resets);
                pulse_width(&mut gpio22, if decision == 1 { 421 } else { 557 });
                quiet_stop();
            }
            #[cfg(feature = "uart2-local-nvic43-delivery")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let decision = uart2_local_nvic43_delivery::run_and_publish(&mut p.resets);
                pulse_width(&mut gpio22, if decision == 1 { 421 } else { 557 });
                quiet_stop();
            }
            #[cfg(feature = "uart3-local-nvic44-delivery")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let decision = uart3_local_nvic44_delivery::run_and_publish(&mut p.resets);
                pulse_width(&mut gpio22, if decision == 1 { 421 } else { 557 });
                quiet_stop();
            }
            #[cfg(feature = "uart4-local-nvic45-delivery")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let decision = uart4_local_nvic45_delivery::run_and_publish(&mut p.resets);
                pulse_width(&mut gpio22, if decision == 1 { 421 } else { 557 });
                quiet_stop();
            }
            #[cfg(feature = "uart5-local-nvic46-delivery")]
            if state5.decision == State5Decision::LinkUp {
                pulse_width(&mut gpio22, 360);
                let decision = uart5_local_nvic46_delivery::run_and_publish(&mut p.resets);
                pulse_width(&mut gpio22, if decision == 1 { 421 } else { 557 });
                quiet_stop();
            }
            #[cfg(feature = "pwm-gpio12-proof")]
            if state5.decision == State5Decision::LinkUp {
                #[cfg(feature = "pwm0-local-irq-proof")]
                {
                    let decision = pwm0_local_irq_proof::run(&mut p.pwm0);
                    pulse_width(&mut gpio22, if decision == 1 { 497 } else { 529 });
                }
                #[cfg(not(feature = "pwm0-local-irq-proof"))]
                {
                    const LOW: Pwm0Config = Pwm0Config::new(5_000_000, 1_250_000);
                    const HIGH: Pwm0Config = Pwm0Config::new(50_000, 37_500);
                    const LOW_PHASE_US: u64 = 1_500_000;

                    match p.pwm0.start_gpio12(LOW) {
                        Ok(mut pwm0) => {
                            let low_snapshot = pwm0.snapshot();
                            publish_pwm0_proof_result(1, low_snapshot, low_snapshot);

                            let phase_start = raw_timer_us();
                            while raw_timer_us().wrapping_sub(phase_start) < LOW_PHASE_US {
                                core::hint::spin_loop();
                            }

                            match pwm0.apply(HIGH) {
                                Ok(()) => {
                                    let high_snapshot = pwm0.snapshot();
                                    publish_pwm0_proof_result(2, low_snapshot, high_snapshot);
                                    pulse_width(&mut gpio22, 93);
                                }
                                Err(error) => {
                                    publish_pwm0_proof_result(
                                        0x200 | error as u32,
                                        low_snapshot,
                                        pwm0.snapshot(),
                                    );
                                    pulse_width(&mut gpio22, 125);
                                }
                            }
                        }
                        Err(error) => {
                            let snapshot = p.pwm0.snapshot();
                            publish_pwm0_proof_result(0x100 | error as u32, snapshot, snapshot);
                            pulse_width(&mut gpio22, 125);
                        }
                    }
                }
                quiet_stop();
            }
            #[cfg(all(feature = "uart0-tx-polling-only", not(feature = "uart0-polled-rx")))]
            if state5.decision == State5Decision::LinkUp {
                const TX_PHASE_PRE_INIT: u32 = 0x5450_0000;
                const TX_PHASE_POST_INIT: u32 = 0x5449_0000;
                let marker = b"\r\nRP1U0 POLLTX 0001\r\nRP1U0 POLLTX 0002\r\nRP1U0 POLLTX 0003\r\n";
                publish_bar2_readonly_identity(TX_PHASE_PRE_INIT);
                let mut uart0 = p.uart0.init_tx_115200_clock_ready();
                publish_bar2_readonly_identity(TX_PHASE_POST_INIT);
                let written = uart0.write_bytes(marker);
                publish_uart0_io_readback(written);
                if written == marker.len() {
                    pulse_width(&mut gpio22, 90);
                } else {
                    pulse_width(&mut gpio22, 122);
                }
            }
            #[cfg(all(feature = "uart0-polled-rx", not(feature = "uart0-rx-irq")))]
            if state5.decision == State5Decision::LinkUp {
                const TX_PHASE_PRE_INIT: u32 = 0x5450_0000;
                const TX_PHASE_POST_INIT: u32 = 0x5449_0000;
                const EXPECTED: &[u8; 18] = b"HOST2RP1 RX 0001\r\n";
                const READY: &[u8] = b"\r\nRP1U0 POLLTX 0001\r\nRP1U0 POLLTX 0002\r\nRP1U0 POLLTX 0003\r\nRP1U0 RXREADY 0001\r\n";
                const RX_OK: &[u8] = b"RP1U0 RXOK 0001\r\n";
                const RX_BAD: &[u8] = b"RP1U0 RXBAD 0001\r\n";
                const RX_TIMEOUT: &[u8] = b"RP1U0 RXTIMEOUT 0001\r\n";
                const RX_ERROR: &[u8] = b"RP1U0 RXERROR 0001\r\n";

                publish_bar2_readonly_identity(TX_PHASE_PRE_INIT);
                let mut uart0 = p.uart0.init_tx_rx_115200_clock_ready();
                publish_bar2_readonly_identity(TX_PHASE_POST_INIT);
                let ready_written = uart0.write_bytes(READY);
                publish_uart0_io_readback(ready_written);

                let mut received = [0u8; EXPECTED.len()];
                let mut received_count = 0usize;
                let mut decision = 0u32;
                while ready_written == READY.len() && received_count < received.len() {
                    match uart0.read_byte() {
                        Ok(byte) => {
                            received[received_count] = byte;
                            received_count += 1;
                        }
                        Err(rp1_hal::uart::Uart0RxError::Timeout) => {
                            decision = 3;
                            break;
                        }
                        Err(rp1_hal::uart::Uart0RxError::DataError(flags)) => {
                            decision = 0x100 | u32::from(flags);
                            break;
                        }
                    }
                }

                if ready_written != READY.len() {
                    decision = 4;
                } else if decision == 0 {
                    decision = if received == *EXPECTED { 1 } else { 2 };
                }
                publish_uart0_rx_result(decision, received_count, &received);

                let response: &[u8] = match decision {
                    1 => RX_OK,
                    2 => RX_BAD,
                    3 => RX_TIMEOUT,
                    _ => RX_ERROR,
                };
                let response_written = uart0.write_bytes(response);
                if decision == 1 && response_written == response.len() {
                    pulse_width(&mut gpio22, 91);
                } else {
                    pulse_width(&mut gpio22, 123);
                }
            }
            #[cfg(feature = "uart0-rx-irq")]
            if state5.decision == State5Decision::LinkUp {
                const IRQ_TIMEOUT_US: u64 = 1_000_000;
                const EXPECTED: &[u8; 19] = b"HOST2RP1 IRQ 0001\r\n";
                const READY: &[u8] = b"\r\nRP1U0 POLLTX 0001\r\nRP1U0 POLLTX 0002\r\nRP1U0 POLLTX 0003\r\nRP1U0 IRQREADY 0001\r\n";
                const IRQ_OK: &[u8] = b"RP1U0 IRQOK 0001\r\n";
                const IRQ_BAD: &[u8] = b"RP1U0 IRQBAD 0001\r\n";
                const IRQ_TIMEOUT: &[u8] = b"RP1U0 IRQTIMEOUT 0001\r\n";
                const IRQ_ERROR: &[u8] = b"RP1U0 IRQERROR 0001\r\n";

                let mut uart0 = p.uart0.init_tx_rx_115200_clock_ready();
                let route_before = rp1_rt::uart0_irq_route_snapshot();
                let prepared = unsafe { rp1_rt::prepare_uart0_irq() };
                let source_ready = prepared && uart0.enable_rx_interrupt(EXPECTED.len());
                if source_ready {
                    unsafe {
                        rp1_rt::enable_uart0_irq();
                    }
                }
                let route_enabled = rp1_rt::uart0_irq_route_snapshot();
                let ready_written = if source_ready {
                    uart0.write_bytes(READY)
                } else {
                    0
                };

                let start = raw_timer_us();
                let mut irq = Uart0Tx::rx_interrupt_snapshot();
                let mut timed_out = false;
                while source_ready
                    && ready_written == READY.len()
                    && irq.decision == rp1_hal::uart::UART0_IRQ_DECISION_PENDING
                {
                    if raw_timer_us().wrapping_sub(start) > IRQ_TIMEOUT_US {
                        timed_out = true;
                        break;
                    }
                    core::hint::spin_loop();
                    irq = Uart0Tx::rx_interrupt_snapshot();
                }

                unsafe {
                    rp1_rt::disable_uart0_irq();
                }
                uart0.mask_and_clear_rx_interrupt();
                irq = Uart0Tx::rx_interrupt_snapshot();
                let route_final = rp1_rt::uart0_irq_route_snapshot();

                let exact = irq.byte_count as usize == EXPECTED.len()
                    && irq.payload[..EXPECTED.len()] == *EXPECTED;
                let decision = if !source_ready || ready_written != READY.len() {
                    4
                } else if timed_out {
                    3
                } else if irq.decision == rp1_hal::uart::UART0_IRQ_DECISION_COMPLETE && exact {
                    1
                } else if irq.decision == rp1_hal::uart::UART0_IRQ_DECISION_COMPLETE {
                    2
                } else {
                    0x100 | irq.decision
                };
                publish_uart0_irq_result(decision, irq, route_before, route_enabled, route_final);

                let response: &[u8] = match decision {
                    1 => IRQ_OK,
                    2 => IRQ_BAD,
                    3 => IRQ_TIMEOUT,
                    _ => IRQ_ERROR,
                };
                let response_written = uart0.write_bytes(response);
                if decision == 1 && response_written == response.len() {
                    pulse_width(&mut gpio22, 92);
                } else {
                    pulse_width(&mut gpio22, 124);
                }
            }
            #[cfg(feature = "gpio-wiring-proof")]
            if state5.decision == State5Decision::LinkUp {
                let miso = p.gpio.pin::<9>().into_input_pull_up();
                let mut i2c_sda = p.gpio.pin::<2>().into_output();
                let mut i2c_scl = p.gpio.pin::<3>().into_output();
                let mut spi_mosi = p.gpio.pin::<10>().into_output();
                let mut spi_sclk = p.gpio.pin::<11>().into_output();
                let mut spi_cs0 = p.gpio.pin::<8>().into_output();
                let mut spi_cs1 = p.gpio.pin::<7>().into_output();

                busy_wait_us(20_000);
                wiring_pulses(&mut i2c_sda, 2);
                wiring_pulses(&mut i2c_scl, 3);
                wiring_pulses(&mut spi_mosi, 4);
                wiring_pulses(&mut spi_sclk, 5);
                wiring_pulses(&mut spi_cs0, 6);
                wiring_pulses(&mut spi_cs1, 7);

                pulse_width(&mut gpio22, 336); // Wiring outputs complete; MISO is input.
                let saw_low = wait_for_input_level(&miso, false, 10_000_000);
                if saw_low {
                    pulse_width(&mut gpio22, 344); // ESP32 pulled MISO low.
                }
                let saw_release = saw_low && wait_for_input_level(&miso, true, 10_000_000);
                if saw_release {
                    pulse_width(&mut gpio22, 352); // ESP32 released MISO; RP1 pull-up won.
                } else if saw_low {
                    pulse_width(&mut gpio22, 488); // MISO release was not observed.
                } else {
                    pulse_width(&mut gpio22, 480); // MISO pull-low was not observed.
                }
                quiet_stop();
            }
            #[cfg(feature = "i2c1-reset-only")]
            if state5.decision == State5Decision::LinkUp {
                match release_i2c1_reset_bank0_bit8() {
                    Ok(()) => pulse_width(&mut gpio22, 376),
                    Err(_) => {
                        pulse_width(&mut gpio22, 512);
                        quiet_stop();
                    }
                }
                #[cfg(not(any(
                    feature = "i2c1-host-proof",
                    feature = "i2c1-local-irq-proof",
                    feature = "i2c1-local-irq-bank1-passive-scout"
                )))]
                quiet_stop();
            }
            #[cfg(feature = "i2c1-local-irq-bank1-passive-scout")]
            if state5.decision == State5Decision::LinkUp {
                let sda = p.gpio.pin::<2>();
                let scl = p.gpio.pin::<3>();
                match p.i2c1.into_host_100khz(sda, scl) {
                    Ok(mut i2c1) => {
                        pulse_width(&mut gpio22, 361); // I2C1 passive scout initialized.
                        let decision = i2c1_local_irq_bank1_passive_scout::run(&mut i2c1);
                        pulse_width(&mut gpio22, if decision == 1 { 371 } else { 507 });
                    }
                    Err(_) => {
                        i2c1_local_irq_bank1_passive_scout::publish_setup_error(0x302);
                        pulse_width(&mut gpio22, 496);
                    }
                }
                quiet_stop();
            }
            #[cfg(feature = "i2c1-local-irq-proof")]
            if state5.decision == State5Decision::LinkUp {
                let sda = p.gpio.pin::<2>();
                let scl = p.gpio.pin::<3>();
                match p.i2c1.into_host_100khz(sda, scl) {
                    Ok(mut i2c1) => {
                        pulse_width(&mut gpio22, 360); // I2C1 host initialized.
                        let decision = i2c1_local_irq_proof::run(&mut i2c1);
                        pulse_width(&mut gpio22, if decision == 1 { 369 } else { 505 });
                    }
                    Err(_) => {
                        i2c1_local_irq_proof::publish_setup_error(0x302);
                        pulse_width(&mut gpio22, 496);
                    }
                }
                quiet_stop();
            }
            #[cfg(feature = "i2c1-host-proof")]
            if state5.decision == State5Decision::LinkUp {
                const I2C1_PROOF_PACKET: [u8; 20] = [
                    0x44, 0x31, 0x44, 0x52, 0x01, 0x49, 0x01, 0x09, 0xdf, 0x9b, 0x57, 0x13, 0xe0,
                    0xac, 0x68, 0x24, 0x31, 0x43, 0x32, 0x49,
                ];

                let sda = p.gpio.pin::<2>();
                let scl = p.gpio.pin::<3>();
                match p.i2c1.into_host_100khz(sda, scl) {
                    Ok(mut i2c1) => {
                        pulse_width(&mut gpio22, 360); // I2C1 host initialized.
                        match i2c1.write(0x2d, &I2C1_PROOF_PACKET) {
                            Ok(_) => pulse_width(&mut gpio22, 368),
                            Err(_) => pulse_width(&mut gpio22, 504),
                        }
                    }
                    Err(_) => pulse_width(&mut gpio22, 496),
                }
                quiet_stop();
            }
            #[cfg(feature = "spi0-reset-only")]
            #[cfg(not(feature = "rp1-clock-independence-proof"))]
            if state5.decision == State5Decision::LinkUp {
                match release_spi0_reset_bank1_bit10() {
                    Ok(()) => pulse_width(&mut gpio22, 384),
                    Err(_) => {
                        pulse_width(&mut gpio22, 520);
                        quiet_stop();
                    }
                }
                #[cfg(feature = "spi0-local-irq-proof")]
                {
                    let decision = spi0_local_irq_proof::run(&mut p.spi0);
                    pulse_width(&mut gpio22, if decision == 1 { 409 } else { 537 });
                    quiet_stop();
                }
                #[cfg(feature = "spi0-local-irq-bank1-passive-scout")]
                {
                    let decision = spi0_local_irq_bank1_passive_scout::run(&mut p.spi0);
                    pulse_width(&mut gpio22, if decision == 1 { 411 } else { 539 });
                    quiet_stop();
                }
                #[cfg(not(any(
                    feature = "spi0-host-proof",
                    feature = "spi0-local-irq-proof",
                    feature = "spi0-local-irq-bank1-passive-scout"
                )))]
                quiet_stop();
            }
            #[cfg(feature = "spi0-host-proof")]
            if state5.decision == State5Decision::LinkUp {
                const SPI0_PROOF_PACKET: [u8; 20] = [
                    0x44, 0x31, 0x53, 0x50, 0x01, 0x53, 0x02, 0x09, 0xdf, 0x9b, 0x57, 0x13, 0xe0,
                    0xac, 0x68, 0x24, 0x53, 0x50, 0x49, 0x30,
                ];

                let cs0 = p.gpio.pin::<8>();
                let miso = p.gpio.pin::<9>();
                let mosi = p.gpio.pin::<10>();
                let sclk = p.gpio.pin::<11>();
                match p.spi0.into_host_mode0_100khz(cs0, miso, mosi, sclk) {
                    Ok(mut spi0) => {
                        pulse_width(&mut gpio22, 400); // SPI0 host initialized.
                        match spi0.write(&SPI0_PROOF_PACKET) {
                            Ok(_) => pulse_width(&mut gpio22, 408),
                            Err(_) => pulse_width(&mut gpio22, 536),
                        }
                    }
                    Err(_) => pulse_width(&mut gpio22, 528),
                }
                quiet_stop();
            }
            #[cfg(feature = "debug-mailbox-ping")]
            if state5.decision == State5Decision::LinkUp {
                loop {
                    rp1_hal::mailbox::poll();
                    core::hint::spin_loop();
                }
            }
            quiet_stop();
        }

        emit_state3_result_frame(&mut gpio22, result);
        quiet_stop();
    }

    #[cfg(not(feature = "state3-composite-boundary"))]
    {
        pulse_group(&mut gpio22, 3);
        let mut uart0 = p.uart0.init_115200();
        pulse_group(&mut gpio22, 4);

        pulse_group(&mut gpio22, 5);
        let bytes = b"\r\nRP1U0 PINMUX 0001\r\nRP1U0 PINMUX 0002\r\nRP1U0 PINMUX 0003\r\n";
        if uart0.write_bytes(bytes) == bytes.len() {
            pulse_group(&mut gpio22, 6);
        } else {
            pulse_group(&mut gpio22, 7);
        }
        emit_readback_frames(&mut gpio22, &uart0);

        #[cfg(feature = "endpoint-clock-only")]
        {
            // Re-emit the completed phase summary after the long readback frame so
            // ECLK0/ECLK1/ECLK2 remain in the bounded ESP32 trace window.
            endpoint_clock_phase(&mut gpio22, 8);
            endpoint_clock_phase(&mut gpio22, 9);
            endpoint_clock_phase(&mut gpio22, 10);
        }

        loop {
            let _ = uart0.write_bytes(b"RP1U0 PINMUX tick\r\n");
            delay_blink();
            delay_blink();
        }
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {}

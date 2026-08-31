#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
use rp1_hal::prelude::*;
#[cfg(target_arch = "arm")]
use rp1_rt as _;

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
    const POLL_LIMIT: usize = 100_000;

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

#[cfg(all(target_arch = "arm", feature = "state3-composite-boundary"))]
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
    not(feature = "uart0-rx-irq")
))]
fn publish_bar2_readonly_identity(flags: u32) {
    const _: () = assert!(
        core::mem::size_of::<rp1_hal::debug::DebugMailbox>() <= rp1_hal::debug::MAILBOX_SIZE
    );

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

#[cfg(all(target_arch = "arm", feature = "pll-sys-core-lock-only"))]
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

#[cfg(all(target_arch = "arm", feature = "pwm-gpio12-proof"))]
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
                not(feature = "uart0-tx-polling-only")
            ))]
            if state5.decision == State5Decision::LinkUp {
                publish_bar2_readonly_identity(0);
            }
            emit_state5_result_frame(&mut gpio22, result, state5);
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
                publish_pll_sys_core_lock_result(pll_sys);
                let pri_ph = if pll_sys.decision == PllSysCoreLockDecision::Locked {
                    enable_pll_sys_pri_ph_bit4()
                } else {
                    Err(PllSysPriPhError::ParentNotLocked)
                };
                match pri_ph {
                    Ok(()) => {
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
            #[cfg(feature = "pwm-gpio12-proof")]
            if state5.decision == State5Decision::LinkUp {
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
                #[cfg(not(feature = "i2c1-host-proof"))]
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
            if state5.decision == State5Decision::LinkUp {
                match release_spi0_reset_bank1_bit10() {
                    Ok(()) => pulse_width(&mut gpio22, 384),
                    Err(_) => {
                        pulse_width(&mut gpio22, 520);
                        quiet_stop();
                    }
                }
                #[cfg(not(feature = "spi0-host-proof"))]
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
            #[cfg(feature = "bar2-rpc-poll")]
            if state5.decision == State5Decision::LinkUp {
                rp1_hal::rpc::init();
                loop {
                    rp1_hal::rpc::poll(&p.raw_timer);
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

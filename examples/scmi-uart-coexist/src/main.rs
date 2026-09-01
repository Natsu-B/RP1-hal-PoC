#![cfg_attr(target_arch = "arm", no_std)]
#![cfg_attr(target_arch = "arm", no_main)]

#[cfg(target_arch = "arm")]
use rp1_hal::prelude::*;
#[cfg(target_arch = "arm")]
use rp1_rt as _;

#[cfg(target_arch = "arm")]
const RESET_CTRL1: *mut u32 = 0x4001_4004 as *mut u32;
#[cfg(target_arch = "arm")]
const RESET_DONE1: *const u32 = 0x4001_401c as *const u32;
#[cfg(target_arch = "arm")]
const UART0_RESET: u32 = 1 << 26;
#[cfg(target_arch = "arm")]
const PLL_SYS_PRIM: *mut u32 = 0x4002_0010 as *mut u32;
#[cfg(target_arch = "arm")]
const PLL_PH_EN: u32 = 1 << 4;
#[cfg(target_arch = "arm")]
const CLK_UART_CTRL: *const u32 = 0x4001_8054 as *const u32;
#[cfg(target_arch = "arm")]
const CLK_UART_ENABLE: u32 = 1 << 11;
#[cfg(target_arch = "arm")]
const HEARTBEAT_PERIOD_US: u64 = 100_000;
#[cfg(target_arch = "arm")]
const HEARTBEAT_TEMPLATE: [u8; 54] = *b"RP1CLK seq=0x00000000 ctrl=0x00000000 off=0x00000000\r\n";

#[cfg(target_arch = "arm")]
fn release_uart0_reset() -> bool {
    unsafe {
        let value = core::ptr::read_volatile(RESET_CTRL1);
        core::ptr::write_volatile(RESET_CTRL1, value & !UART0_RESET);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    for _ in 0..100_000 {
        if unsafe { core::ptr::read_volatile(RESET_DONE1) } & UART0_RESET != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

#[cfg(target_arch = "arm")]
fn ensure_uart_apb_phase_enabled() -> bool {
    unsafe {
        let value = core::ptr::read_volatile(PLL_SYS_PRIM);
        core::ptr::write_volatile(PLL_SYS_PRIM, value | PLL_PH_EN);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        core::ptr::read_volatile(PLL_SYS_PRIM) & PLL_PH_EN != 0
    }
}

#[cfg(target_arch = "arm")]
fn write_hex32(uart: &mut Uart0Tx, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0; 8];
    for index in 0..8 {
        bytes[index] = HEX[((value >> ((7 - index) * 4)) & 0xf) as usize];
    }
    let _ = uart.write_bytes(&bytes);
}

#[cfg(target_arch = "arm")]
fn write_field(uart: &mut Uart0Tx, name: &[u8], value: u32) {
    let _ = uart.write_bytes(b"|");
    let _ = uart.write_bytes(name);
    let _ = uart.write_bytes(b"=");
    write_hex32(uart, value);
}

#[cfg(target_arch = "arm")]
fn write_telemetry(uart: &mut Uart0Tx, heartbeat: u32) {
    let state = rp1_hal::scmi::telemetry();
    let _ = uart.write_bytes(b"RP1SCMI|TELEMETRY");
    write_field(uart, b"heartbeat", heartbeat);
    write_field(uart, b"irq", state.irq_count);
    write_field(uart, b"proc_event", state.proc_event_count);
    write_field(uart, b"host_event", state.host_event_count);
    write_field(uart, b"config_set", state.clock_config_set_count);
    write_field(uart, b"enable", state.clock_enable_count);
    write_field(uart, b"disable", state.clock_disable_count);
    write_field(uart, b"linux_votes", state.linux_votes);
    write_field(uart, b"firmware_votes", state.firmware_votes);
    write_field(uart, b"clk_uart_ctrl", state.clk_uart_ctrl);
    write_field(uart, b"clk_uart_div_int", state.clk_uart_div_int);
    write_field(uart, b"clk_uart_sel", state.clk_uart_sel);
    write_field(uart, b"pll_sys_prim", state.pll_sys_prim);
    let _ = uart.write_bytes(b"\r\n");
}

#[cfg(target_arch = "arm")]
fn encode_hex_u32(out: &mut [u8], offset: usize, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for index in 0..8 {
        out[offset + index] = HEX[((value >> ((7 - index) * 4)) & 0xf) as usize];
    }
}

#[cfg(target_arch = "arm")]
fn heartbeat_line(sequence: u32, ctrl: u32, off_periods: u32) -> [u8; 54] {
    let mut line = HEARTBEAT_TEMPLATE;
    encode_hex_u32(&mut line, 13, sequence);
    encode_hex_u32(&mut line, 29, ctrl);
    encode_hex_u32(&mut line, 44, off_periods);
    line
}

#[cfg(target_arch = "arm")]
#[rp1_hal::main]
fn main(mut p: Peripherals) -> ! {
    let mut gpio22 = p.gpio.pin::<22>().into_output();
    if !ensure_uart_apb_phase_enabled() || !release_uart0_reset() {
        gpio22.set_high();
        loop {
            core::hint::spin_loop();
        }
    }

    let mut uart0 = p.uart0.init_115200();
    let _ = uart0.write_bytes(b"RP1 SCMI UART clock server boot\r\n");

    rp1_hal::rpc::init();
    rp1_hal::scmi::init_uart_clock_server();
    rp1_hal::scmi::set_firmware_uart_votes(true, true);
    unsafe {
        rp1_hal::scmi_irq::enable();
    }

    let start = p.raw_timer.now();
    let mut next_us = 0;
    let mut off_periods = 0u32;
    loop {
        rp1_hal::rpc::poll(&p.raw_timer);
        let elapsed_us = p.raw_timer.elapsed_since(start);
        if elapsed_us >= next_us {
            let sequence = (elapsed_us / HEARTBEAT_PERIOD_US) as u32;
            if sequence % 100 == 0 {
                gpio22.set_high();
                p.raw_timer.delay_us(1_000);
                gpio22.set_low();
            }

            let ctrl = unsafe { core::ptr::read_volatile(CLK_UART_CTRL) };
            if ctrl & CLK_UART_ENABLE != 0 {
                let line = heartbeat_line(sequence, ctrl, off_periods);
                let _ = uart0.write_bytes(&line);
                if sequence % 10 == 0 {
                    write_telemetry(&mut uart0, sequence);
                }
            } else {
                off_periods = off_periods.wrapping_add(1);
            }
            next_us = (elapsed_us / HEARTBEAT_PERIOD_US + 1) * HEARTBEAT_PERIOD_US;
        }
        core::hint::spin_loop();
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {}

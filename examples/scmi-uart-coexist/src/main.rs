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
fn ensure_uart_apb_phase_enabled() {
    unsafe {
        let value = core::ptr::read_volatile(PLL_SYS_PRIM);
        core::ptr::write_volatile(PLL_SYS_PRIM, value | PLL_PH_EN);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(target_arch = "arm")]
#[rp1_hal::main]
fn main(p: Peripherals) -> ! {
    ensure_uart_apb_phase_enabled();
    if !release_uart0_reset() {
        loop {
            core::hint::spin_loop();
        }
    }

    // Programs RP1_CLK_UART to XOSC / 1 = 50 MHz and selects GPIO14/15 for
    // firmware-owned UART0.
    let mut uart0 = p.uart0.init_115200();
    let _ = uart0.write_bytes(b"RP1 SCMI UART clock server boot\r\n");

    rp1_hal::scmi::init_uart_clock_server();
    rp1_hal::scmi::set_firmware_uart_votes(true, true);
    unsafe {
        rp1_hal::scmi_irq::install_and_enable();
    }

    loop {
        let _ = uart0.write_bytes(b"RP1 UART0 alive\r\n");
        for _ in 0..1_000_000 {
            core::hint::spin_loop();
        }
    }
}

#[cfg(not(target_arch = "arm"))]
fn main() {}

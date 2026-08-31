#![no_std]

#[cfg(all(
    any(feature = "debug-mailbox-init", feature = "debug-stub"),
    target_arch = "arm"
))]
pub mod debug_stub;

#[cfg(all(feature = "pcie-ep-init", target_arch = "arm"))]
pub mod pcie_ep_init;

#[cfg(target_arch = "arm")]
use core::panic::PanicInfo;

#[cfg(target_arch = "arm")]
unsafe extern "C" {
    fn _stack_start();
}

#[cfg(target_arch = "arm")]
unsafe extern "Rust" {
    fn rp1_entry() -> !;
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
unsafe extern "C" {
    fn UART0_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "scmi-uart-clock"))]
unsafe extern "C" {
    fn RP1_SCMI_IRQHandler();
}

#[cfg(target_arch = "arm")]
unsafe extern "C" {
    static mut __sbss: u8;
    static mut __ebss: u8;
}

#[cfg(all(
    target_arch = "arm",
    not(any(feature = "uart0-rx-irq", feature = "scmi-uart-clock"))
))]
#[unsafe(link_section = ".vector_table")]
#[used]
pub static VECTOR_TABLE: [unsafe extern "C" fn(); 16] = [
    _stack_start,
    Reset,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
    DefaultHandler,
];

#[cfg(all(
    target_arch = "arm",
    feature = "uart0-rx-irq",
    feature = "scmi-uart-clock"
))]
const fn local_irq_vector_table() -> [unsafe extern "C" fn(); 80] {
    let mut vectors = [DefaultHandler as unsafe extern "C" fn(); 80];
    vectors[0] = _stack_start;
    vectors[1] = Reset;
    vectors[UART0_VECTOR_INDEX] = UART0_IRQHandler;
    vectors[SCMI_VECTOR_INDEX] = RP1_SCMI_IRQHandler;
    vectors
}

#[cfg(all(
    target_arch = "arm",
    feature = "uart0-rx-irq",
    not(feature = "scmi-uart-clock")
))]
const fn local_irq_vector_table() -> [unsafe extern "C" fn(); 80] {
    let mut vectors = [DefaultHandler as unsafe extern "C" fn(); 80];
    vectors[0] = _stack_start;
    vectors[1] = Reset;
    vectors[UART0_VECTOR_INDEX] = UART0_IRQHandler;
    vectors
}

#[cfg(all(
    target_arch = "arm",
    feature = "scmi-uart-clock",
    not(feature = "uart0-rx-irq")
))]
const fn local_irq_vector_table() -> [unsafe extern "C" fn(); 80] {
    let mut vectors = [DefaultHandler as unsafe extern "C" fn(); 80];
    vectors[0] = _stack_start;
    vectors[1] = Reset;
    vectors[SCMI_VECTOR_INDEX] = RP1_SCMI_IRQHandler;
    vectors
}

#[cfg(all(
    target_arch = "arm",
    any(feature = "uart0-rx-irq", feature = "scmi-uart-clock")
))]
#[unsafe(link_section = ".vector_table")]
#[used]
pub static VECTOR_TABLE: [unsafe extern "C" fn(); 80] = local_irq_vector_table();

pub const UART0_IRQ_NUMBER: usize = 25;
pub const UART0_VECTOR_INDEX: usize = 16 + UART0_IRQ_NUMBER;
pub const SCMI_IRQ_NUMBER: usize = 57;
pub const SCMI_VECTOR_INDEX: usize = 16 + SCMI_IRQ_NUMBER;

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const UART0_IRQ_BIT: u32 = 1 << UART0_IRQ_NUMBER;
#[cfg(all(
    target_arch = "arm",
    feature = "uart0-rx-irq",
    feature = "scmi-uart-clock"
))]
const SCMI_IRQ_BIT1: u32 = 1 << (SCMI_IRQ_NUMBER - 32);
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const VECTOR_TABLE_BASE: u32 = 0x2000_0000;
#[cfg(all(
    target_arch = "arm",
    any(feature = "uart0-rx-irq", feature = "scmi-uart-clock")
))]
const SCB_VTOR: *mut u32 = 0xe000_ed08 as *mut u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const NVIC_ISER0: *mut u32 = 0xe000_e100 as *mut u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const NVIC_ISER1: *const u32 = 0xe000_e104 as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const NVIC_ICER0: *mut u32 = 0xe000_e180 as *mut u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const NVIC_ICPR0: *mut u32 = 0xe000_e280 as *mut u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const NVIC_ISPR0: *const u32 = 0xe000_e200 as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const NVIC_IABR0: *const u32 = 0xe000_e300 as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const SCB_ICSR: *const u32 = 0xe000_ed04 as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const SCB_SHCSR: *const u32 = 0xe000_ed24 as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const SCB_CFSR: *const u32 = 0xe000_ed28 as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const SCB_HFSR: *const u32 = 0xe000_ed2c as *const u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const UART0_IRQ_DIAGNOSTIC_ADDR: *mut u32 = rp1_abi::debug::MAILBOX_ADDR as *mut u32;
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const UART0_IRQ_ENTRY_MAGIC: u32 = u32::from_le_bytes(*b"U0EN");
#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const UART0_EXCEPTION_MAGIC: u32 = u32::from_le_bytes(*b"U0EX");

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
#[derive(Clone, Copy)]
pub struct Uart0IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
pub fn uart0_irq_route_snapshot() -> Uart0IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Uart0IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
#[inline(always)]
unsafe fn record_uart0_exception(magic: u32) {
    let ipsr: u32;
    unsafe {
        core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        core::ptr::write_volatile(UART0_IRQ_DIAGNOSTIC_ADDR, 0);
        core::ptr::write_volatile(UART0_IRQ_DIAGNOSTIC_ADDR.add(1), ipsr);
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(2),
            core::ptr::read_volatile(SCB_ICSR),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(3),
            core::ptr::read_volatile(SCB_VTOR),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(4),
            core::ptr::read_volatile(NVIC_ISER0),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(5),
            core::ptr::read_volatile(NVIC_ISPR0),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(6),
            core::ptr::read_volatile(NVIC_IABR0),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(7),
            core::ptr::read_volatile(SCB_SHCSR),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(8),
            core::ptr::read_volatile(SCB_CFSR),
        );
        core::ptr::write_volatile(
            UART0_IRQ_DIAGNOSTIC_ADDR.add(9),
            core::ptr::read_volatile(SCB_HFSR),
        );
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(UART0_IRQ_DIAGNOSTIC_ADDR, magic);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
pub unsafe fn record_uart0_irq_entry() {
    unsafe {
        record_uart0_exception(UART0_IRQ_ENTRY_MAGIC);
    }
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
pub unsafe fn prepare_uart0_irq() -> bool {
    let before = uart0_irq_route_snapshot();
    #[cfg(feature = "scmi-uart-clock")]
    let allowed_iser1 = SCMI_IRQ_BIT1;
    #[cfg(not(feature = "scmi-uart-clock"))]
    let allowed_iser1 = 0;
    if before.vtor != VECTOR_TABLE_BASE
        || before.iser0 & !UART0_IRQ_BIT != 0
        || before.iser1 & !allowed_iser1 != 0
    {
        return false;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, UART0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, UART0_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    true
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
pub unsafe fn enable_uart0_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICPR0, UART0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ISER0, UART0_IRQ_BIT);
        core::arch::asm!(
            "dsb sy",
            "isb",
            "cpsie i",
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
pub unsafe fn disable_uart0_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, UART0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, UART0_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(
    target_arch = "arm",
    any(feature = "uart0-rx-irq", feature = "scmi-uart-clock")
))]
unsafe fn configure_vector_table() {
    let address = core::ptr::addr_of!(VECTOR_TABLE) as usize as u32;
    unsafe {
        core::ptr::write_volatile(SCB_VTOR, address);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() {
    unsafe {
        zero_bss();
    }
    #[cfg(any(feature = "uart0-rx-irq", feature = "scmi-uart-clock"))]
    unsafe {
        configure_vector_table();
    }
    #[cfg(feature = "pcie-ep-init")]
    pcie_ep_init::init();
    #[cfg(all(feature = "debug-mailbox-init", not(feature = "debug-stub")))]
    debug_stub::init_mailbox();
    #[cfg(feature = "debug-stub")]
    debug_stub::init();
    unsafe { rp1_entry() }
}

#[cfg(target_arch = "arm")]
unsafe fn zero_bss() {
    let mut ptr = core::ptr::addr_of_mut!(__sbss);
    let end = core::ptr::addr_of_mut!(__ebss);
    while ptr < end {
        unsafe {
            ptr.write_volatile(0);
            ptr = ptr.add(1);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DefaultHandler() {
    #[cfg(all(feature = "uart0-rx-irq", target_arch = "arm"))]
    unsafe {
        record_uart0_exception(UART0_EXCEPTION_MAGIC);
    }

    #[cfg(all(feature = "debug-stub", target_arch = "arm"))]
    debug_stub::fault();

    #[cfg(not(all(feature = "debug-stub", target_arch = "arm")))]
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "arm")]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    #[cfg(feature = "debug-stub")]
    debug_stub::panic();

    #[cfg(not(feature = "debug-stub"))]
    loop {
        core::hint::spin_loop();
    }
}

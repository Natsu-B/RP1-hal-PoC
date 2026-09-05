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

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
unsafe extern "C" {
    fn UART1_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
unsafe extern "C" {
    fn UART2_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
unsafe extern "C" {
    fn UART3_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
unsafe extern "C" {
    fn UART4_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
unsafe extern "C" {
    fn UART5_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
unsafe extern "C" {
    fn PWM0_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "spi0-local-irq"))]
unsafe extern "C" {
    fn SPI0_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq"))]
unsafe extern "C" {
    fn I2C1_IRQHandler();
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
unsafe extern "C" {
    fn TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler();
}

#[cfg(target_arch = "arm")]
unsafe extern "C" {
    static mut __sbss: u8;
    static mut __ebss: u8;
}

#[cfg(all(
    target_arch = "arm",
    not(any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate",
        feature = "expected-fault-recovery"
    ))
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

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
unsafe extern "C" {
    fn ExpectedFaultHandler();
    fn ExpectedFaultTriggerUdf(record: *mut u32);
    fn ExpectedFaultProbeRead(record: *mut u32, address: *const u32);
    fn ExpectedFaultProbeWrite(record: *mut u32, address: *mut u32, value: u32);
}

#[cfg(all(
    target_arch = "arm",
    feature = "expected-fault-recovery",
    not(any(
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    ))
))]
#[unsafe(link_section = ".vector_table")]
#[used]
pub static VECTOR_TABLE: [unsafe extern "C" fn(); 16] = [
    _stack_start,
    Reset,
    DefaultHandler,
    ExpectedFaultHandler,
    ExpectedFaultHandler,
    ExpectedFaultHandler,
    ExpectedFaultHandler,
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
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const fn local_irq_vector_table() -> [unsafe extern "C" fn(); 80] {
    let mut vectors = [DefaultHandler as unsafe extern "C" fn(); 80];
    vectors[0] = _stack_start;
    vectors[1] = Reset;
    #[cfg(feature = "expected-fault-recovery")]
    {
        vectors[3] = ExpectedFaultHandler;
        vectors[4] = ExpectedFaultHandler;
        vectors[5] = ExpectedFaultHandler;
        vectors[6] = ExpectedFaultHandler;
    }
    #[cfg(feature = "pwm0-local-irq")]
    {
        vectors[PWM0_VECTOR_INDEX] = PWM0_IRQHandler;
    }
    #[cfg(feature = "spi0-local-irq")]
    {
        vectors[SPI0_VECTOR_INDEX] = SPI0_IRQHandler;
    }
    #[cfg(feature = "i2c1-local-irq")]
    {
        vectors[I2C1_VECTOR_INDEX] = I2C1_IRQHandler;
    }
    #[cfg(feature = "timer0-alarm0-irq26-candidate")]
    {
        vectors[TIMER0_ALARM0_IRQ26_CANDIDATE_VECTOR_INDEX] =
            TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler;
    }
    #[cfg(feature = "uart0-rx-irq")]
    {
        vectors[UART0_VECTOR_INDEX] = UART0_IRQHandler;
    }
    #[cfg(feature = "uart1-local-irq")]
    {
        vectors[UART1_VECTOR_INDEX] = UART1_IRQHandler;
    }
    #[cfg(feature = "uart2-local-irq")]
    {
        vectors[UART2_VECTOR_INDEX] = UART2_IRQHandler;
    }
    #[cfg(feature = "uart3-local-irq")]
    {
        vectors[UART3_VECTOR_INDEX] = UART3_IRQHandler;
    }
    #[cfg(feature = "uart4-local-irq")]
    {
        vectors[UART4_VECTOR_INDEX] = UART4_IRQHandler;
    }
    #[cfg(feature = "uart5-local-irq")]
    {
        vectors[UART5_VECTOR_INDEX] = UART5_IRQHandler;
    }
    vectors
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
#[unsafe(link_section = ".vector_table")]
#[used]
pub static VECTOR_TABLE: [unsafe extern "C" fn(); 80] = local_irq_vector_table();

pub const UART0_IRQ_NUMBER: usize = 25;
pub const UART0_VECTOR_INDEX: usize = 16 + UART0_IRQ_NUMBER;
pub const UART1_IRQ_NUMBER: usize = 42;
pub const UART1_VECTOR_INDEX: usize = 16 + UART1_IRQ_NUMBER;
pub const UART2_IRQ_NUMBER: usize = 43;
pub const UART2_VECTOR_INDEX: usize = 16 + UART2_IRQ_NUMBER;
pub const UART3_IRQ_NUMBER: usize = 44;
pub const UART3_VECTOR_INDEX: usize = 16 + UART3_IRQ_NUMBER;
pub const UART4_IRQ_NUMBER: usize = 45;
pub const UART4_VECTOR_INDEX: usize = 16 + UART4_IRQ_NUMBER;
pub const UART5_IRQ_NUMBER: usize = 46;
pub const UART5_VECTOR_INDEX: usize = 16 + UART5_IRQ_NUMBER;
pub const PWM0_IRQ_NUMBER: usize = 5;
pub const PWM0_VECTOR_INDEX: usize = 16 + PWM0_IRQ_NUMBER;
pub const SPI0_IRQ_NUMBER: usize = 19;
pub const SPI0_VECTOR_INDEX: usize = 16 + SPI0_IRQ_NUMBER;
pub const I2C1_IRQ_NUMBER: usize = 8;
pub const I2C1_VECTOR_INDEX: usize = 16 + I2C1_IRQ_NUMBER;
pub const TIMER0_ALARM0_IRQ26_CANDIDATE_NUMBER: usize = 26;
pub const TIMER0_ALARM0_IRQ26_CANDIDATE_VECTOR_INDEX: usize =
    16 + TIMER0_ALARM0_IRQ26_CANDIDATE_NUMBER;

#[cfg(all(target_arch = "arm", feature = "uart0-rx-irq"))]
const UART0_IRQ_BIT: u32 = 1 << UART0_IRQ_NUMBER;
#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
const UART1_IRQ_BIT1: u32 = 1 << (UART1_IRQ_NUMBER - 32);
#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
const UART2_IRQ_BIT1: u32 = 1 << (UART2_IRQ_NUMBER - 32);
#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
const UART3_IRQ_BIT1: u32 = 1 << (UART3_IRQ_NUMBER - 32);
#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
const UART4_IRQ_BIT1: u32 = 1 << (UART4_IRQ_NUMBER - 32);
#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
const UART5_IRQ_BIT1: u32 = 1 << (UART5_IRQ_NUMBER - 32);
#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
const PWM0_IRQ_BIT: u32 = 1 << PWM0_IRQ_NUMBER;
#[cfg(all(target_arch = "arm", feature = "spi0-local-irq"))]
const SPI0_IRQ_BIT: u32 = 1 << SPI0_IRQ_NUMBER;
#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq"))]
const I2C1_IRQ_BIT: u32 = 1 << I2C1_IRQ_NUMBER;
#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
const TIMER0_ALARM0_IRQ26_CANDIDATE_BIT: u32 = 1 << TIMER0_ALARM0_IRQ26_CANDIDATE_NUMBER;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const VECTOR_TABLE_BASE: u32 = 0x2000_0000;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate",
        feature = "expected-fault-recovery"
    )
))]
const SCB_VTOR: *mut u32 = 0xe000_ed08 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_ISER0: *mut u32 = 0xe000_e100 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_ISER1: *mut u32 = 0xe000_e104 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_ICER0: *mut u32 = 0xe000_e180 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
    )
))]
const NVIC_ICER1: *mut u32 = 0xe000_e184 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "i2c1-local-irq",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_ICPR0: *mut u32 = 0xe000_e280 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
    )
))]
const NVIC_ICPR1: *mut u32 = 0xe000_e284 as *mut u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_ISPR0: *const u32 = 0xe000_e200 as *const u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_ISPR1: *const u32 = 0xe000_e204 as *const u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_IABR0: *const u32 = 0xe000_e300 as *const u32;
#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate"
    )
))]
const NVIC_IABR1: *const u32 = 0xe000_e304 as *const u32;
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

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart1IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart1IrqSaved {
    enabled: bool,
    primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart2IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart2IrqSaved {
    enabled: bool,
    primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart3IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart3IrqSaved {
    enabled: bool,
    primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart4IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart4IrqSaved {
    enabled: bool,
    primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart5IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
#[derive(Clone, Copy)]
pub struct Uart5IrqSaved {
    enabled: bool,
    primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
#[derive(Clone, Copy)]
pub struct Pwm0IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub iabr0: u32,
    pub primask: u32,
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout"
    )
))]
#[derive(Clone, Copy)]
pub struct Spi0IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout"
    )
))]
#[derive(Clone, Copy)]
pub struct I2c1IrqRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub ispr1: u32,
    pub iabr0: u32,
    pub iabr1: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
#[derive(Clone, Copy)]
pub struct Timer0Alarm0Irq26CandidateRouteSnapshot {
    pub vtor: u32,
    pub iser0: u32,
    pub iser1: u32,
    pub ispr0: u32,
    pub iabr0: u32,
    pub primask: u32,
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
impl Timer0Alarm0Irq26CandidateRouteSnapshot {
    pub fn pack(self) -> u32 {
        let bit = TIMER0_ALARM0_IRQ26_CANDIDATE_BIT;
        u32::from(self.iser0 & bit != 0)
            | (u32::from(self.ispr0 & bit != 0) << 1)
            | (u32::from(self.iabr0 & bit != 0) << 2)
            | ((self.primask & 1) << 3)
            | (u32::from(self.vtor == VECTOR_TABLE_BASE) << 4)
            | (u32::from(self.iser0 & !bit != 0) << 8)
            | (u32::from(self.iser1 != 0) << 9)
            | (u32::from(self.ispr0 & !bit != 0) << 10)
            | (u32::from(self.iabr0 & !bit != 0) << 11)
    }
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

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
pub fn uart1_irq_route_snapshot() -> Uart1IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Uart1IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
pub fn uart2_irq_route_snapshot() -> Uart2IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Uart2IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
pub fn uart3_irq_route_snapshot() -> Uart3IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Uart3IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
pub fn uart4_irq_route_snapshot() -> Uart4IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Uart4IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
pub fn uart5_irq_route_snapshot() -> Uart5IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Uart5IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
pub fn pwm0_irq_route_snapshot() -> Pwm0IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Pwm0IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            primask,
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout"
    )
))]
pub fn spi0_irq_route_snapshot() -> Spi0IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Spi0IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout"
    )
))]
pub fn i2c1_irq_route_snapshot() -> I2c1IrqRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        I2c1IrqRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            ispr1: core::ptr::read_volatile(NVIC_ISPR1),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
            iabr1: core::ptr::read_volatile(NVIC_IABR1),
            primask,
        }
    }
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
pub fn timer0_alarm0_irq26_candidate_route_snapshot() -> Timer0Alarm0Irq26CandidateRouteSnapshot {
    let primask: u32;
    unsafe {
        core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack, preserves_flags));
        Timer0Alarm0Irq26CandidateRouteSnapshot {
            vtor: core::ptr::read_volatile(SCB_VTOR),
            iser0: core::ptr::read_volatile(NVIC_ISER0),
            iser1: core::ptr::read_volatile(NVIC_ISER1),
            ispr0: core::ptr::read_volatile(NVIC_ISPR0),
            iabr0: core::ptr::read_volatile(NVIC_IABR0),
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
    if before.vtor != VECTOR_TABLE_BASE || before.iser0 & !UART0_IRQ_BIT != 0 || before.iser1 != 0 {
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

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
pub unsafe fn prepare_uart1_irq() -> Option<Uart1IrqSaved> {
    let before = uart1_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE {
        return None;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART1_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART1_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    Some(Uart1IrqSaved {
        enabled: before.iser1 & UART1_IRQ_BIT1 != 0,
        primask: before.primask,
    })
}

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
pub unsafe fn enable_uart1_irq_after_source_asserted() {
    unsafe {
        core::ptr::write_volatile(NVIC_ISER1, UART1_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart1-local-irq"))]
pub unsafe fn restore_uart1_irq(saved: Uart1IrqSaved) {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART1_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART1_IRQ_BIT1);
        if saved.enabled {
            core::ptr::write_volatile(NVIC_ISER1, UART1_IRQ_BIT1);
        }
        core::arch::asm!("msr PRIMASK, {}", in(reg) saved.primask & 1, options(nostack, preserves_flags));
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
pub unsafe fn prepare_uart2_irq() -> Option<Uart2IrqSaved> {
    let before = uart2_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE {
        return None;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART2_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART2_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    Some(Uart2IrqSaved {
        enabled: before.iser1 & UART2_IRQ_BIT1 != 0,
        primask: before.primask,
    })
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
pub unsafe fn enable_uart2_irq_after_source_asserted() {
    unsafe {
        core::ptr::write_volatile(NVIC_ISER1, UART2_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart2-local-irq"))]
pub unsafe fn restore_uart2_irq(saved: Uart2IrqSaved) {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART2_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART2_IRQ_BIT1);
        if saved.enabled {
            core::ptr::write_volatile(NVIC_ISER1, UART2_IRQ_BIT1);
        }
        core::arch::asm!("msr PRIMASK, {}", in(reg) saved.primask & 1, options(nostack, preserves_flags));
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
pub unsafe fn prepare_uart3_irq() -> Option<Uart3IrqSaved> {
    let before = uart3_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE {
        return None;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART3_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART3_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    Some(Uart3IrqSaved {
        enabled: before.iser1 & UART3_IRQ_BIT1 != 0,
        primask: before.primask,
    })
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
pub unsafe fn enable_uart3_irq_after_source_asserted() {
    unsafe {
        core::ptr::write_volatile(NVIC_ISER1, UART3_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart3-local-irq"))]
pub unsafe fn restore_uart3_irq(saved: Uart3IrqSaved) {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART3_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART3_IRQ_BIT1);
        if saved.enabled {
            core::ptr::write_volatile(NVIC_ISER1, UART3_IRQ_BIT1);
        }
        core::arch::asm!("msr PRIMASK, {}", in(reg) saved.primask & 1, options(nostack, preserves_flags));
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
pub unsafe fn prepare_uart4_irq() -> Option<Uart4IrqSaved> {
    let before = uart4_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE {
        return None;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART4_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART4_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    Some(Uart4IrqSaved {
        enabled: before.iser1 & UART4_IRQ_BIT1 != 0,
        primask: before.primask,
    })
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
pub unsafe fn enable_uart4_irq_after_source_asserted() {
    unsafe {
        core::ptr::write_volatile(NVIC_ISER1, UART4_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart4-local-irq"))]
pub unsafe fn restore_uart4_irq(saved: Uart4IrqSaved) {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART4_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART4_IRQ_BIT1);
        if saved.enabled {
            core::ptr::write_volatile(NVIC_ISER1, UART4_IRQ_BIT1);
        }
        core::arch::asm!("msr PRIMASK, {}", in(reg) saved.primask & 1, options(nostack, preserves_flags));
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
pub unsafe fn prepare_uart5_irq() -> Option<Uart5IrqSaved> {
    let before = uart5_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE {
        return None;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART5_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART5_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    Some(Uart5IrqSaved {
        enabled: before.iser1 & UART5_IRQ_BIT1 != 0,
        primask: before.primask,
    })
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
pub unsafe fn enable_uart5_irq_after_source_asserted() {
    unsafe {
        core::ptr::write_volatile(NVIC_ISER1, UART5_IRQ_BIT1);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "uart5-local-irq"))]
pub unsafe fn restore_uart5_irq(saved: Uart5IrqSaved) {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, UART5_IRQ_BIT1);
        core::ptr::write_volatile(NVIC_ICPR1, UART5_IRQ_BIT1);
        if saved.enabled {
            core::ptr::write_volatile(NVIC_ISER1, UART5_IRQ_BIT1);
        }
        core::arch::asm!("msr PRIMASK, {}", in(reg) saved.primask & 1, options(nostack, preserves_flags));
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
pub unsafe fn prepare_pwm0_irq() -> bool {
    let before = pwm0_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE || before.iser0 & !PWM0_IRQ_BIT != 0 || before.iser1 != 0 {
        return false;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, PWM0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, PWM0_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    true
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
pub unsafe fn enable_pwm0_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICPR0, PWM0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ISER0, PWM0_IRQ_BIT);
        core::arch::asm!(
            "dsb sy",
            "isb",
            "cpsie i",
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(all(target_arch = "arm", feature = "pwm0-local-irq"))]
pub unsafe fn disable_pwm0_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, PWM0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, PWM0_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "spi0-local-irq"))]
pub unsafe fn prepare_spi0_irq() -> bool {
    let before = spi0_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE || before.iser0 & !SPI0_IRQ_BIT != 0 || before.iser1 != 0 {
        return false;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, SPI0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, SPI0_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    true
}

#[cfg(all(target_arch = "arm", feature = "spi0-local-irq"))]
pub unsafe fn enable_spi0_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICPR0, SPI0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ISER0, SPI0_IRQ_BIT);
        core::arch::asm!(
            "dsb sy",
            "isb",
            "cpsie i",
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(all(target_arch = "arm", feature = "spi0-local-irq"))]
pub unsafe fn disable_spi0_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, SPI0_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, SPI0_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq"))]
pub unsafe fn prepare_i2c1_irq() -> bool {
    let before = i2c1_irq_route_snapshot();
    if before.vtor != VECTOR_TABLE_BASE || before.iser0 & !I2C1_IRQ_BIT != 0 || before.iser1 != 0 {
        return false;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, I2C1_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, I2C1_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    true
}

#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq"))]
pub unsafe fn enable_i2c1_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICPR0, I2C1_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ISER0, I2C1_IRQ_BIT);
        core::arch::asm!(
            "dsb sy",
            "isb",
            "cpsie i",
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(all(target_arch = "arm", feature = "i2c1-local-irq"))]
pub unsafe fn disable_i2c1_irq() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, I2C1_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, I2C1_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
pub unsafe fn prepare_timer0_alarm0_irq26_candidate() -> bool {
    let before = timer0_alarm0_irq26_candidate_route_snapshot();
    let bit = TIMER0_ALARM0_IRQ26_CANDIDATE_BIT;
    if before.vtor != VECTOR_TABLE_BASE
        || before.iser0 & !bit != 0
        || before.iser1 != 0
        || before.ispr0 & !bit != 0
        || before.iabr0 & !bit != 0
        || before.primask != 0
    {
        return false;
    }

    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, bit);
        core::ptr::write_volatile(NVIC_ICPR0, bit);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
    timer0_alarm0_irq26_candidate_route_snapshot().pack() == 0x10
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
pub unsafe fn enable_timer0_alarm0_irq26_candidate() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICPR0, TIMER0_ALARM0_IRQ26_CANDIDATE_BIT);
        core::ptr::write_volatile(NVIC_ISER0, TIMER0_ALARM0_IRQ26_CANDIDATE_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "timer0-alarm0-irq26-candidate"))]
pub unsafe fn disable_timer0_alarm0_irq26_candidate() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER0, TIMER0_ALARM0_IRQ26_CANDIDATE_BIT);
        core::ptr::write_volatile(NVIC_ICPR0, TIMER0_ALARM0_IRQ26_CANDIDATE_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate",
        feature = "expected-fault-recovery"
    )
))]
unsafe fn configure_vector_table() {
    let address = core::ptr::addr_of!(VECTOR_TABLE) as usize as u32;
    unsafe {
        core::ptr::write_volatile(SCB_VTOR, address);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_FAULT_ACTIVE_MAGIC: u32 = 0x3152_4645; // EFR1
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_FAULT_KIND_UDF: u32 = 1;
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_FAULT_KIND_DATA_READ: u32 = 2;
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_FAULT_KIND_DATA_WRITE: u32 = 3;
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_FAULT_RECOVERED: u32 = 0x5643_4552; // RECV
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_FAULT_WORDS: usize = 19;

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
mod expected_fault_index {
    pub const ACTIVE: usize = 0;
    pub const SEQUENCE: usize = 1;
    pub const KIND: usize = 2;
    pub const PROBE_PC: usize = 3;
    pub const RESUME_PC: usize = 4;
    pub const EXCEPTION: usize = 5;
    pub const EXC_RETURN: usize = 6;
    pub const STACKED_PC: usize = 7;
    pub const STACKED_LR: usize = 8;
    pub const STACKED_XPSR: usize = 9;
    pub const CFSR: usize = 10;
    pub const HFSR: usize = 11;
    pub const BFAR: usize = 12;
    pub const MMFAR: usize = 13;
    pub const COMPLETION: usize = 14;
    pub const HANDLER_COUNT: usize = 15;
    pub const TARGET_ADDRESS: usize = 16;
    pub const ACCESS_RESULT: usize = 17;
    pub const FAULTED: usize = 18;
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[repr(C, align(16))]
struct ExpectedFaultRecord {
    words: [u32; EXPECTED_FAULT_WORDS],
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[unsafe(no_mangle)]
static mut EXPECTED_FAULT_RECORD: ExpectedFaultRecord = ExpectedFaultRecord {
    words: [0; EXPECTED_FAULT_WORDS],
};

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[derive(Clone, Copy)]
pub struct ExpectedFaultSnapshot {
    pub active: u32,
    pub sequence: u32,
    pub kind: u32,
    pub probe_pc: u32,
    pub resume_pc: u32,
    pub exception: u32,
    pub exc_return: u32,
    pub stacked_pc: u32,
    pub stacked_lr: u32,
    pub stacked_xpsr: u32,
    pub cfsr: u32,
    pub hfsr: u32,
    pub bfar: u32,
    pub mmfar: u32,
    pub completion: u32,
    pub handler_count: u32,
    pub target_address: u32,
    pub access_result: u32,
    pub faulted: u32,
}

#[cfg(any(test, all(target_arch = "arm", feature = "expected-fault-recovery")))]
const fn precise_bus_data_fault(
    exception: u32,
    cfsr: u32,
    hfsr: u32,
    bfar: u32,
    target_address: u32,
) -> bool {
    (exception == 3 || exception == 5)
        && cfsr & (1 << 9) != 0
        && cfsr & ((1 << 10) | (1 << 11) | (1 << 12)) == 0
        && cfsr & (1 << 15) != 0
        && cfsr & 0xff == 0
        && cfsr & 0xffff_0000 == 0
        && bfar == target_address
        && (exception != 3 || hfsr & (1 << 30) != 0)
        && hfsr & (1 << 1) == 0
}

#[cfg(any(test, all(target_arch = "arm", feature = "expected-fault-recovery")))]
const fn precise_memmanage_data_fault(
    exception: u32,
    cfsr: u32,
    hfsr: u32,
    mmfar: u32,
    target_address: u32,
) -> bool {
    const DACCVIOL: u32 = 1 << 1;
    const MMARVALID: u32 = 1 << 7;

    exception == 4 && cfsr == DACCVIOL | MMARVALID && hfsr == 0 && mmfar == target_address
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
impl ExpectedFaultSnapshot {
    pub fn recovered(self) -> bool {
        let expected_exception = self.exception == 3 || self.exception == 6;
        let expected_status = self.cfsr & (1 << 16) != 0
            && self.cfsr & (1 << 10) == 0
            && (self.exception != 3 || self.hfsr & (1 << 30) != 0);
        self.active == 0
            && self.sequence == 1
            && self.kind == EXPECTED_FAULT_KIND_UDF
            && expected_exception
            && expected_status
            && self.probe_pc == self.stacked_pc
            && self.probe_pc != self.resume_pc
            && self.completion == EXPECTED_FAULT_RECOVERED
            && self.handler_count == 1
    }

    pub fn data_access_succeeded(self, expected_kind: u32) -> bool {
        self.active == 0
            && self.sequence == 1
            && self.kind == expected_kind
            && self.exception == 0
            && self.cfsr == 0
            && self.hfsr == 0
            && self.completion == 0
            && self.handler_count == 0
            && self.faulted == 0
    }

    pub fn data_read_succeeded(self) -> bool {
        self.data_access_succeeded(EXPECTED_FAULT_KIND_DATA_READ)
    }

    pub fn data_write_succeeded(self) -> bool {
        self.data_access_succeeded(EXPECTED_FAULT_KIND_DATA_WRITE)
    }

    pub fn recovered_data_access_fault(self, expected_kind: u32, expected_address: u32) -> bool {
        let expected_status = precise_bus_data_fault(
            self.exception,
            self.cfsr,
            self.hfsr,
            self.bfar,
            expected_address,
        ) || precise_memmanage_data_fault(
            self.exception,
            self.cfsr,
            self.hfsr,
            self.mmfar,
            expected_address,
        );
        self.active == 0
            && self.sequence == 1
            && self.kind == expected_kind
            && expected_status
            && self.target_address == expected_address
            && self.probe_pc == self.stacked_pc
            && self.probe_pc != self.resume_pc
            && self.completion == EXPECTED_FAULT_RECOVERED
            && self.handler_count == 1
            && self.faulted == 1
    }

    pub fn recovered_data_read_fault(self, expected_address: u32) -> bool {
        self.recovered_data_access_fault(EXPECTED_FAULT_KIND_DATA_READ, expected_address)
    }

    pub fn recovered_data_write_fault(self, expected_address: u32) -> bool {
        self.recovered_data_access_fault(EXPECTED_FAULT_KIND_DATA_WRITE, expected_address)
    }

    pub fn recovered_memmanage_data_read_fault(self, expected_address: u32) -> bool {
        precise_memmanage_data_fault(
            self.exception,
            self.cfsr,
            self.hfsr,
            self.mmfar,
            expected_address,
        ) && self.recovered_data_read_fault(expected_address)
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_SCB_CFSR: *mut u32 = 0xe000_ed28 as *mut u32;
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_SCB_HFSR: *mut u32 = 0xe000_ed2c as *mut u32;
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_SCB_MMFAR: *const u32 = 0xe000_ed34 as *const u32;
#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
const EXPECTED_SCB_BFAR: *const u32 = 0xe000_ed38 as *const u32;

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[inline(always)]
unsafe fn expected_fault_base() -> *mut u32 {
    core::ptr::addr_of_mut!(EXPECTED_FAULT_RECORD).cast::<u32>()
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[inline(always)]
unsafe fn expected_fault_read(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(expected_fault_base().add(index)) }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[inline(always)]
unsafe fn expected_fault_write(index: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(expected_fault_base().add(index), value);
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.ExpectedFaultHandler,"ax",%progbits
    .global ExpectedFaultHandler
    .type ExpectedFaultHandler,%function
    .thumb_func
ExpectedFaultHandler:
    mov r1, lr
    tst r1, #4
    ite eq
    mrseq r0, msp
    mrsne r0, psp
    mrs r2, ipsr
    b expected_fault_rust
    .size ExpectedFaultHandler, . - ExpectedFaultHandler

    .section .text.ExpectedFaultTriggerUdf,"ax",%progbits
    .global ExpectedFaultTriggerUdf
    .type ExpectedFaultTriggerUdf,%function
    .thumb_func
ExpectedFaultTriggerUdf:
    ldr r1, =0x31524645
    str r1, [r0, #0]
    adr r1, .Lexpected_udf_probe
    str r1, [r0, #12]
    adr r1, .Lexpected_udf_resume
    str r1, [r0, #16]
    dsb sy
    isb
.Lexpected_udf_probe:
    udf #0
.Lexpected_udf_resume:
    movs r1, #0
    str r1, [r0, #0]
    dsb sy
    bx lr
    .size ExpectedFaultTriggerUdf, . - ExpectedFaultTriggerUdf

    .section .text.ExpectedFaultProbeRead,"ax",%progbits
    .global ExpectedFaultProbeRead
    .type ExpectedFaultProbeRead,%function
    .thumb_func
ExpectedFaultProbeRead:
    ldr r2, =0x31524645
    str r2, [r0, #0]
    adr r2, .Lexpected_read_probe
    str r2, [r0, #12]
    adr r2, .Lexpected_read_fault_resume
    str r2, [r0, #16]
    dsb sy
    isb
.Lexpected_read_probe:
    ldr r2, [r1]
    str r2, [r0, #68]
    b .Lexpected_read_complete
.Lexpected_read_fault_resume:
    nop
.Lexpected_read_complete:
    movs r2, #0
    str r2, [r0, #0]
    dsb sy
    bx lr
    .size ExpectedFaultProbeRead, . - ExpectedFaultProbeRead

    .section .text.ExpectedFaultProbeWrite,"ax",%progbits
    .global ExpectedFaultProbeWrite
    .type ExpectedFaultProbeWrite,%function
    .thumb_func
ExpectedFaultProbeWrite:
    ldr r3, =0x31524645
    str r3, [r0, #0]
    adr r3, .Lexpected_write_probe
    str r3, [r0, #12]
    adr r3, .Lexpected_write_fault_resume
    str r3, [r0, #16]
    dsb sy
    isb
.Lexpected_write_probe:
    str r2, [r1]
    b .Lexpected_write_complete
.Lexpected_write_fault_resume:
    nop
.Lexpected_write_complete:
    movs r3, #0
    str r3, [r0, #0]
    dsb sy
    bx lr
    .size ExpectedFaultProbeWrite, . - ExpectedFaultProbeWrite
"#
);

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[inline(never)]
fn expected_fault_fail_stop() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
#[unsafe(no_mangle)]
unsafe extern "C" fn expected_fault_rust(frame: *mut u32, exc_return: u32, exception: u32) {
    let frame_address = frame as usize;
    if !(0x2000_0000..=0x2000_ffe0).contains(&frame_address) || frame_address & 3 != 0 {
        expected_fault_fail_stop();
    }

    let stacked_lr = unsafe { core::ptr::read_volatile(frame.add(5)) };
    let stacked_pc = unsafe { core::ptr::read_volatile(frame.add(6)) };
    let stacked_xpsr = unsafe { core::ptr::read_volatile(frame.add(7)) };
    let cfsr = unsafe { core::ptr::read_volatile(EXPECTED_SCB_CFSR) };
    let hfsr = unsafe { core::ptr::read_volatile(EXPECTED_SCB_HFSR) };
    let bfar = unsafe { core::ptr::read_volatile(EXPECTED_SCB_BFAR) };
    let mmfar = unsafe { core::ptr::read_volatile(EXPECTED_SCB_MMFAR) };

    unsafe {
        expected_fault_write(expected_fault_index::EXCEPTION, exception);
        expected_fault_write(expected_fault_index::EXC_RETURN, exc_return);
        expected_fault_write(expected_fault_index::STACKED_PC, stacked_pc);
        expected_fault_write(expected_fault_index::STACKED_LR, stacked_lr);
        expected_fault_write(expected_fault_index::STACKED_XPSR, stacked_xpsr);
        expected_fault_write(expected_fault_index::CFSR, cfsr);
        expected_fault_write(expected_fault_index::HFSR, hfsr);
        expected_fault_write(expected_fault_index::BFAR, bfar);
        expected_fault_write(expected_fault_index::MMFAR, mmfar);
    }

    let active = unsafe { expected_fault_read(expected_fault_index::ACTIVE) };
    let sequence = unsafe { expected_fault_read(expected_fault_index::SEQUENCE) };
    let kind = unsafe { expected_fault_read(expected_fault_index::KIND) };
    let probe_pc = unsafe { expected_fault_read(expected_fault_index::PROBE_PC) };
    let resume_pc = unsafe { expected_fault_read(expected_fault_index::RESUME_PC) };
    let target_address = unsafe { expected_fault_read(expected_fault_index::TARGET_ADDRESS) };
    let udf_fault = kind == EXPECTED_FAULT_KIND_UDF
        && (exception == 3 || exception == 6)
        && cfsr & (1 << 16) != 0
        && cfsr & 0xffff == 0;
    let data_fault = (kind == EXPECTED_FAULT_KIND_DATA_READ
        || kind == EXPECTED_FAULT_KIND_DATA_WRITE)
        && (precise_bus_data_fault(exception, cfsr, hfsr, bfar, target_address)
            || precise_memmanage_data_fault(exception, cfsr, hfsr, mmfar, target_address));
    let expected_hfsr = exception != 3 || hfsr & (1 << 30) != 0;
    let safe_resume = (0x2000_0000..0x2001_0000).contains(&(resume_pc as usize));

    if active != EXPECTED_FAULT_ACTIVE_MAGIC
        || sequence != 1
        || !(udf_fault || data_fault)
        || !expected_hfsr
        || hfsr & (1 << 1) != 0
        || stacked_pc != probe_pc
        || !safe_resume
        || resume_pc == probe_pc
    {
        expected_fault_fail_stop();
    }

    unsafe {
        core::ptr::write_volatile(frame.add(6), resume_pc & !1);
        expected_fault_write(expected_fault_index::COMPLETION, EXPECTED_FAULT_RECOVERED);
        expected_fault_write(expected_fault_index::HANDLER_COUNT, 1);
        if data_fault {
            expected_fault_write(expected_fault_index::FAULTED, 1);
        }
        core::ptr::write_volatile(EXPECTED_SCB_CFSR, cfsr);
        core::ptr::write_volatile(EXPECTED_SCB_HFSR, hfsr);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
unsafe fn prepare_expected_fault(kind: u32, target_address: u32) {
    let cfsr_before = unsafe { core::ptr::read_volatile(EXPECTED_SCB_CFSR) };
    let hfsr_before = unsafe { core::ptr::read_volatile(EXPECTED_SCB_HFSR) };
    unsafe {
        if cfsr_before != 0 {
            core::ptr::write_volatile(EXPECTED_SCB_CFSR, cfsr_before);
        }
        if hfsr_before != 0 {
            core::ptr::write_volatile(EXPECTED_SCB_HFSR, hfsr_before);
        }
        for index in 0..EXPECTED_FAULT_WORDS {
            expected_fault_write(index, 0);
        }
        expected_fault_write(expected_fault_index::SEQUENCE, 1);
        expected_fault_write(expected_fault_index::KIND, kind);
        expected_fault_write(expected_fault_index::TARGET_ADDRESS, target_address);
        expected_fault_write(expected_fault_index::ACCESS_RESULT, u32::MAX);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
unsafe fn expected_fault_snapshot() -> ExpectedFaultSnapshot {
    ExpectedFaultSnapshot {
        active: unsafe { expected_fault_read(expected_fault_index::ACTIVE) },
        sequence: unsafe { expected_fault_read(expected_fault_index::SEQUENCE) },
        kind: unsafe { expected_fault_read(expected_fault_index::KIND) },
        probe_pc: unsafe { expected_fault_read(expected_fault_index::PROBE_PC) },
        resume_pc: unsafe { expected_fault_read(expected_fault_index::RESUME_PC) },
        exception: unsafe { expected_fault_read(expected_fault_index::EXCEPTION) },
        exc_return: unsafe { expected_fault_read(expected_fault_index::EXC_RETURN) },
        stacked_pc: unsafe { expected_fault_read(expected_fault_index::STACKED_PC) },
        stacked_lr: unsafe { expected_fault_read(expected_fault_index::STACKED_LR) },
        stacked_xpsr: unsafe { expected_fault_read(expected_fault_index::STACKED_XPSR) },
        cfsr: unsafe { expected_fault_read(expected_fault_index::CFSR) },
        hfsr: unsafe { expected_fault_read(expected_fault_index::HFSR) },
        bfar: unsafe { expected_fault_read(expected_fault_index::BFAR) },
        mmfar: unsafe { expected_fault_read(expected_fault_index::MMFAR) },
        completion: unsafe { expected_fault_read(expected_fault_index::COMPLETION) },
        handler_count: unsafe { expected_fault_read(expected_fault_index::HANDLER_COUNT) },
        target_address: unsafe { expected_fault_read(expected_fault_index::TARGET_ADDRESS) },
        access_result: unsafe { expected_fault_read(expected_fault_index::ACCESS_RESULT) },
        faulted: unsafe { expected_fault_read(expected_fault_index::FAULTED) },
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
pub unsafe fn run_expected_udf_recovery() -> ExpectedFaultSnapshot {
    unsafe {
        prepare_expected_fault(EXPECTED_FAULT_KIND_UDF, 0);
        ExpectedFaultTriggerUdf(expected_fault_base());
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        expected_fault_snapshot()
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
pub unsafe fn run_expected_data_read(address: *const u32) -> ExpectedFaultSnapshot {
    unsafe {
        prepare_expected_fault(EXPECTED_FAULT_KIND_DATA_READ, address as usize as u32);
        ExpectedFaultProbeRead(expected_fault_base(), address);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        expected_fault_snapshot()
    }
}

#[cfg(all(target_arch = "arm", feature = "expected-fault-recovery"))]
pub unsafe fn run_expected_data_write(address: *mut u32, value: u32) -> ExpectedFaultSnapshot {
    unsafe {
        prepare_expected_fault(EXPECTED_FAULT_KIND_DATA_WRITE, address as usize as u32);
        ExpectedFaultProbeWrite(expected_fault_base(), address, value);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        expected_fault_snapshot()
    }
}

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() {
    unsafe {
        zero_bss();
    }
    #[cfg(any(
        feature = "uart0-rx-irq",
        feature = "uart1-local-irq",
        feature = "uart2-local-irq",
        feature = "uart3-local-irq",
        feature = "uart4-local-irq",
        feature = "uart5-local-irq",
        feature = "pwm0-local-irq",
        feature = "spi0-local-irq",
        feature = "spi0-local-irq-bank1-passive-scout",
        feature = "i2c1-local-irq",
        feature = "i2c1-local-irq-bank1-passive-scout",
        feature = "timer0-alarm0-irq26-candidate",
        feature = "expected-fault-recovery"
    ))]
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

#[cfg(test)]
mod tests {
    use super::{precise_bus_data_fault, precise_memmanage_data_fault};

    #[test]
    fn accepts_exact_memmanage_data_violation() {
        assert!(precise_memmanage_data_fault(
            4,
            (1 << 1) | (1 << 7),
            0,
            0x2000_0100,
            0x2000_0100,
        ));
    }

    #[test]
    fn rejects_inexact_memmanage_faults() {
        let exact = (1 << 1) | (1 << 7);
        assert!(!precise_memmanage_data_fault(
            3,
            exact,
            0,
            0x2000_0100,
            0x2000_0100,
        ));
        assert!(!precise_memmanage_data_fault(
            4,
            exact | (1 << 4),
            0,
            0x2000_0100,
            0x2000_0100,
        ));
        assert!(!precise_memmanage_data_fault(
            4,
            exact,
            0,
            0x2000_0120,
            0x2000_0100,
        ));
        assert!(!precise_memmanage_data_fault(
            4,
            exact,
            1 << 30,
            0x2000_0100,
            0x2000_0100,
        ));
    }

    #[test]
    fn preserves_precise_bus_fault_acceptance() {
        let cfsr = (1 << 9) | (1 << 15);
        assert!(precise_bus_data_fault(5, cfsr, 0, 0x4000_0000, 0x4000_0000,));
        assert!(precise_bus_data_fault(
            3,
            cfsr,
            1 << 30,
            0x4000_0000,
            0x4000_0000,
        ));
        assert!(!precise_bus_data_fault(
            3,
            cfsr,
            0,
            0x4000_0000,
            0x4000_0000,
        ));
    }
}

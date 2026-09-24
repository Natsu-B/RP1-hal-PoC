#![no_std]

pub use rp1_abi::debug;
pub use rp1_macros::main;

pub mod addr;
pub mod clock_profile_generated;
pub mod clock_adopt;
pub mod scmi_clock;
pub mod scmi_mailbox;
pub mod gpio;
pub mod i2c;
pub mod i2c_rx_state;
#[cfg(target_arch = "arm")]
pub mod i2c_rx_irq_adapter;
pub mod mailbox;
pub mod mmio;
pub mod owner;
pub mod peripherals;
pub mod prelude;
pub mod pwm;
pub mod reset;
pub mod spi;
pub mod timer;
pub mod uart;

pub use peripherals::Peripherals;

pub fn init() -> Option<Peripherals> {
    Peripherals::take()
}

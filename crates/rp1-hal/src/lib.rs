#![no_std]

pub use rp1_abi::debug;
pub use rp1_macros::main;

pub mod addr;
pub mod gpio;
pub mod i2c;
pub mod mailbox;
pub mod mmio;
pub mod owner;
pub mod peripherals;
pub mod prelude;
pub mod pwm;
#[cfg(feature = "bar2-rpc-v2")]
pub mod rpc;
pub mod spi;
pub mod timer;
pub mod uart;

pub use peripherals::Peripherals;

pub fn init() -> Option<Peripherals> {
    Peripherals::take()
}

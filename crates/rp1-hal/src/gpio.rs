use core::marker::PhantomData;

use crate::mmio::Reg;

const PROC_MISC: usize = 0x4001_7004;
const IO_BANK0_BASE: usize = 0x400d_0000;
const PADS_BANK0_BASE: usize = 0x400f_0000;
const SYS_RIO_OUT: usize = 0x400e_0000;
const SYS_RIO_OE: usize = 0x400e_0004;
const SYS_RIO_IN: usize = 0x400e_0008;

const PROC_MISC_RESET_CLEAR: u32 = 1 << 19;
const FUNCSEL_SYS_RIO: u32 = 0x85;
const PAD_G33_OUTPUT: u32 = 0x56;
const PAD_SCHMITT: u32 = 1 << 1;
const PAD_PULL_MASK: u32 = 0b11 << 2;
const PAD_PULL_UP: u32 = 0b10 << 2;
const PAD_IE: u32 = 1 << 6;
const PAD_OD: u32 = 1 << 7;

pub struct Gpio {
    _private: (),
}

pub struct Pin<const N: u8> {
    _private: (),
}

pub struct Input;
pub struct Output;
pub struct Function<const F: u8>;

pub struct ConfiguredPin<const N: u8, Mode> {
    _mode: PhantomData<Mode>,
}

impl Gpio {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    pub fn pin<const N: u8>(&mut self) -> Pin<N> {
        Pin { _private: () }
    }
}

impl<const N: u8> Pin<N> {
    pub fn into_input(self) -> ConfiguredPin<N, Input> {
        configure_input::<N>(0)
    }

    pub fn into_input_pull_up(self) -> ConfiguredPin<N, Input> {
        configure_input::<N>(PAD_PULL_UP)
    }

    pub fn into_output(self) -> ConfiguredPin<N, Output> {
        assert!(N < 32, "SYS_RIO GPIO output currently supports pins 0..31");
        reg(PROC_MISC).write(PROC_MISC_RESET_CLEAR);
        reg(gpio_ctrl_addr(N)).write(FUNCSEL_SYS_RIO);
        reg(gpio_pad_addr(N)).write(PAD_G33_OUTPUT);
        reg(SYS_RIO_OE).modify(|value| value | gpio_bit(N));
        reg(SYS_RIO_OUT).modify(|value| value & !gpio_bit(N));
        ConfiguredPin { _mode: PhantomData }
    }

    pub fn into_function<const F: u8>(self) -> ConfiguredPin<N, Function<F>> {
        // TODO: write actual function select once RP1 GPIO register offsets are verified.
        ConfiguredPin { _mode: PhantomData }
    }
}

impl<const N: u8> ConfiguredPin<N, Input> {
    pub fn is_high(&self) -> bool {
        assert!(N < 32, "SYS_RIO GPIO input currently supports pins 0..31");
        reg(SYS_RIO_IN).read() & gpio_bit(N) != 0
    }

    pub fn is_low(&self) -> bool {
        !self.is_high()
    }
}

impl<const N: u8> ConfiguredPin<N, Output> {
    pub fn set_high(&mut self) {
        assert!(N < 32, "SYS_RIO GPIO output currently supports pins 0..31");
        reg(SYS_RIO_OUT).modify(|value| value | gpio_bit(N));
    }

    pub fn set_low(&mut self) {
        assert!(N < 32, "SYS_RIO GPIO output currently supports pins 0..31");
        reg(SYS_RIO_OUT).modify(|value| value & !gpio_bit(N));
    }

    pub fn toggle(&mut self) {
        assert!(N < 32, "SYS_RIO GPIO output currently supports pins 0..31");
        reg(SYS_RIO_OUT).modify(|value| value ^ gpio_bit(N));
    }
}

#[inline(always)]
fn reg(addr: usize) -> Reg<u32> {
    unsafe { Reg::new(addr) }
}

fn configure_input<const N: u8>(pull: u32) -> ConfiguredPin<N, Input> {
    assert!(N < 32, "SYS_RIO GPIO input currently supports pins 0..31");
    reg(PROC_MISC).write(PROC_MISC_RESET_CLEAR);
    reg(SYS_RIO_OE).modify(|value| value & !gpio_bit(N));
    reg(gpio_ctrl_addr(N)).write(FUNCSEL_SYS_RIO);
    reg(gpio_pad_addr(N))
        .modify(|value| (value & !PAD_PULL_MASK) | PAD_SCHMITT | PAD_IE | PAD_OD | pull);
    ConfiguredPin { _mode: PhantomData }
}

pub(crate) const fn gpio_ctrl_addr(n: u8) -> usize {
    IO_BANK0_BASE + 0x04 + (n as usize) * 8
}

pub(crate) const fn gpio_pad_addr(n: u8) -> usize {
    PADS_BANK0_BASE + 0x04 + (n as usize) * 4
}

pub(crate) const fn gpio_bit(n: u8) -> u32 {
    1u32 << n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpio_ctrl_addresses_match_golden_sequence() {
        assert_eq!(gpio_ctrl_addr(0), 0x400d_0004);
        assert_eq!(gpio_ctrl_addr(17), 0x400d_008c);
        assert_eq!(gpio_ctrl_addr(22), 0x400d_00b4);
    }

    #[test]
    fn gpio_pad_addresses_match_golden_sequence() {
        assert_eq!(gpio_pad_addr(0), 0x400f_0004);
        assert_eq!(gpio_pad_addr(17), 0x400f_0048);
        assert_eq!(gpio_pad_addr(22), 0x400f_005c);
    }

    #[test]
    fn gpio_bits_match_sys_rio_mask() {
        assert_eq!(gpio_bit(22), 0x0040_0000);
    }

    #[test]
    fn input_pad_contract_uses_pull_up_without_enabling_output() {
        let value = PAD_SCHMITT | PAD_IE | PAD_OD | PAD_PULL_UP;
        assert_eq!(value & PAD_PULL_MASK, PAD_PULL_UP);
        assert_ne!(value & PAD_IE, 0);
        assert_ne!(value & PAD_OD, 0);
        assert_eq!(SYS_RIO_IN, 0x400e_0008);
    }
}

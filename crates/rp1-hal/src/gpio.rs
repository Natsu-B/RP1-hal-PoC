use core::marker::PhantomData;
use core::convert::Infallible;
use embedded_hal::digital::{ErrorType, InputPin, OutputPin, StatefulOutputPin};

use crate::mmio::Reg;

mod interrupt;
pub use interrupt::{InterruptDestination, InterruptInput, InterruptKind, InterruptToken};

const PROC_MISC: usize = 0x4001_7004;
const IO_BANK0_BASE: usize = 0x400d_0000;
const PADS_BANK0_BASE: usize = 0x400f_0000;
const SYS_RIO_OUT: usize = 0x400e_0000;
const SYS_RIO_OE: usize = 0x400e_0004;
const SYS_RIO_IN: usize = 0x400e_0008;
// RP1 Peripherals §2.4/3.3: hardware atomic aliases, not software RMW.
const XOR: usize = 0x1000;
const SET: usize = 0x2000;
const CLEAR: usize = 0x3000;
// Bank 0 is GPIO0..27. GPIO28..53 use different banks/reset contracts;
// never issue a bank-0 transaction for them through this implementation.
const BANK0_MASK: u32 = (1 << 28) - 1;

const PROC_MISC_RESET_CLEAR: u32 = 1 << 19;
const FUNCSEL_SYS_RIO: u32 = 0x85;
const FUNCSEL_MASK: u32 = 0x1f;
const PAD_G33_OUTPUT: u32 = 0x56;
const PAD_SLEW_FAST: u32 = 1 << 0;
const PAD_SCHMITT: u32 = 1 << 1;
const PAD_PULL_MASK: u32 = 0b11 << 2;
const PAD_PULL_UP: u32 = 0b10 << 2;
const PAD_DRIVE_MASK: u32 = 0b11 << 4;
const PAD_DRIVE_12MA: u32 = 0b11 << 4;
const PAD_IE: u32 = 1 << 6;
const PAD_OD: u32 = 1 << 7;

pub struct Gpio {
    allowed: u32,
    available: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinError { UnsupportedPin, Unavailable, InUse, InvalidMask, InterruptConflict }

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
        Self { allowed: BANK0_MASK, available: BANK0_MASK }
    }

    /// Issue each bank-0 token once. Use `try_pin` for fallible configuration.
    #[inline(always)]
    pub fn pin<const N: u8>(&mut self) -> Pin<N> {
        match self.try_pin() {
            Ok(pin) => pin,
            // Keep failure admission, omit enum formatting on the tiny target.
            // Callers needing the reason use try_pin instead.
            Err(_) => panic!("GPIO unavailable"),
        }
    }

    /// No MMIO: denied, duplicate and unsupported requests fail before access.
    #[inline(always)]
    pub fn try_pin<const N: u8>(&mut self) -> Result<Pin<N>, PinError> {
        if N >= 28 { return Err(PinError::UnsupportedPin); }
        let bit = gpio_bit(N);
        if self.available & bit == 0 { return Err(PinError::Unavailable); }
        self.available &= !bit;
        Ok(Pin { _private: () })
    }

    /// Deployment restriction before issuing any token; cannot widen a mask.
    /// This does not configure Linux pinctrl or revoke an existing owner.
    pub fn restrict(&mut self, mask: u32) -> Result<(), PinError> {
        if mask & !self.allowed != 0 { return Err(PinError::InvalidMask); }
        if self.available != self.allowed { return Err(PinError::InUse); }
        self.allowed = mask; self.available = mask;
        Ok(())
    }

    /// Return the unique token for later reconfiguration. Hardware is unchanged.
    /// A configured pin must first be consumed with `into_pin()`.
    #[inline(always)]
    pub fn return_pin<const N: u8>(&mut self, _pin: Pin<N>) -> Result<(), PinError> {
        if N >= 28 { return Err(PinError::UnsupportedPin); }
        let bit = gpio_bit(N);
        if self.allowed & !self.available & bit == 0 { return Err(PinError::Unavailable); }
        self.available |= bit;
        Ok(())
    }
}

impl<const N: u8, Mode> ConfiguredPin<N, Mode> {
    /// Consume the old mode token without changing the pin. No second handle
    /// is minted; the caller can configure another mode or return it to Gpio.
    pub fn into_pin(self) -> Pin<N> { Pin { _private: () } }
}

impl<const N: u8> Pin<N> {
    pub fn into_input(self) -> ConfiguredPin<N, Input> {
        configure_input::<N>(0)
    }

    pub fn into_input_pull_up(self) -> ConfiguredPin<N, Input> {
        configure_input::<N>(PAD_PULL_UP)
    }

    // Reused by cold and warm setup; avoid duplicating the register sequence.
    #[inline(never)]
    pub fn into_output(self) -> ConfiguredPin<N, Output> {
        assert!(N < 28, "GPIO bank 0");
        reg(PROC_MISC).write(PROC_MISC_RESET_CLEAR);
        reg(gpio_ctrl_addr(N)).write(FUNCSEL_SYS_RIO);
        reg(gpio_pad_addr(N)).write(PAD_G33_OUTPUT);
        // Establish the initial low latch before enabling the output.
        reg(SYS_RIO_OUT + CLEAR).write(gpio_bit(N));
        reg(SYS_RIO_OE + SET).write(gpio_bit(N));
        ConfiguredPin { _mode: PhantomData }
    }

    pub fn into_function<const F: u8>(self) -> ConfiguredPin<N, Function<F>> {
        assert!(N < 28, "GPIO bank 0");
        assert!(F < 32, "RP1 function select is a five-bit field");
        reg(gpio_ctrl_addr(N)).modify(|value| (value & !FUNCSEL_MASK) | u32::from(F));
        reg(gpio_pad_addr(N)).modify(|value| (value | PAD_IE) & !PAD_OD);
        ConfiguredPin { _mode: PhantomData }
    }
}

impl<const N: u8> ConfiguredPin<N, Input> {
    pub fn is_high(&self) -> bool {
        assert!(N < 28, "GPIO bank 0");
        reg(SYS_RIO_IN).read() & gpio_bit(N) != 0
    }

    pub fn is_low(&self) -> bool {
        !self.is_high()
    }
}

impl<const N: u8> ConfiguredPin<N, Output> {
    #[inline(never)]
    pub fn set_high(&mut self) {
        assert!(N < 28, "GPIO bank 0");
        reg(SYS_RIO_OUT + SET).write(gpio_bit(N));
    }

    #[inline(never)]
    pub fn set_low(&mut self) {
        assert!(N < 28, "GPIO bank 0");
        reg(SYS_RIO_OUT + CLEAR).write(gpio_bit(N));
    }

    pub fn toggle(&mut self) {
        assert!(N < 28, "GPIO bank 0");
        reg(SYS_RIO_OUT + XOR).write(gpio_bit(N));
    }
}

impl<const N: u8, Mode> ErrorType for ConfiguredPin<N, Mode> { type Error = Infallible; }
impl<const N: u8> InputPin for ConfiguredPin<N, Input> {
    fn is_high(&mut self) -> Result<bool, Infallible> { Ok(ConfiguredPin::<N, Input>::is_high(self)) }
    fn is_low(&mut self) -> Result<bool, Infallible> { Ok(ConfiguredPin::<N, Input>::is_low(self)) }
}
impl<const N: u8> OutputPin for ConfiguredPin<N, Output> {
    fn set_high(&mut self) -> Result<(), Infallible> { ConfiguredPin::<N, Output>::set_high(self); Ok(()) }
    fn set_low(&mut self) -> Result<(), Infallible> { ConfiguredPin::<N, Output>::set_low(self); Ok(()) }
}
impl<const N: u8> StatefulOutputPin for ConfiguredPin<N, Output> {
    fn is_set_high(&mut self) -> Result<bool, Infallible> { Ok(reg(SYS_RIO_OUT).read() & gpio_bit(N) != 0) }
    fn is_set_low(&mut self) -> Result<bool, Infallible> { self.is_set_high().map(|v| !v) }
    fn toggle(&mut self) -> Result<(), Infallible> { ConfiguredPin::<N, Output>::toggle(self); Ok(()) }
}

#[inline(always)]
fn reg(addr: usize) -> Reg<u32> {
    unsafe { Reg::new(addr) }
}

fn configure_input<const N: u8>(pull: u32) -> ConfiguredPin<N, Input> {
    assert!(N < 28, "GPIO bank 0");
    reg(PROC_MISC).write(PROC_MISC_RESET_CLEAR);
    reg(SYS_RIO_OE + CLEAR).write(gpio_bit(N));
    reg(gpio_ctrl_addr(N)).write(FUNCSEL_SYS_RIO);
    reg(gpio_pad_addr(N))
        .modify(|value| (value & !PAD_PULL_MASK) | PAD_SCHMITT | PAD_IE | PAD_OD | pull);
    ConfiguredPin { _mode: PhantomData }
}

pub(crate) fn configure_i2c_pin<const N: u8>() {
    assert!(N < 28, "GPIO bank 0");
    reg(gpio_pad_addr(N)).modify(|value| {
        (value & !(PAD_SLEW_FAST | PAD_PULL_MASK | PAD_DRIVE_MASK | PAD_OD))
            | PAD_SCHMITT
            | PAD_PULL_UP
            | PAD_DRIVE_12MA
            | PAD_IE
    });
}

pub(crate) fn configure_spi_data_pin<const N: u8>() {
    assert!(N < 28, "GPIO bank 0");
    reg(gpio_pad_addr(N)).modify(|value| {
        (value & !(PAD_PULL_MASK | PAD_DRIVE_MASK | PAD_OD))
            | PAD_SLEW_FAST
            | PAD_SCHMITT
            | PAD_DRIVE_12MA
            | PAD_IE
    });
}

pub(crate) fn configure_spi_cs_pin<const N: u8>() {
    assert!(
        N < 28,
        "GPIO bank 0"
    );
    reg(gpio_pad_addr(N)).modify(|value| {
        (value & !(PAD_PULL_MASK | PAD_DRIVE_MASK | PAD_OD))
            | PAD_SLEW_FAST
            | PAD_SCHMITT
            | PAD_PULL_UP
            | PAD_DRIVE_12MA
            | PAD_IE
    });
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
    fn unique_tokens_restriction_and_explicit_return() {
        let mut gpio = unsafe { Gpio::new() };
        assert_eq!(gpio.restrict(1 << 28), Err(PinError::InvalidMask));
        gpio.restrict((1 << 22) | (1 << 27)).unwrap();
        assert!(matches!(gpio.try_pin::<0>(), Err(PinError::Unavailable)));
        assert!(matches!(gpio.try_pin::<28>(), Err(PinError::UnsupportedPin)));
        assert!(matches!(gpio.try_pin::<53>(), Err(PinError::UnsupportedPin)));
        assert!(matches!(gpio.try_pin::<255>(), Err(PinError::UnsupportedPin)));
        let pin = gpio.try_pin::<22>().unwrap();
        assert!(matches!(gpio.try_pin::<22>(), Err(PinError::Unavailable)));
        assert_eq!(gpio.restrict(1 << 22), Err(PinError::InUse));
        gpio.return_pin(pin).unwrap();
        gpio.restrict(1 << 22).unwrap();
        assert_eq!(gpio.restrict((1 << 22) | 1), Err(PinError::InvalidMask));
        assert!(matches!(gpio.try_pin::<27>(), Err(PinError::Unavailable)));
        let _pin = gpio.try_pin::<22>().unwrap();
        assert!(matches!(gpio.try_pin::<22>(), Err(PinError::Unavailable)));
    }

    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    #[test]
    fn actual_gpio_trait_methods_use_only_owned_bit_aliases() {
        use core::ffi::{c_int, c_void};
        unsafe extern "C" {
            fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
            fn munmap(addr: *mut c_void, len: usize) -> c_int;
        }
        struct Maps([usize;4]);
        impl Drop for Maps {
            fn drop(&mut self) { for a in self.0 { if a!=0 { unsafe { munmap(a as *mut c_void,0x4000); } } } }
        }
        let mut maps=Maps([0;4]);
        for (i,a) in [0x40014000usize,IO_BANK0_BASE,SYS_RIO_OUT,PADS_BANK0_BASE].into_iter().enumerate() {
            // Anonymous host RAM, not /dev/mem; never replace an existing map.
            assert_eq!(unsafe { mmap(a as *mut c_void,0x4000,3,0x100000|0x20|2,-1,0) } as usize,a);
            maps.0[i]=a;
        }
        let linux_bit=1;let owned=1<<22;
        reg(SYS_RIO_OUT).write(linux_bit|owned);reg(SYS_RIO_OE).write(linux_bit);
        let mut gpio=unsafe {Gpio::new()};gpio.restrict(owned).unwrap();
        let mut output=gpio.pin::<22>().into_output();
        assert_eq!(reg(SYS_RIO_OUT+CLEAR).read(),owned);
        assert_eq!(reg(SYS_RIO_OE+SET).read(),owned);
        OutputPin::set_high(&mut output).unwrap();
        assert_eq!(reg(SYS_RIO_OUT+SET).read(),owned);
        OutputPin::set_low(&mut output).unwrap();
        assert_eq!(reg(SYS_RIO_OUT+CLEAR).read(),owned);
        StatefulOutputPin::toggle(&mut output).unwrap();
        assert_eq!(reg(SYS_RIO_OUT+XOR).read(),owned);
        assert!(StatefulOutputPin::is_set_high(&mut output).unwrap());
        // Anonymous RAM records writes, but does NOT emulate hardware aliases.
        // These assertions detect any accidental whole-bank normal write.
        assert_eq!(reg(SYS_RIO_OUT).read(),linux_bit|owned);
        assert_eq!(reg(SYS_RIO_OE).read(),linux_bit);
        let mut input=output.into_pin().into_input_pull_up();
        assert!(matches!(gpio.try_pin::<22>(),Err(PinError::Unavailable)));
        assert_eq!(reg(SYS_RIO_OE+CLEAR).read(),owned);
        reg(SYS_RIO_IN).write(owned);assert!(InputPin::is_high(&mut input).unwrap());
        reg(SYS_RIO_IN).write(linux_bit);assert!(InputPin::is_low(&mut input).unwrap());
        gpio.return_pin(input.into_pin()).unwrap();
        let _pin=gpio.pin::<22>();
        let ctrl0=reg(gpio_ctrl_addr(0)).read();
        let pad0=reg(gpio_pad_addr(0)).read();
        let oe=reg(SYS_RIO_OE).read();
        interrupt::tests::mapped_register_checks();
        assert_eq!(reg(gpio_ctrl_addr(0)).read(),ctrl0);
        assert_eq!(reg(gpio_pad_addr(0)).read(),pad0);
        assert_eq!(reg(SYS_RIO_OE).read(),oe);
    }

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

    #[test]
    fn i2c_pad_contract_matches_rp1_linux_pin_configuration() {
        let initial = PAD_SLEW_FAST | PAD_OD;
        let value = (initial & !(PAD_SLEW_FAST | PAD_PULL_MASK | PAD_DRIVE_MASK | PAD_OD))
            | PAD_SCHMITT
            | PAD_PULL_UP
            | PAD_DRIVE_12MA
            | PAD_IE;

        assert_eq!(value & PAD_SLEW_FAST, 0);
        assert_eq!(value & PAD_PULL_MASK, PAD_PULL_UP);
        assert_eq!(value & PAD_DRIVE_MASK, PAD_DRIVE_12MA);
        assert_ne!(value & PAD_SCHMITT, 0);
        assert_ne!(value & PAD_IE, 0);
        assert_eq!(value & PAD_OD, 0);
    }

    #[test]
    fn spi_data_pad_contract_is_fast_push_pull_with_no_bias() {
        let initial = PAD_PULL_UP | PAD_OD;
        let value = (initial & !(PAD_PULL_MASK | PAD_DRIVE_MASK | PAD_OD))
            | PAD_SLEW_FAST
            | PAD_SCHMITT
            | PAD_DRIVE_12MA
            | PAD_IE;
        assert_ne!(value & PAD_SLEW_FAST, 0);
        assert_eq!(value & PAD_PULL_MASK, 0);
        assert_eq!(value & PAD_DRIVE_MASK, PAD_DRIVE_12MA);
        assert_ne!(value & PAD_IE, 0);
        assert_eq!(value & PAD_OD, 0);
    }

    #[test]
    fn spi_chip_select_pad_contract_keeps_idle_pull_up() {
        let initial = PAD_OD;
        let value = (initial & !(PAD_PULL_MASK | PAD_DRIVE_MASK | PAD_OD))
            | PAD_SLEW_FAST
            | PAD_SCHMITT
            | PAD_PULL_UP
            | PAD_DRIVE_12MA
            | PAD_IE;
        assert_eq!(value & PAD_PULL_MASK, PAD_PULL_UP);
        assert_eq!(value & PAD_OD, 0);
    }
}

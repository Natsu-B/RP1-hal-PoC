use core::sync::atomic::{AtomicBool, Ordering};

static PERIPHERALS_TAKEN: AtomicBool = AtomicBool::new(false);

pub struct Peripherals {
    pub gpio: crate::gpio::Gpio,
    pub i2c1: crate::i2c::I2c1,
    pub pwm0: crate::pwm::Pwm0,
    pub raw_timer: crate::timer::RawTimer,
    pub resets: crate::reset::ResetController,
    pub spi0: crate::spi::Spi0,
    pub uart0: crate::uart::Uart0,
    _private: (),
}

impl Peripherals {
    pub fn take() -> Option<Self> {
        if PERIPHERALS_TAKEN
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }
        Some(unsafe { Self::new() })
    }

    /// # Safety
    ///
    /// This bypasses singleton ownership. Callers must ensure no other
    /// `Peripherals` instance is alive.
    pub unsafe fn steal() -> Self {
        PERIPHERALS_TAKEN.store(true, Ordering::Release);
        unsafe { Self::new() }
    }

    unsafe fn new() -> Self {
        Self {
            gpio: unsafe { crate::gpio::Gpio::new() },
            i2c1: unsafe { crate::i2c::I2c1::new() },
            pwm0: unsafe { crate::pwm::Pwm0::new() },
            raw_timer: unsafe { crate::timer::RawTimer::new() },
            resets: unsafe { crate::reset::ResetController::new() },
            spi0: unsafe { crate::spi::Spi0::new() },
            uart0: unsafe { crate::uart::Uart0::new() },
            _private: (),
        }
    }
}

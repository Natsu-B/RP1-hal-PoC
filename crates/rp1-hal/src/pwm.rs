use crate::mmio::Reg;

const CLOCKS_MAIN_BASE: usize = 0x4001_8000;
const CLK_PWM0_CTRL: usize = CLOCKS_MAIN_BASE + 0x74;
const CLK_PWM0_DIV_INT: usize = CLOCKS_MAIN_BASE + 0x78;
const CLK_PWM0_DIV_FRAC: usize = CLOCKS_MAIN_BASE + 0x7c;
const CLK_PWM0_SEL: usize = CLOCKS_MAIN_BASE + 0x80;
const CLK_CTRL_ENABLE: u32 = 1 << 11;
const CLK_CTRL_AUXSRC_MASK: u32 = 0x0000_03e0;
const CLK_CTRL_XOSC: u32 = 2 << 5;
const CLK_SEL_AUX: u32 = 1;

const RESET_CTRL1: usize = 0x4001_4004;
const RESET_DONE1: usize = 0x4001_401c;
const RESET_CLEAR1: usize = 0x4001_7004;
const RESET_PWM0: u32 = 1 << 4;

const GPIO12_CTRL: usize = 0x400d_0064;
const GPIO12_PAD: usize = 0x400f_0034;
const GPIO_CTRL_FUNCSEL_MASK: u32 = 0x0000_001f;
const GPIO_CTRL_OVERRIDE_MASK: u32 = 0x0003_f000;
const GPIO_FUNCSEL_PWM0_CH0: u32 = 0;
const PAD_PULL_MASK: u32 = 0x0000_000c;
const PAD_INPUT_ENABLE: u32 = 1 << 6;
const PAD_OUTPUT_DISABLE: u32 = 1 << 7;

const PWM0_BASE: usize = 0x4009_8000;
const PWM_GLOBAL_CTRL: usize = PWM0_BASE;
const PWM_CH0_CTRL: usize = PWM0_BASE + 0x14;
const PWM_CH0_RANGE: usize = PWM0_BASE + 0x18;
const PWM_CH0_DUTY: usize = PWM0_BASE + 0x20;
const PWM_GLOBAL_SET_UPDATE: u32 = 1 << 31;
const PWM_GLOBAL_CH0_ENABLE: u32 = 1;
const PWM_CH0_TRAILING_EDGE_MARK_SPACE: u32 = (1 << 8) | 1;
const POLL_LIMIT: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pwm0Config {
    pub range: u32,
    pub duty: u32,
}

impl Pwm0Config {
    pub const fn new(range: u32, duty: u32) -> Self {
        Self { range, duty }
    }

    const fn valid(self) -> bool {
        self.range != 0 && self.duty <= self.range
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Pwm0Error {
    InvalidConfig = 1,
    ClockReadback = 2,
    ClockSelectTimeout = 3,
    ResetWriteRejected = 4,
    ResetDoneTimeout = 5,
    PinmuxReadback = 6,
    UpdateTimeout = 7,
    PwmReadback = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pwm0Snapshot {
    pub clock_ctrl: u32,
    pub clock_div_int: u32,
    pub clock_div_frac: u32,
    pub clock_sel: u32,
    pub reset_ctrl1: u32,
    pub reset_done1: u32,
    pub gpio12_ctrl: u32,
    pub gpio12_pad: u32,
    pub global_ctrl: u32,
    pub channel_ctrl: u32,
    pub range: u32,
    pub duty: u32,
}

pub struct Pwm0 {
    _private: (),
}

pub struct Pwm0Channel0 {
    _private: (),
}

impl Pwm0 {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    pub fn start_gpio12(&mut self, config: Pwm0Config) -> Result<Pwm0Channel0, Pwm0Error> {
        if !config.valid() {
            return Err(Pwm0Error::InvalidConfig);
        }

        configure_pwm0_clock_50mhz()?;
        release_pwm0_reset()?;

        reg(PWM_GLOBAL_CTRL).modify(|value| value & !PWM_GLOBAL_CH0_ENABLE);
        reg(PWM_CH0_CTRL).write(PWM_CH0_TRAILING_EDGE_MARK_SPACE);
        reg(PWM_CH0_DUTY).write(config.duty);
        reg(PWM_CH0_RANGE).write(config.range);
        configure_gpio12_pwm0()?;

        reg(PWM_GLOBAL_CTRL).modify(|value| value | PWM_GLOBAL_CH0_ENABLE | PWM_GLOBAL_SET_UPDATE);
        wait_for_update()?;

        let channel = Pwm0Channel0 { _private: () };
        let snapshot = channel.snapshot();
        if snapshot.channel_ctrl != PWM_CH0_TRAILING_EDGE_MARK_SPACE
            || snapshot.range != config.range
            || snapshot.duty != config.duty
            || snapshot.global_ctrl & PWM_GLOBAL_CH0_ENABLE == 0
        {
            return Err(Pwm0Error::PwmReadback);
        }
        Ok(channel)
    }

    pub fn snapshot(&self) -> Pwm0Snapshot {
        snapshot_pwm0()
    }
}

impl Pwm0Channel0 {
    pub fn apply(&mut self, config: Pwm0Config) -> Result<(), Pwm0Error> {
        if !config.valid() {
            return Err(Pwm0Error::InvalidConfig);
        }

        reg(PWM_CH0_DUTY).write(config.duty);
        reg(PWM_CH0_RANGE).write(config.range);
        reg(PWM_GLOBAL_CTRL).modify(|value| value | PWM_GLOBAL_SET_UPDATE);
        wait_for_update()?;

        if reg(PWM_CH0_RANGE).read() != config.range || reg(PWM_CH0_DUTY).read() != config.duty {
            return Err(Pwm0Error::PwmReadback);
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Pwm0Snapshot {
        snapshot_pwm0()
    }
}

fn snapshot_pwm0() -> Pwm0Snapshot {
    Pwm0Snapshot {
        clock_ctrl: reg(CLK_PWM0_CTRL).read(),
        clock_div_int: reg(CLK_PWM0_DIV_INT).read(),
        clock_div_frac: reg(CLK_PWM0_DIV_FRAC).read(),
        clock_sel: reg(CLK_PWM0_SEL).read(),
        reset_ctrl1: reg(RESET_CTRL1).read(),
        reset_done1: reg(RESET_DONE1).read(),
        gpio12_ctrl: reg(GPIO12_CTRL).read(),
        gpio12_pad: reg(GPIO12_PAD).read(),
        global_ctrl: reg(PWM_GLOBAL_CTRL).read(),
        channel_ctrl: reg(PWM_CH0_CTRL).read(),
        range: reg(PWM_CH0_RANGE).read(),
        duty: reg(PWM_CH0_DUTY).read(),
    }
}

fn configure_pwm0_clock_50mhz() -> Result<(), Pwm0Error> {
    let original = reg(CLK_PWM0_CTRL).read();
    let source = (original & !(CLK_CTRL_AUXSRC_MASK | CLK_CTRL_ENABLE)) | CLK_CTRL_XOSC;

    reg(CLK_PWM0_DIV_INT).write(1);
    reg(CLK_PWM0_DIV_FRAC).write(0);
    reg(CLK_PWM0_CTRL).write(source);
    dsb_sy();
    reg(CLK_PWM0_CTRL).write(source | CLK_CTRL_ENABLE);
    dsb_sy();

    if reg(CLK_PWM0_DIV_INT).read() != 1
        || reg(CLK_PWM0_DIV_FRAC).read() != 0
        || reg(CLK_PWM0_CTRL).read() & (CLK_CTRL_AUXSRC_MASK | CLK_CTRL_ENABLE)
            != CLK_CTRL_XOSC | CLK_CTRL_ENABLE
    {
        return Err(Pwm0Error::ClockReadback);
    }

    for _ in 0..POLL_LIMIT {
        if reg(CLK_PWM0_SEL).read() & CLK_SEL_AUX != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(Pwm0Error::ClockSelectTimeout)
}

fn release_pwm0_reset() -> Result<(), Pwm0Error> {
    if reg(RESET_CTRL1).read() & RESET_PWM0 != 0 {
        reg(RESET_CLEAR1).write(RESET_PWM0);
        dsb_sy();
        if reg(RESET_CTRL1).read() & RESET_PWM0 != 0 {
            return Err(Pwm0Error::ResetWriteRejected);
        }
    }

    for _ in 0..POLL_LIMIT {
        if reg(RESET_DONE1).read() & RESET_PWM0 != 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(Pwm0Error::ResetDoneTimeout)
}

fn configure_gpio12_pwm0() -> Result<(), Pwm0Error> {
    reg(GPIO12_CTRL).modify(gpio12_ctrl_value);
    reg(GPIO12_PAD).modify(gpio12_pad_value);
    dsb_sy();
    isb();

    if reg(GPIO12_CTRL).read() & (GPIO_CTRL_FUNCSEL_MASK | GPIO_CTRL_OVERRIDE_MASK)
        != GPIO_FUNCSEL_PWM0_CH0
        || reg(GPIO12_PAD).read() & (PAD_PULL_MASK | PAD_INPUT_ENABLE | PAD_OUTPUT_DISABLE)
            != PAD_INPUT_ENABLE
    {
        return Err(Pwm0Error::PinmuxReadback);
    }
    Ok(())
}

fn wait_for_update() -> Result<(), Pwm0Error> {
    dsb_sy();
    for _ in 0..POLL_LIMIT {
        if reg(PWM_GLOBAL_CTRL).read() & PWM_GLOBAL_SET_UPDATE == 0 {
            return Ok(());
        }
        core::hint::spin_loop();
    }
    Err(Pwm0Error::UpdateTimeout)
}

#[inline(always)]
fn gpio12_ctrl_value(current: u32) -> u32 {
    (current & !(GPIO_CTRL_FUNCSEL_MASK | GPIO_CTRL_OVERRIDE_MASK)) | GPIO_FUNCSEL_PWM0_CH0
}

#[inline(always)]
fn gpio12_pad_value(current: u32) -> u32 {
    (current & !(PAD_PULL_MASK | PAD_OUTPUT_DISABLE)) | PAD_INPUT_ENABLE
}

#[inline(always)]
fn reg(addr: usize) -> Reg<u32> {
    unsafe { Reg::new(addr) }
}

#[inline(always)]
fn dsb_sy() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[inline(always)]
fn isb() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("isb", options(nostack, preserves_flags));
    }

    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pwm0_register_contract_matches_official_map() {
        assert_eq!(CLK_PWM0_CTRL, 0x4001_8074);
        assert_eq!(CLK_PWM0_DIV_INT, 0x4001_8078);
        assert_eq!(CLK_PWM0_DIV_FRAC, 0x4001_807c);
        assert_eq!(CLK_PWM0_SEL, 0x4001_8080);
        assert_eq!(CLK_SEL_AUX, 1);
        assert_eq!(RESET_CLEAR1, 0x4001_7004);
        assert_eq!(RESET_PWM0, 1 << 4);
        assert_eq!(PWM_GLOBAL_CTRL, 0x4009_8000);
        assert_eq!(PWM_CH0_CTRL, 0x4009_8014);
        assert_eq!(PWM_CH0_RANGE, 0x4009_8018);
        assert_eq!(PWM_CH0_DUTY, 0x4009_8020);
    }

    #[test]
    fn gpio12_pwm0_pinmux_preserves_unrelated_fields() {
        let original = 0xa5a4_0fff | GPIO_CTRL_OVERRIDE_MASK;
        let ctrl = gpio12_ctrl_value(original);
        assert_eq!(ctrl & GPIO_CTRL_FUNCSEL_MASK, GPIO_FUNCSEL_PWM0_CH0);
        assert_eq!(ctrl & GPIO_CTRL_OVERRIDE_MASK, 0);
        assert_eq!(
            ctrl & !(GPIO_CTRL_FUNCSEL_MASK | GPIO_CTRL_OVERRIDE_MASK),
            original & !(GPIO_CTRL_FUNCSEL_MASK | GPIO_CTRL_OVERRIDE_MASK)
        );

        let pad = gpio12_pad_value(0xffff_ffff);
        assert_eq!(pad & PAD_PULL_MASK, 0);
        assert_eq!(pad & PAD_OUTPUT_DISABLE, 0);
        assert_eq!(pad & PAD_INPUT_ENABLE, PAD_INPUT_ENABLE);
    }

    #[test]
    fn proof_configs_are_valid_for_50mhz_clock() {
        let low = Pwm0Config::new(5_000_000, 1_250_000);
        let high = Pwm0Config::new(50_000, 37_500);
        assert!(low.valid());
        assert!(high.valid());
        assert_eq!(50_000_000 / low.range, 10);
        assert_eq!(low.duty * 100 / low.range, 25);
        assert_eq!(50_000_000 / high.range, 1_000);
        assert_eq!(high.duty * 100 / high.range, 75);
    }
}

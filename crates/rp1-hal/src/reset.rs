use crate::mmio::Reg;

const RESET_CTRL: [usize; 2] = [0x4001_4000, 0x4001_4004];
const RESET_SET: [usize; 2] = [0x4001_6000, 0x4001_6004];
const RESET_CLEAR: [usize; 2] = [0x4001_7000, 0x4001_7004];
const RESET_DONE: [usize; 2] = [0x4001_4018, 0x4001_401c];

const CLK_UART_CTRL: usize = 0x4001_8054;
const CLK_UART_DIV_INT: usize = 0x4001_8058;
const CLK_UART_SEL: usize = 0x4001_8060;
const PLL_SYS_CS: usize = 0x4002_0000;
const PLL_SYS_PRIM: usize = 0x4002_0010;
const CLK_UART_RELEVANT: u32 = 0x0000_0fe0;
const CLK_UART_XOSC_ENABLED: u32 = 0x0000_0840;
const PLL_SYS_LOCKED: u32 = 0x8000_0001;
const PLL_SYS_PRI_PH_ENABLED: u32 = 1 << 4;

pub struct ResetController {
    _private: (),
}

/// Targets whose current hardware evidence authorizes release, but not assert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeassertOnlyReset {
    PllSys,
    Pwm0,
    I2c1,
    Spi0,
}

/// UART reset targets with controlled assert and deassert hardware evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UartReset {
    Uart0,
    Uart1,
    Uart2,
    Uart3,
    Uart4,
    Uart5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResetState {
    pub asserted: bool,
    pub done: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetError {
    ClockNotReady,
    PreconditionMismatch(ResetState),
    WriteRejected(ResetState),
    Timeout(ResetState),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResetSpec {
    bank: usize,
    bit: u8,
}

const PROC1: ResetSpec = ResetSpec { bank: 0, bit: 31 };

impl ResetController {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    /// Releases a proven release-only target after the caller has established
    /// the target-specific clock and ownership preconditions.
    pub fn deassert_preconditions_met(
        &mut self,
        target: DeassertOnlyReset,
        poll_limit: usize,
    ) -> Result<ResetState, ResetError> {
        transition(target.spec(), false, poll_limit)
    }

    /// Releases proc1 after a valid Boot ROM entry/stack/start tuple is ready.
    ///
    /// # Safety
    ///
    /// The caller must ensure the proc1 start tuple and shared memory visible
    /// to proc1 are valid before releasing reset.
    pub unsafe fn deassert_proc1_boot_tuple_ready(
        &mut self,
        poll_limit: usize,
    ) -> Result<ResetState, ResetError> {
        transition(PROC1, false, poll_limit)
    }

    /// Returns true only for the exact XOSC UART clock envelope already used
    /// by the UART reset-DONE and external UART proofs.
    pub fn uart_clocks_ready(&self) -> bool {
        reg(CLK_UART_CTRL).read() & CLK_UART_RELEVANT == CLK_UART_XOSC_ENABLED
            && reg(CLK_UART_DIV_INT).read() == 1
            && reg(CLK_UART_SEL).read() == 1
            && reg(PLL_SYS_CS).read() == PLL_SYS_LOCKED
            && reg(PLL_SYS_PRIM).read() & PLL_SYS_PRI_PH_ENABLED != 0
    }

    /// Asserts an owned, quiesced UART while the proven clock envelope holds.
    pub fn assert_uart_clock_ready(
        &mut self,
        target: UartReset,
        poll_limit: usize,
    ) -> Result<ResetState, ResetError> {
        if !self.uart_clocks_ready() {
            return Err(ResetError::ClockNotReady);
        }
        transition(target.spec(), true, poll_limit)
    }

    /// Releases an owned UART while the proven clock envelope holds.
    pub fn deassert_uart_clock_ready(
        &mut self,
        target: UartReset,
        poll_limit: usize,
    ) -> Result<ResetState, ResetError> {
        if !self.uart_clocks_ready() {
            return Err(ResetError::ClockNotReady);
        }
        transition(target.spec(), false, poll_limit)
    }
}

impl DeassertOnlyReset {
    const fn spec(self) -> ResetSpec {
        match self {
            Self::PllSys => ResetSpec { bank: 0, bit: 29 },
            Self::Pwm0 => ResetSpec { bank: 1, bit: 4 },
            Self::I2c1 => ResetSpec { bank: 0, bit: 8 },
            Self::Spi0 => ResetSpec { bank: 1, bit: 10 },
        }
    }
}

impl UartReset {
    const fn spec(self) -> ResetSpec {
        let bit = match self {
            Self::Uart0 => 26,
            Self::Uart1 => 27,
            Self::Uart2 => 28,
            Self::Uart3 => 29,
            Self::Uart4 => 30,
            Self::Uart5 => 31,
        };
        ResetSpec { bank: 1, bit }
    }
}

fn transition(
    spec: ResetSpec,
    asserted: bool,
    poll_limit: usize,
) -> Result<ResetState, ResetError> {
    let before = state(spec);
    let expected_before = ResetState {
        asserted: !asserted,
        done: asserted,
    };
    if before != expected_before {
        return Err(ResetError::PreconditionMismatch(before));
    }

    let mask = 1 << spec.bit;
    reg(if asserted {
        RESET_SET[spec.bank]
    } else {
        RESET_CLEAR[spec.bank]
    })
    .write(mask);
    dsb_sy();

    let observed = state(spec);
    if observed.asserted != asserted {
        return Err(ResetError::WriteRejected(observed));
    }
    if observed.done == !asserted {
        return Ok(observed);
    }

    for _ in 0..poll_limit {
        let observed = state(spec);
        if observed.done == !asserted {
            return Ok(observed);
        }
        core::hint::spin_loop();
    }
    Err(ResetError::Timeout(state(spec)))
}

fn state(spec: ResetSpec) -> ResetState {
    let mask = 1 << spec.bit;
    ResetState {
        asserted: reg(RESET_CTRL[spec.bank]).read() & mask != 0,
        done: reg(RESET_DONE[spec.bank]).read() & mask != 0,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_allowlist_matches_hardware_evidence() {
        let release_only = [
            (DeassertOnlyReset::PllSys, 0, 29),
            (DeassertOnlyReset::Pwm0, 1, 4),
            (DeassertOnlyReset::I2c1, 0, 8),
            (DeassertOnlyReset::Spi0, 1, 10),
        ];
        for (target, bank, bit) in release_only {
            assert_eq!(target.spec(), ResetSpec { bank, bit });
        }

        let uarts = [
            UartReset::Uart0,
            UartReset::Uart1,
            UartReset::Uart2,
            UartReset::Uart3,
            UartReset::Uart4,
            UartReset::Uart5,
        ];
        for (index, target) in uarts.into_iter().enumerate() {
            assert_eq!(
                target.spec(),
                ResetSpec {
                    bank: 1,
                    bit: 26 + index as u8,
                }
            );
        }

        assert_eq!(PROC1, ResetSpec { bank: 0, bit: 31 });
        assert_eq!(RESET_CTRL, [0x4001_4000, 0x4001_4004]);
        assert_eq!(RESET_SET, [0x4001_6000, 0x4001_6004]);
        assert_eq!(RESET_CLEAR, [0x4001_7000, 0x4001_7004]);
        assert_eq!(RESET_DONE, [0x4001_4018, 0x4001_401c]);
    }
}

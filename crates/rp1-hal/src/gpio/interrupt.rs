//! Bank-0 GPIO event routing; NVIC/vector setup remains the application owner's job.
use super::*;

const EVENT_MASK: u32 = 0xff << 20;
const IRQRESET: u32 = 1 << 28;
const IRQOVER: u32 = 3 << 30;
const PROC0_INTE: usize = IO_BANK0_BASE + 0x104;
const PROC1_INTE: usize = IO_BANK0_BASE + 0x110;
const PCIE_INTE: usize = IO_BANK0_BASE + 0x11c;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptDestination { Proc0, Proc1 }

impl InterruptDestination {
    #[inline(always)]
    fn enable_addr(self) -> usize {
        match self { Self::Proc0 => PROC0_INTE, Self::Proc1 => PROC1_INTE }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptKind {
    EdgeLow, EdgeHigh, EdgeBoth, LevelLow, LevelHigh,
    FilteredEdgeLow, FilteredEdgeHigh, FilteredEdgeBoth,
    DebouncedLevelLow, DebouncedLevelHigh,
}

impl InterruptKind {
    #[inline(always)]
    fn mask(self) -> u32 {
        match self {
            Self::EdgeLow => 1 << 20, Self::EdgeHigh => 1 << 21,
            Self::EdgeBoth => 3 << 20, Self::LevelLow => 1 << 22,
            Self::LevelHigh => 1 << 23, Self::FilteredEdgeLow => 1 << 24,
            Self::FilteredEdgeHigh => 1 << 25, Self::FilteredEdgeBoth => 3 << 24,
            Self::DebouncedLevelLow => 1 << 26, Self::DebouncedLevelHigh => 1 << 27,
        }
    }
}

/// Unique deployment grant, distinct from the pin's digital-I/O token.
pub struct InterruptToken<const N: u8> { destination: InterruptDestination }

impl<const N: u8> InterruptToken<N> {
    /// No MMIO. Generated resource partitioning issues only assigned grants.
    ///
    /// # Safety
    /// The caller must own this pin's event configuration and the selected
    /// processor route for the complete token lifetime, issue it exactly once,
    /// and exclude concurrent Linux/other-core configuration of this same pin.
    /// Install a bounded handler and configure the target NVIC before routing.
    pub const unsafe fn new(destination: InterruptDestination) -> Self {
        assert!(N < 28, "GPIO bank 0");
        Self { destination }
    }
}

/// Owns the input pin and its interrupt grant until masked or dropped.
/// Masking does not cancel an already-pending NVIC interrupt. Keep the handler
/// installed until the application has drained its processor's bank IRQ.
pub struct InterruptInput<const N: u8> {
    destination: InterruptDestination,
    enabled: bool,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<const N: u8> ConfiguredPin<N, Input> {
    /// Reject an existing enable or forced event in *any* destination before
    /// issuing writes. Static deployment ownership is still required: this
    /// readback is not an interprocessor lock or a hardware security boundary.
    #[inline(always)]
    pub fn into_interrupt(self, grant: InterruptToken<N>, kind: InterruptKind)
        -> Result<InterruptInput<N>, (PinError, Self, InterruptToken<N>)>
    {
        let bit = gpio_bit(N);
        for enable in [PROC0_INTE, PROC1_INTE, PCIE_INTE] {
            if (reg(enable).read() | reg(enable + 4).read()) & bit != 0 {
                return Err((PinError::InterruptConflict, self, grant));
            }
        }
        let ctrl = gpio_ctrl_addr(N);
        // RP1 uses per-pin eight-event masks, not RP2040's four-bit INTR banks.
        reg(ctrl + CLEAR).write(EVENT_MASK | IRQOVER);
        reg(ctrl + SET).write(IRQRESET);
        irq_barrier();
        reg(ctrl + SET).write(kind.mask());
        reg(grant.destination.enable_addr() + SET).write(bit);
        irq_barrier();
        Ok(InterruptInput { destination: grant.destination, enabled: true, _not_send_sync: PhantomData })
    }
}

impl<const N: u8> InterruptInput<N> {
    #[inline(always)]
    pub fn is_interrupt_set(&self) -> bool {
        reg(self.destination.enable_addr() + 8).read() & gpio_bit(N) != 0
    }

    /// Raw event bits: falling, rising, low, high, then filtered/debounced forms.
    /// Level events are live conditions; edge latches can coalesce multiple
    /// edges. These flags cannot establish a loss-free acquisition count.
    #[inline(always)]
    pub fn event_status(&self) -> u8 {
        (reg(gpio_ctrl_addr(N) - 4).read() >> 20) as u8
    }

    /// Resets all four edge latches for this pin; does not clear an active level.
    #[inline(always)]
    pub fn clear_interrupt(&mut self) {
        reg(gpio_ctrl_addr(N) + SET).write(IRQRESET);
        irq_barrier();
    }

    #[inline(always)]
    pub fn is_high(&self) -> bool { reg(SYS_RIO_IN).read() & gpio_bit(N) != 0 }
    pub fn is_low(&self) -> bool { !self.is_high() }

    #[inline(always)]
    fn mask(&mut self) {
        if self.enabled {
            reg(self.destination.enable_addr() + CLEAR).write(gpio_bit(N));
            irq_barrier();
            reg(gpio_ctrl_addr(N) + CLEAR).write(EVENT_MASK);
            reg(gpio_ctrl_addr(N) + SET).write(IRQRESET);
            irq_barrier();
            self.enabled = false;
        }
    }

    #[inline(always)]
    pub fn into_input(mut self) -> (ConfiguredPin<N, Input>, InterruptToken<N>) {
        self.mask();
        (ConfiguredPin { _mode: PhantomData }, InterruptToken { destination: self.destination })
    }
}

impl<const N: u8> Drop for InterruptInput<N> {
    fn drop(&mut self) { self.mask(); }
}

impl<const N: u8> ErrorType for InterruptInput<N> { type Error = Infallible; }
impl<const N: u8> InputPin for InterruptInput<N> {
    fn is_high(&mut self) -> Result<bool, Infallible> { Ok(InterruptInput::is_high(self)) }
    fn is_low(&mut self) -> Result<bool, Infallible> { Ok(InterruptInput::is_low(self)) }
}

#[inline(always)]
fn irq_barrier() {
    #[cfg(target_arch = "arm")]
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[test]
    fn rp1_routes_and_event_masks_match_reference() {
        assert_eq!(InterruptDestination::Proc0.enable_addr(), 0x400d_0104);
        assert_eq!(InterruptDestination::Proc1.enable_addr(), 0x400d_0110);
        let events = [InterruptKind::EdgeLow, InterruptKind::EdgeHigh,
            InterruptKind::LevelLow, InterruptKind::LevelHigh,
            InterruptKind::FilteredEdgeLow, InterruptKind::FilteredEdgeHigh,
            InterruptKind::DebouncedLevelLow, InterruptKind::DebouncedLevelHigh];
        for (i, event) in events.into_iter().enumerate() {
            assert_eq!(event.mask(), 1 << (20 + i));
        }
        assert_eq!(InterruptKind::EdgeBoth.mask(), 3 << 20);
        assert_eq!(InterruptKind::FilteredEdgeBoth.mask(), 3 << 24);
    }

    #[test]
    #[should_panic(expected = "GPIO bank 0")]
    fn unsupported_grant_fails_without_mmio() {
        let _ = unsafe { InterruptToken::<28>::new(InterruptDestination::Proc1) };
    }

    // Called inside the existing anonymous-map test, avoiding parallel mmap
    // collisions. RAM records addresses/values, not hardware alias semantics.
    pub(in crate::gpio) fn mapped_register_checks() {
        let bit = 1 << 9;
        let other = 1 << 0;
        let ctrl = FUNCSEL_SYS_RIO | (7 << 5);
        reg(gpio_ctrl_addr(9)).write(ctrl);
        for destination in [InterruptDestination::Proc0, InterruptDestination::Proc1] {
            for conflict in [PROC0_INTE, PROC0_INTE + 4, PROC1_INTE,
                PROC1_INTE + 4, PCIE_INTE, PCIE_INTE + 4]
            {
                for addr in [PROC0_INTE, PROC0_INTE + 4, PROC1_INTE,
                    PROC1_INTE + 4, PCIE_INTE, PCIE_INTE + 4] { reg(addr).write(other); }
                reg(conflict).write(other | bit);
                for addr in [gpio_ctrl_addr(9) + SET, gpio_ctrl_addr(9) + CLEAR,
                    PROC0_INTE + SET, PROC1_INTE + SET,
                    PROC0_INTE + CLEAR, PROC1_INTE + CLEAR] { reg(addr).write(0x5a5a_1234); }
                let pin = ConfiguredPin::<9, Input> { _mode: PhantomData };
                let grant = unsafe { InterruptToken::new(destination) };
                let Err((error, pin, grant)) = pin.into_interrupt(grant, InterruptKind::EdgeLow)
                    else { panic!("existing route accepted") };
                assert_eq!(error, PinError::InterruptConflict);
                for addr in [gpio_ctrl_addr(9) + SET, gpio_ctrl_addr(9) + CLEAR,
                    PROC0_INTE + SET, PROC1_INTE + SET,
                    PROC0_INTE + CLEAR, PROC1_INTE + CLEAR] { assert_eq!(reg(addr).read(), 0x5a5a_1234); }
                reg(conflict).write(other);
                let Ok(mut input) = pin.into_interrupt(grant, InterruptKind::EdgeLow)
                    else { panic!("unrouted owned input rejected") };
                assert_eq!(reg(gpio_ctrl_addr(9) + CLEAR).read(), EVENT_MASK | IRQOVER);
                assert_eq!(reg(gpio_ctrl_addr(9) + SET).read(), 1 << 20);
                assert_eq!(reg(destination.enable_addr() + SET).read(), bit);
                assert_eq!(reg(gpio_ctrl_addr(9)).read(), ctrl);
                // Other destinations and other pins' control/pad/OE are untouched.
                for addr in [PROC0_INTE, PROC1_INTE, PCIE_INTE] { assert_eq!(reg(addr).read(), other); }
                reg(destination.enable_addr() + 8).write(other);
                assert!(!input.is_interrupt_set());
                reg(destination.enable_addr() + 8).write(other | bit);
                assert!(input.is_interrupt_set());
                reg(gpio_ctrl_addr(9) - 4).write((0x91 << 20) | (3 << 28));
                assert_eq!(input.event_status(), 0x91);
                reg(SYS_RIO_IN).write(bit);
                assert!(InputPin::is_high(&mut input).unwrap());
                input.clear_interrupt();
                assert_eq!(reg(gpio_ctrl_addr(9) + SET).read(), IRQRESET);
                let (pin, grant) = input.into_input();
                assert_eq!(reg(destination.enable_addr() + CLEAR).read(), bit);
                assert_eq!(reg(gpio_ctrl_addr(9) + CLEAR).read(), EVENT_MASK);
                assert_eq!(reg(gpio_ctrl_addr(9)).read(), ctrl);
                let Ok(input) = pin.into_interrupt(grant, InterruptKind::LevelLow)
                    else { panic!("returned grant rejected") };
                assert_eq!(reg(gpio_ctrl_addr(9) + SET).read(), 1 << 22);
                drop(input);
                assert_eq!(reg(destination.enable_addr() + CLEAR).read(), bit);
                assert_eq!(reg(gpio_ctrl_addr(9) + CLEAR).read(), EVENT_MASK);
            }
        }
    }
}

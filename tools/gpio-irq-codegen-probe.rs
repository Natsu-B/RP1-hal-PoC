//! Compile-only probe. It installs no vectors, starts no acquisition and is
//! never deployed. The granted profile must install its handler before bind.
#![no_std]

use rp1_hal::gpio::{ConfiguredPin, Input, InterruptInput, InterruptKind, InterruptToken, PinError};

mod resources { include!(env!("AMP_GPIO_IRQ_RESOURCES")); }

#[unsafe(no_mangle)]
pub fn gpio9_irq_bind(all: rp1_hal::Peripherals)
    -> Result<InterruptInput<9>, (PinError, ConfiguredPin<9, Input>, InterruptToken<9>)>
{
    let owned = resources::Resources::from_peripherals(all);
    owned.gpio9.into_input_pull_up().into_interrupt(owned.gpio9_irq, InterruptKind::EdgeLow)
}

#[unsafe(no_mangle)]
pub fn gpio9_irq_ack(input: &mut InterruptInput<9>) -> (bool, u8) {
    let pending = input.is_interrupt_set();
    let events = input.event_status();
    if pending { input.clear_interrupt(); }
    (pending, events)
}

#[unsafe(no_mangle)]
pub fn gpio9_irq_release(input: InterruptInput<9>) -> (ConfiguredPin<9, Input>, InterruptToken<9>) {
    input.into_input()
}

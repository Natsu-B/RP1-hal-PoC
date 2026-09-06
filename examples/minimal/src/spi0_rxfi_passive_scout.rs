//! A bounded RXFI source observation, never an IRQ-handler or payload proof.
use rp1_hal::spi::{Spi0Host, Spi0IrqSnapshot};
use rp1_rt::Spi0IrqRouteSnapshot;

const MAGIC: u32 = u32::from_le_bytes(*b"S0R1");
const RXFI: u32 = 0x10;
const ERRORS: u32 = 0x0e;
const RX_MASK: u32 = RXFI | ERRORS;
const INITIAL_PENDING1: u32 = 1 << 21;
const OBSERVE_US: u64 = 4_000;

struct Spi {
    irq: Spi0IrqSnapshot,
    rx: u32,
    control: u32,
    selected: u32,
    baud: u32,
    rx_threshold: u32,
}

fn snapshot() -> Spi {
    // Fixed fields already used by spi.rs/receive.rs, not an MMIO scan.
    let read = |offset| unsafe {
        core::ptr::read_volatile((rp1_hal::addr::SPI0_BASE + offset) as *const u32)
    };
    Spi {
        irq: rp1_hal::spi::spi0_irq_snapshot(),
        rx: read(0x24),
        control: read(0x00),
        selected: read(0x10),
        baud: read(0x14),
        rx_threshold: read(0x1c),
    }
}

fn configured(s: &Spi) -> bool {
    s.control == 7 << 16 && s.baud == 2_000 && s.rx_threshold == 0
}

fn clean(s: &Spi) -> bool {
    s.irq.enable == 0
        && s.selected == 0
        && s.irq.interrupt_mask == 0
        && s.irq.masked_interrupt_status == 0
        && s.irq.raw_interrupt_status & ERRORS == 0
        && s.rx == 0
        && s.irq.tx_fifo_level == 0
}

fn route_quiet(r: Spi0IrqRouteSnapshot) -> bool {
    r.iser0 == 0 && r.iser1 == 0 && r.iabr0 == 0 && r.iabr1 == 0
}

pub fn publish_setup_error(code: u32) {
    let mut words = [0; 16];
    words[1] = code;
    publish(words);
}

pub fn run(host: &mut Spi0Host) -> u32 {
    let before = rp1_rt::spi0_irq_route_snapshot();
    if !route_quiet(before) || before.ispr0 != 0 || before.ispr1 & !INITIAL_PENDING1 != 0 {
        publish_setup_error(0x321);
        return 0x321;
    }
    let mut rx = [0; 1];
    let mut transfer = match host.prepare_irq_transfer(&[0xa5], &mut rx) {
        Ok(t) => t,
        Err(_) => {
            publish_setup_error(0x322);
            return 0x322;
        }
    };
    let prepared = snapshot();
    let prepared_ok = configured(&prepared)
        && prepared.irq.enable == 1
        && prepared.selected == 0
        && prepared.irq.interrupt_mask == 0
        && prepared.irq.masked_interrupt_status == 0
        && prepared.irq.raw_interrupt_status & (ERRORS | RXFI) == 0
        && prepared.rx == 0
        && prepared.irq.tx_fifo_level == 1;
    if !prepared_ok {
        let code = if transfer.abort().is_ok() {
            0x323
        } else {
            0x325
        };
        publish_setup_error(code);
        return code;
    }
    if transfer.start().is_err() {
        let code = if transfer.abort().is_ok() {
            0x326
        } else {
            0x325
        };
        publish_setup_error(code);
        return code;
    }
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)) };
    super::busy_wait_us(OBSERVE_US);
    let observed = snapshot();
    let during = rp1_rt::spi0_irq_route_snapshot();
    // Deliberately do not call on_interrupt or read DR: no payload claim.
    let abort_ok = transfer.abort().is_ok();
    drop(transfer);
    let final_spi = snapshot();
    let after = rp1_rt::spi0_irq_route_snapshot();
    let source_ok = configured(&observed)
        && observed.irq.enable == 1
        && observed.selected == 1
        && observed.irq.interrupt_mask == RX_MASK
        && observed.irq.raw_interrupt_status == RXFI | 1
        && observed.irq.masked_interrupt_status == RXFI
        && observed.rx == 1
        && observed.irq.tx_fifo_level == 0
        && observed.irq.status & 5 == 4;
    let routes_ok = route_quiet(during) && route_quiet(after);
    let context_ok = before.vtor == during.vtor
        && before.vtor == after.vtor
        && before.primask == during.primask
        && before.primask == after.primask;
    let flags = (prepared_ok as u32)
        | ((source_ok as u32) << 1)
        | ((abort_ok as u32) << 2)
        | ((clean(&final_spi) as u32) << 3)
        | ((routes_ok as u32) << 4)
        | ((context_ok as u32) << 5);
    let decision = if flags == 0x3f { 1 } else { 0x324 };
    let words = [
        0,
        decision,
        before.ispr0,
        before.ispr1,
        during.ispr0,
        during.ispr1,
        after.ispr0,
        after.ispr1,
        before.iser0 | before.iser1 | during.iser0 | during.iser1 | after.iser0 | after.iser1,
        before.iabr0 | before.iabr1 | during.iabr0 | during.iabr1 | after.iabr0 | after.iabr1,
        observed.irq.interrupt_mask,
        observed.irq.raw_interrupt_status,
        observed.irq.masked_interrupt_status,
        observed.rx,
        final_spi.irq.interrupt_mask
            | final_spi.irq.masked_interrupt_status
            | final_spi.rx
            | final_spi.irq.enable
            | final_spi.selected
            | final_spi.irq.tx_fifo_level
            | (final_spi.irq.raw_interrupt_status & ERRORS),
        flags,
    ];
    publish(words);
    decision
}

fn publish(words: [u32; 16]) {
    const _: () = assert!(16 * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

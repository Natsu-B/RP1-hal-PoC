//! A bounded RXFI source observation, never an IRQ-handler or payload proof.
use rp1_hal::spi::{Spi0Host, Spi0IrqSnapshot, Spi0IrqTransfer};
use rp1_rt::Spi0IrqRouteSnapshot;

#[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
const MAGIC: u32 = u32::from_le_bytes(*b"S0R1");
#[cfg(all(
    feature = "spi0-rxfi-wrapper-readonly",
    not(feature = "spi0-rxfi-wrapper-or-once")
))]
const MAGIC: u32 = u32::from_le_bytes(*b"S0W1");
#[cfg(feature = "spi0-rxfi-wrapper-or-once")]
const MAGIC: u32 = u32::from_le_bytes(*b"S0O1");
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

fn prepared_state(s: &Spi) -> bool {
    configured(s)
        && s.irq.enable == 1
        && s.selected == 0
        && s.irq.interrupt_mask == 0
        && s.irq.masked_interrupt_status == 0
        && s.irq.raw_interrupt_status & (ERRORS | RXFI) == 0
        && s.rx == 0
        && s.irq.tx_fifo_level == 1
}

fn source_state(s: &Spi) -> bool {
    configured(s)
        && s.irq.enable == 1
        && s.selected == 1
        && s.irq.interrupt_mask == RX_MASK
        && s.irq.raw_interrupt_status == RXFI | 1
        && s.irq.masked_interrupt_status == RXFI
        && s.rx == 1
        && s.irq.tx_fifo_level == 0
        && s.irq.status & 5 == 4
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

#[cfg(feature = "spi0-rxfi-wrapper-or-once")]
fn write_route_ok(before: Spi0IrqRouteSnapshot, now: Spi0IrqRouteSnapshot) -> bool {
    route_quiet(now)
        && now.ispr0 == 0
        && now.ispr1 == INITIAL_PENDING1
        && now.vtor == before.vtor
        && now.primask == before.primask
}

#[cfg(feature = "spi0-rxfi-wrapper-readonly")]
fn read_wrapper() -> u32 {
    // Only the stock-consumer-aligned read, with no inferred bit semantics.
    unsafe { core::ptr::read_volatile((rp1_hal::addr::SPI0_BASE + 0x108) as *const u32) }
}

#[cfg(feature = "spi0-rxfi-wrapper-readonly")]
fn wait_interval() -> bool {
    const LOW: *const u32 = 0x400a_c028 as *const u32;
    let start = unsafe { core::ptr::read_volatile(LOW) };
    // A frozen timer must reach abort. An MMIO bus stall is outside this bound.
    for _ in 0..1_000_000 {
        if unsafe { core::ptr::read_volatile(LOW) }.wrapping_sub(start) >= OBSERVE_US as u32 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

pub fn publish_setup_error(code: u32) {
    #[cfg(not(feature = "spi0-rxfi-wrapper-or-once"))]
    let mut words = [0; 16];
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    let mut words = if code == 0x320 { [0; 16] } else { progress() };
    words[1] = code;
    publish(words);
}

#[cfg(feature = "spi0-rxfi-wrapper-or-once")]
fn progress() -> [u32; 16] {
    // Owned private SRAM ABI, not saved MMIO and not a wrapper restore image.
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    let mut words = [0; 16];
    unsafe {
        if core::ptr::read_volatile(out) != MAGIC {
            return words;
        }
        for (i, word) in words.iter_mut().enumerate().skip(1) {
            *word = core::ptr::read_volatile(out.add(i));
        }
    }
    if words[15] >> 8 > 3 { [0; 16] } else { words }
}

fn abort_and_publish_error(transfer: &mut Spi0IrqTransfer<'_>, code: u32) -> u32 {
    let code = if transfer.abort().is_ok() {
        code
    } else {
        0x325
    };
    publish_setup_error(code);
    code
}

pub fn run(host: &mut Spi0Host) -> u32 {
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    let mut progress_words = {
        let mut words = [0; 16];
        words[1] = 0x360; // In progress, never PASS; stage 0 means no attempt.
        words[10..15].fill(u32::MAX); // Unavailable samples, not observed zeroes.
        publish(words);
        words
    };
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
    let prepared_ok = prepared_state(&prepared);
    if !prepared_ok {
        return abort_and_publish_error(&mut transfer, 0x323);
    }
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    let before = {
        // Fresh two-bank/context evidence after preparation, before any write.
        let fresh = rp1_rt::spi0_irq_route_snapshot();
        progress_words[2] = fresh.ispr0;
        progress_words[3] = fresh.ispr1;
        progress_words[8] = fresh.iser0 | fresh.iser1;
        progress_words[9] = fresh.iabr0 | fresh.iabr1;
        publish(progress_words);
        if !write_route_ok(before, fresh) {
            return abort_and_publish_error(&mut transfer, 0x361);
        }
        fresh
    };
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    let wrapper_prepared = {
        let value = read_wrapper();
        #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
        {
            progress_words[10] = value;
            publish(progress_words);
            if value != 0 {
                return abort_and_publish_error(&mut transfer, 0x362);
            }
        }
        if !prepared_state(&snapshot()) {
            return abort_and_publish_error(&mut transfer, 0x350);
        }
        value
    };
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    let wrapper_post_write = {
        if !write_route_ok(before, rp1_rt::spi0_irq_route_snapshot()) {
            return abort_and_publish_error(&mut transfer, 0x361);
        }
        // Exact primary-known u32 OR1, once only. No IRQ-enable API or undo.
        // Stage 1: about to issue; a fault can leave store completion uncertain.
        progress_words[15] = 1 << 8;
        publish(progress_words);
        unsafe {
            core::ptr::write_volatile(
                (rp1_hal::addr::SPI0_BASE + 0x108) as *mut u32,
                wrapper_prepared | 1,
            );
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
        progress_words[15] = 2 << 8; // Store issued; readback not verified.
        publish(progress_words);
        let value = read_wrapper(); // Exactly one immediate readback.
        progress_words[11] = value;
        publish(progress_words); // Preserve even a rejected readback value.
        if value != 1 {
            return abort_and_publish_error(&mut transfer, 0x363);
        }
        progress_words[15] = 3 << 8; // Readback 1 verified.
        publish(progress_words);
        if !prepared_state(&snapshot())
            || !write_route_ok(before, rp1_rt::spi0_irq_route_snapshot())
        {
            return abort_and_publish_error(&mut transfer, 0x364);
        }
        value
    };
    if transfer.start().is_err() {
        return abort_and_publish_error(&mut transfer, 0x326);
    }
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)) };
    #[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
    super::busy_wait_us(OBSERVE_US);
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    if !wait_interval() {
        return abort_and_publish_error(&mut transfer, 0x351);
    }
    let observed = snapshot();
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    {
        progress_words[13] = observed.rx;
        publish(progress_words);
    }
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    let _wrapper_active = {
        if !source_state(&observed) {
            return abort_and_publish_error(&mut transfer, 0x352);
        }
        let value = read_wrapper();
        #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
        if value != 1 {
            return abort_and_publish_error(&mut transfer, 0x365);
        }
        if !source_state(&snapshot()) {
            return abort_and_publish_error(&mut transfer, 0x353);
        }
        value
    };
    let during = rp1_rt::spi0_irq_route_snapshot();
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    {
        progress_words[4] = during.ispr0;
        progress_words[5] = during.ispr1;
        progress_words[8] |= during.iser0 | during.iser1;
        progress_words[9] |= during.iabr0 | during.iabr1;
        publish(progress_words);
    }
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    if !route_quiet(during) || before.vtor != during.vtor || before.primask != during.primask {
        return abort_and_publish_error(&mut transfer, 0x355);
    }
    // Deliberately do not call on_interrupt or read DR: no payload claim.
    let abort_ok = transfer.abort().is_ok();
    #[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
    drop(transfer);
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    if !abort_ok {
        publish_setup_error(0x325);
        return 0x325;
    }
    let final_spi = snapshot();
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    let wrapper_final = {
        if !clean(&final_spi) {
            return abort_and_publish_error(&mut transfer, 0x354);
        }
        let value = read_wrapper();
        #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
        {
            progress_words[12] = value;
            publish(progress_words);
            if value != 1 {
                return abort_and_publish_error(&mut transfer, 0x366);
            }
        }
        value
    };
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    let final_spi = snapshot();
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    if !clean(&final_spi) {
        return abort_and_publish_error(&mut transfer, 0x354);
    }
    let after = rp1_rt::spi0_irq_route_snapshot();
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    {
        progress_words[6] = after.ispr0;
        progress_words[7] = after.ispr1;
        progress_words[8] |= after.iser0 | after.iser1;
        progress_words[9] |= after.iabr0 | after.iabr1;
        progress_words[14] = final_spi.irq.interrupt_mask
            | final_spi.irq.masked_interrupt_status
            | final_spi.rx
            | final_spi.irq.enable
            | final_spi.selected
            | final_spi.irq.tx_fifo_level
            | (final_spi.irq.raw_interrupt_status & ERRORS);
        publish(progress_words);
    }
    let source_ok = source_state(&observed);
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
    #[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
    let decision = if flags == 0x3f { 1 } else { 0x324 };
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    if flags != 0x3f {
        return abort_and_publish_error(&mut transfer, 0x355);
    }
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    drop(transfer);
    // Reaching here proves every guarded recheck and the bounded wait completed.
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    let flags = flags | (1 << 6) | (1 << 7);
    #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
    let flags = flags | (3 << 8);
    #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
    let decision = 1;
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
        #[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
        observed.irq.interrupt_mask,
        #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
        wrapper_prepared,
        #[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
        observed.irq.raw_interrupt_status,
        #[cfg(all(
            feature = "spi0-rxfi-wrapper-readonly",
            not(feature = "spi0-rxfi-wrapper-or-once")
        ))]
        _wrapper_active,
        #[cfg(feature = "spi0-rxfi-wrapper-or-once")]
        wrapper_post_write,
        #[cfg(not(feature = "spi0-rxfi-wrapper-readonly"))]
        observed.irq.masked_interrupt_status,
        #[cfg(feature = "spi0-rxfi-wrapper-readonly")]
        wrapper_final,
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

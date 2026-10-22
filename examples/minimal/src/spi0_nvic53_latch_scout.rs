//! Pending bookkeeping only: never enable IRQ53 or acknowledge its RP1 source.
use rp1_rt::Spi0IrqRouteSnapshot as Route;

const BIT: u32 = 1 << 21;
const ICPR1: *mut u32 = 0xe000_e284 as *mut u32;
const ISPR1: *mut u32 = 0xe000_e204 as *mut u32;
const ICTR: *const u32 = 0xe000_e004 as *const u32;
const SAMPLES: u32 = 64;

fn wait_interval() -> bool {
    // Existing raw-timer LOW, sufficient for a short wrap-safe interval.
    // A frozen counter must not strand pending-state restoration.
    const LOW: *const u32 = 0x400a_c028 as *const u32;
    let start = unsafe { core::ptr::read_volatile(LOW) };
    for _ in 0..100_000 {
        if unsafe { core::ptr::read_volatile(LOW) }.wrapping_sub(start) >= 64 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn quiet(r: Route) -> u32 {
    r.iser0 | r.iser1 | r.iabr0 | r.iabr1
}

fn same_context(a: Route, b: Route) -> bool {
    a.vtor == b.vtor && a.primask == b.primask
}

fn spi_stopped() -> bool {
    let s = rp1_hal::spi::spi0_irq_snapshot();
    let read = |offset| unsafe {
        core::ptr::read_volatile((rp1_hal::addr::SPI0_BASE + offset) as *const u32)
    };
    s.enable == 0
        && s.interrupt_mask == 0
        && s.masked_interrupt_status == 0
        && s.raw_interrupt_status & 0x1e == 0
        && s.status & 1 == 0
        && s.tx_fifo_level == 0
        && read(0x24) == 0
        && read(0x10) == 0
}

pub fn run() -> u32 {
    let before = rp1_rt::spi0_irq_route_snapshot();
    let ictr = unsafe { core::ptr::read_volatile(ICTR) };
    if ictr & 15 != 1
        || quiet(before) != 0
        || before.ispr0 != 0
        || before.ispr1 != BIT
        || !spi_stopped()
    {
        // Failed setup is not a latch experiment. Preserve fixed raw SPI fields.
        let s = rp1_hal::spi::spi0_irq_snapshot();
        let read = |offset| unsafe {
            core::ptr::read_volatile((rp1_hal::addr::SPI0_BASE + offset) as *const u32)
        };
        publish([
            0,
            0x331,
            before.ispr0,
            before.ispr1,
            s.enable,
            s.interrupt_mask,
            s.masked_interrupt_status,
            s.raw_interrupt_status,
            s.tx_fifo_level,
            read(0x24),
            quiet(before),
            read(0x10),
            s.status,
            0,
            0,
            ictr,
        ]);
        return 0x331;
    }
    // One-hot architectural clear only. No early exit below this point.
    unsafe {
        core::ptr::write_volatile(ICPR1, BIT);
        core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
    }
    let immediate = rp1_rt::spi0_irq_route_snapshot();
    let mut pending0 = immediate.ispr0;
    let mut pending1 = immediate.ispr1;
    let mut quiet_or = quiet(before) | quiet(immediate);
    let mut context_ok = same_context(before, immediate);
    let mut zero_samples = (immediate.ispr1 & BIT == 0) as u32;
    let mut last = immediate;
    let mut timing_ok = true;
    let mut sample_count = 1;
    for _ in 0..SAMPLES {
        if !wait_interval() {
            timing_ok = false;
            break; // Continue through the restoration epilogue.
        }
        last = rp1_rt::spi0_irq_route_snapshot();
        sample_count += 1;
        pending0 |= last.ispr0;
        pending1 |= last.ispr1;
        quiet_or |= quiet(last);
        context_ok &= same_context(before, last);
        zero_samples += (last.ispr1 & BIT == 0) as u32;
    }
    let stopped = spi_stopped();
    // Software restoration is recorded separately, never counted as delivery.
    let restore_written = unsafe { core::ptr::read_volatile(ISPR1) } & BIT == 0;
    if restore_written {
        unsafe {
            core::ptr::write_volatile(ISPR1, BIT);
            core::arch::asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
        }
    }
    let restored = rp1_rt::spi0_irq_route_snapshot();
    quiet_or |= quiet(restored);
    context_ok &= same_context(before, restored);
    let flags = (stopped as u32)
        | ((spi_stopped() as u32) << 1)
        | (((quiet_or == 0) as u32) << 2)
        | ((context_ok as u32) << 3)
        | (((restored.ispr0 == before.ispr0 && restored.ispr1 == before.ispr1) as u32) << 4)
        | (((pending0 == 0 && pending1 & !BIT == 0) as u32) << 5)
        | ((timing_ok as u32) << 6);
    let decision = if flags == 0x7f { 1 } else { 0x332 };
    publish([
        0,
        decision,
        before.ispr0,
        before.ispr1,
        immediate.ispr0,
        immediate.ispr1,
        last.ispr0,
        last.ispr1,
        restored.ispr0,
        restored.ispr1,
        quiet_or,
        pending0,
        pending1,
        zero_samples,
        flags,
        (sample_count << 16) | restore_written as u32,
    ]);
    decision
}

pub fn publish_setup_error(code: u32) {
    let mut words = [0; 16];
    words[1] = code;
    publish(words);
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
        core::ptr::write_volatile(out, u32::from_le_bytes(*b"S0L1"));
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

//! Distinct SPI lifecycle workload: current peer is input-only, MISO biased high.
//! No external sequence/payload claim; normal external frames have their own test.
use super::*;
use os::spi0::{Driver, Error, Receipt};

fn reg(offset: usize) -> u32 {
    unsafe { ((rp1_hal::addr::SPI0_BASE + offset) as *const u32).read_volatile() }
}

/// Existing lower-priority monitor also cancels one request. Keep its original
/// absolute wake deadline: notification must not accelerate GPIO logging.
pub unsafe fn monitor_wait(ticks: u32) {
    let deadline = unsafe { os::tick().unwrap() }.wrapping_add(ticks);
    while let Some(left) = os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline) {
        if unsafe { os::notification_take(true, left).unwrap() } == 0 { continue; }
        let phase = get(173);
        #[cfg(not(feature = "freertos-r2-spi-cancel-window"))]
        assert_eq!(phase, 1);
        #[cfg(feature = "freertos-r2-spi-cancel-window")]
        assert!(phase == 1 || phase == 2);
        put(173, 0);
        let generation = unsafe { os::spi0::active_generation() };
        if phase == 1 {
            put(133, generation); put(134, reg(0x10)); put(135, reg(0x28));
            assert!(generation > 1 && get(134) == 1 && get(135) & 1 == 1);
        } else {
            assert_eq!(generation, 3);
            assert_eq!(reg(0x10), 0); assert_eq!(reg(0x28) & 1, 0);
            put(181, generation);
        }
        assert!(!unsafe { os::spi0::cancel(0) });
        assert!(!unsafe { os::spi0::cancel(generation - 1) });
        put(160, get(160) | 6);
        // This call wakes/preempts to owner5. Return may occur after teardown.
        assert!(unsafe { os::spi0::cancel(generation) });
        put(160, get(160) | 8); put(172, phase);
    }
}

/// Test image seam only. The official scheduler runs the lower-priority monitor
/// while this owner sleeps, with an already-completed SPI transfer still published.
#[cfg(feature = "freertos-r2-spi-cancel-window")]
#[unsafe(no_mangle)]
unsafe extern "C" fn rp1_spi_cancel_window_probe(generation: u32) {
    assert_eq!(generation, 3); assert_eq!(unsafe { os::spi0::active_generation() }, 3);
    put(174, generation); put(175, reg(0x10)); put(176, reg(0x2c));
    put(177, unsafe { (0xe000_e100 as *const u32).read_volatile() } & (1 << 19));
    put(178, reg(0x28));
    assert_eq!(get(175) | get(176) | get(177) | (get(178) & 1), 0);
    let started = raw_low(); put(179, unsafe { os::tick().unwrap() });
    put(173, 2);
    unsafe { task(0).notification_give().unwrap(); os::delay(2).unwrap(); }
    put(180, raw_low().wrapping_sub(started));
    assert_eq!(get(172), 2); assert_eq!(get(181), generation);
    assert_eq!(unsafe { os::spi0::active_generation() }, generation);
    assert!(get(180) > 0 && get(180) < 10_000);
    put(182, 1);
}

fn record(index: usize, r: Receipt, rx: &Buffer) {
    let before = unsafe { ptr::addr_of!(rx.before).read_volatile() };
    let after = unsafe { ptr::addr_of!(rx.after).read_volatile() };
    assert_eq!(before, 0x5aa5_a55a); assert_eq!(after, 0xa55a_5aa5);
    for (i, value) in [u32::from_be_bytes(rx.bytes), r.generation, r.irq_entries,
        r.elapsed_us, r.irq_body_max_us, r.irq_end_to_task_us, before, after]
        .into_iter().enumerate() { put(index+i, value); }
}

unsafe fn quiet(notification_index: usize) {
    assert_eq!(unsafe { os::spi0::active_generation() }, 0);
    let drained = unsafe { os::notification_take(true, 0).unwrap() };
    put(notification_index, drained); assert_eq!(drained, 0);
    for (i, address) in [(164, 0xe000_e100usize), (165, 0xe000_e200)] {
        let value = unsafe { (address as *const u32).read_volatile() } & (1 << 19);
        put(i, value); assert_eq!(value, 0);
    }
    for (i, offset) in [(166,0x2c), (167,0x10), (168,0x08), (169,0x24), (170,0x20)] {
        let value = reg(offset); put(i, value); assert_eq!(value, 0);
    }
    put(171, reg(0x108)); assert_eq!(get(171), 1);
}

pub unsafe fn run(driver: &mut Driver) -> ! {
    #[cfg(not(feature = "freertos-r2-spi-cancel-window"))]
    put(128, u32::from_le_bytes(*b"RL01"));
    #[cfg(feature = "freertos-r2-spi-cancel-window")]
    put(128, u32::from_le_bytes(*b"SCW1"));
    put(129, 1);
    unsafe { wait_level(true, 3000); os::delay(10).unwrap(); }
    let mut rx = Buffer { before:0x5aa5_a55a, bytes:[0xc3;4], after:0xa55a_5aa5 };
    for timeout in [0, 0x8000_0000] {
        assert!(matches!(unsafe { driver.receive(&[0;4], &mut rx.bytes, timeout) }, Err(Error::InvalidDeadline)));
        assert!(driver.last_receipt().is_none()); increment(130);
    }
    assert!(matches!(unsafe { driver.receive(&[0;3], &mut rx.bytes, 50) },
        Err(Error::Receive(rp1_hal::spi::Spi0RxError::LengthMismatch { .. }))));
    assert!(driver.last_receipt().is_none()); increment(130);
    assert_eq!(rx.bytes, [0xc3;4]);
    assert!(!unsafe { os::spi0::cancel(1) }); put(160, 1);

    // Align after a tick. Proven four-byte serial transfer exceeds1ms, so this
    // deadline should expire with an actual partially received IRQ prefix.
    unsafe { os::delay(1).unwrap(); }
    let result = unsafe { driver.receive(&[0;4], &mut rx.bytes, 1) };
    if let Some(r) = driver.last_receipt() { record(136, r, &rx); }
    assert!(matches!(result, Err(Error::Timeout)));
    assert!(get(138) > 0 && get(138) < 4);
    assert!(rx.bytes[0] == 0xff && rx.bytes[3] == 0xc3);
    unsafe { quiet(161); } increment(131); put(129, 2);

    rx.bytes = [0xc3;4];
    put(173, 1);
    // Owner5 continues until receive blocks; only then can monitor4 cancel it.
    unsafe { task(0).notification_give().unwrap(); }
    let result = unsafe { driver.receive(&[0;4], &mut rx.bytes, 50) };
    if let Some(r) = driver.last_receipt() { record(144, r, &rx); }
    assert!(matches!(result, Err(Error::Cancelled)));
    assert_eq!(get(133), get(145));
    unsafe { quiet(162); os::delay(2).unwrap(); }
    assert_eq!(get(172), 1);
    let saved = unsafe { ptr::addr_of!(rx.bytes).read_volatile() };
    unsafe { os::delay(2).unwrap(); }
    assert_eq!(unsafe { ptr::addr_of!(rx.bytes).read_volatile() }, saved);
    assert!(!unsafe { os::spi0::cancel(get(145)) }); put(160, get(160) | 16);
    increment(131); put(129, 3);

    rx.bytes = [0xc3;4];
    let result = unsafe { driver.receive(&[0;4], &mut rx.bytes, 50) };
    #[cfg(not(feature = "freertos-r2-spi-cancel-window"))]
    let r = result.unwrap();
    #[cfg(feature = "freertos-r2-spi-cancel-window")]
    let r = {
        assert!(matches!(result, Err(Error::Cancelled)));
        assert_eq!(get(182), 1);
        driver.last_receipt().unwrap()
    };
    record(152, r, &rx);
    assert_eq!(rx.bytes, [0xff;4]); assert_eq!(r.generation, 3);
    unsafe { quiet(163); } increment(131);
    #[cfg(feature = "freertos-r2-spi-cancel-window")]
    {
        let saved = unsafe { ptr::addr_of!(rx.bytes).read_volatile() };
        unsafe { os::delay(2).unwrap(); }
        assert_eq!(unsafe { ptr::addr_of!(rx.bytes).read_volatile() }, saved);
        assert!(!unsafe { os::spi0::cancel(3) }); put(183, 1);
        rx.bytes = [0xc3;4];
        let r = unsafe { driver.receive(&[0;4], &mut rx.bytes, 50) }.unwrap();
        record(184, r, &rx); assert_eq!(r.generation, 4); assert_eq!(rx.bytes, [0xff;4]);
        unsafe { quiet(163); } increment(131);
    }
    put(129, 4);
    loop { unsafe { os::delay(1000).unwrap(); } increment(132); }
}

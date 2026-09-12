//! Deliberate software Reset-entry diagnostic, NOT watchdog expiry/restart.
//! Proc0/peripheral-free only. Consume the cookie before BSS clear; stop before
//! PCIe/application init because warm .data/peripheral contracts are still OPEN.
use super::{get, put, raw_low, reset_identity as model, watchdog_quiescence};
use core::arch::asm;
use rp1_hal::gpio::{ConfiguredPin, Output};

const RECORD: *mut u32 = 0x2000_fa20 as *mut u32; // R1 words136..139, not panic/fault.
unsafe fn record() -> [u32; 4] {
    unsafe {
        [
            RECORD.read_volatile(),
            RECORD.add(1).read_volatile(),
            RECORD.add(2).read_volatile(),
            RECORD.add(3).read_volatile(),
        ]
    }
}
unsafe fn publish(words: [u32; 4]) {
    unsafe {
        RECORD.write_volatile(0);
        RECORD.add(1).write_volatile(words[1]);
        RECORD.add(2).write_volatile(words[2]);
        RECORD.add(3).write_volatile(words[3]);
        asm!("dsb sy", options(nostack));
        RECORD.write_volatile(words[0]);
        asm!("dsb sy", options(nostack));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rp1_freertos_capture_reset_entry() -> u32 {
    let prior = unsafe { record() };
    if model::valid_arm(prior) {
        let reason = unsafe { (0x4015_4008 as *const u32).read_volatile() };
        if let Some(entry) = model::entry_from_arm(prior, reason) {
            unsafe {
                publish(entry);
            }
            return 1;
        }
    }
    // No ARM magic means cold/invalid/stale entry, never positive reentry.
    unsafe {
        RECORD.write_volatile(0);
        asm!("dsb sy", options(nostack));
    }
    0
}

fn hold(us: u32) -> bool {
    let start = raw_low();
    for _ in 0..10_000_000 {
        if raw_low().wrapping_sub(start) >= us {
            return true;
        }
        core::hint::spin_loop();
    }
    false // Timer stalls are a missing packet, never positive restart.
}
fn halt() -> ! {
    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rp1_freertos_reset_entry_halt() -> ! {
    // Reset disabled IRQs, replaced MSP and cleared BSS. No pre-reset Rust
    // resource/task will execute again; take no lock or exclusive retry loop.
    let mut p = unsafe { rp1_hal::Peripherals::steal() };
    let mut marker = p.gpio.pin::<22>().into_output();
    marker.set_low();
    let entry = unsafe { record() };
    if !model::valid_entry(entry) {
        halt();
    }
    let Some(word) = model::encode_entry(entry[1], entry[2]) else {
        halt();
    };
    if !hold(500_000) {
        halt();
    }
    for bit in (0..32).rev() {
        marker.set_high();
        let ok = hold(if word & (1 << bit) != 0 {
            150_000
        } else {
            50_000
        });
        marker.set_low();
        if !ok || !hold(50_000) {
            halt();
        }
    }
    marker.set_high();
    let _ = hold(400_000);
    marker.set_low();
    halt(); // No PCIe reinitialization, scheduler restart or repeat packet.
}

/// Existing GPIO monitor owns the deliberate branch after the disabled ACK.
pub unsafe fn reenter_pending(marker: &mut ConfiguredPin<22, Output>) -> bool {
    if get(98) != 6 {
        return false;
    }
    let ack = core::array::from_fn(|i| get(176 + i));
    assert!(watchdog_quiescence::valid_ack(ack));
    assert_eq!(ack[6], get(139));
    assert_eq!((get(99), get(145), get(146)), (0, 1, 1));
    let ctrl = unsafe { (0x4015_4000 as *const u32).read_volatile() };
    assert_eq!(ctrl & 0xff00_0000, 0); // WDT stays disabled; no new WDT write.
    let cookie = model::arm_words(ack[6]).expect("validated16-bit nonce");
    put(98, 7);
    marker.set_low();
    unsafe {
        publish(cookie);
        // Never return into abandoned PSP/TCB state. This is a software branch,
        // not AIRCR reset, watchdog reset, or a hardware reset-domain test.
        asm!("cpsid i", "b Reset", options(noreturn));
    }
}

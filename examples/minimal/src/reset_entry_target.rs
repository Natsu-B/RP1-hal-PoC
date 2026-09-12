//! Early Reset-entry reporter for separate software/WDT7 expiry experiments.
//! Proc0/peripheral-free only. Consume the cookie before BSS clear; stop before
//! PCIe/application init because warm .data/peripheral contracts are still OPEN.
use super::{get, put, raw_low, reset_identity as model, watchdog_quiescence};
use core::arch::asm;
use rp1_hal::gpio::{ConfiguredPin, Output};

const RECORD: *mut u32 = 0x2000_fa20 as *mut u32; // R1 words136..139, not panic/fault.
pub(super) unsafe fn record() -> [u32; 4] {
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
        #[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
        unsafe {
            // Only our valid ARM cookie authorizes this already-proven disable.
            // Do not let an unknown CTRL state enter warm runtime initialization.
            let ctrl = 0x4015_4000 as *mut u32;
            if !super::boot_entry::known_ctrl(ctrl.read_volatile()) { halt(); }
            ctrl.write_volatile(0);
            asm!("dsb sy", options(nostack));
            if ctrl.read_volatile() & 0xff00_0000 != 0 { halt(); }
        }
        if let Some(entry) = model::entry_from_arm(prior, reason) {
            unsafe {
                publish(entry);
            }
            return 1;
        }
    }
    #[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
    if matches!(prior[0],model::ARM_MAGIC | model::ENTRY_MAGIC) {
        halt(); // Corrupt/consumed retained record must not start a warm kernel.
    }
    // Absent/unrecognized cookie cannot distinguish a cold boot from lost SRAM.
    // It is never positive reentry evidence.
    unsafe {
        RECORD.write_volatile(0);
        asm!("dsb sy", options(nostack));
    }
    0
}

pub(super) fn hold(us: u32) -> bool {
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

/// Terminal WDT9 guard diagnostic, including before BSS clear. Inherited GPIO22
/// setup only: no HAL singleton, .data/.bss, RTOS, reset or clock writer.
#[cfg(feature = "freertos-r3-watchdog-warm-guard")]
pub(super) unsafe fn diagnostic_halt(code: u32) -> ! {
    unsafe { asm!("cpsid i", options(nomem, nostack)); }
    let entry = unsafe { record() };
    if !model::valid_entry(entry) { halt(); }
    let Some(word) = super::kernel_restart::diagnostic_packet(entry[1], code) else { halt(); };
    unsafe fn level(high: bool) {
        let out = 0x400e_0000 as *mut u32; // Same known SYS_RIO_OUT RMW as HAL GPIO.
        unsafe { out.write_volatile((out.read_volatile() & !(1<<22)) | if high {1<<22} else {0}); }
    }
    unsafe { level(false); }
    if !hold(500_000) { halt(); }
    for bit in (0..32).rev() {
        unsafe { level(true); }
        let ok = hold(if word & (1<<bit) != 0 {150_000} else {50_000});
        unsafe { level(false); }
        if !ok || !hold(50_000) { halt(); }
    }
    unsafe { level(true); }
    let _ = hold(400_000);
    unsafe { level(false); }
    halt() // E identifies only a guard stop, never advances to a kernel.
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
#[cfg(feature = "freertos-r3-reset-entry-selftest")]
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

#[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
unsafe fn boot_tuple() -> [u32;3] {
    unsafe { [(0x4015_400c as *const u32).read_volatile(),
        (0x4015_4010 as *const u32).read_volatile(),
        (0x4015_4018 as *const u32).read_volatile()] }
}

/// Only the consumed proc0 encoded-entry word is republished. No reset trigger.
#[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
pub(super) unsafe fn arm_boot(nonce: u32) -> bool {
    let before=unsafe { boot_tuple() };
    let sp=unsafe { (0x2000_0000 as *const u32).read_volatile() };
    let entry=unsafe { (0x2000_0004 as *const u32).read_volatile() };
    let Some(desired)=super::boot_entry::planned_tuple(before,sp,entry) else { return false; };
    if unsafe { record() } != [0,0,0,nonce] { return false; }
    let Some(cookie)=model::arm_words(nonce) else { return false; };
    unsafe {
        publish(cookie);
        (0x4015_4010 as *mut u32).write_volatile(desired[1]);
        asm!("dsb sy", options(nostack));
        super::boot_entry::owned_tuple(boot_tuple(),desired)
    }
}

/// Alive/error path must reclaim its own future-start token and cookie.
#[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
pub(super) unsafe fn cancel_boot() -> bool {
    let prior=unsafe { record() };
    if !model::valid_arm(prior) { return prior[0]==0; }
    if prior[1]!=get(182) { return false; } // Own the final ACK's nonce only.
    let before=[super::boot_entry::BOOT_MAGIC,0,super::boot_entry::VECTOR_SP];
    let sp=unsafe { (0x2000_0000 as *const u32).read_volatile() };
    let entry=unsafe { (0x2000_0004 as *const u32).read_volatile() };
    let Some(desired)=super::boot_entry::planned_tuple(before,sp,entry) else { return false; };
    let current=unsafe { boot_tuple() };
    // Failed publication may leave the exact original tuple; no write needed.
    // A foreign/partial tuple is not ours to clear, even with our cookie present.
    if !super::boot_entry::restored_tuple(current,before) &&
       !super::boot_entry::owned_tuple(current,desired) { return false; }
    unsafe {
        if current != before {
            (0x4015_4010 as *mut u32).write_volatile(0);
            asm!("dsb sy", options(nostack));
        }
        if !super::boot_entry::restored_tuple(boot_tuple(),before) { return false; }
        RECORD.write_volatile(0);
        asm!("dsb sy", options(nostack));
    }
    true
}

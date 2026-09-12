//! WDT2 bounded receipt; WDL1 adds one cold enabled-LOAD refresh witness.
//! No expiry, feed policy, reset or POWER write in either bounded probe.
//! Normal R1 only; words96..175 are firmware-owned,176..183 host-owned.
#[cfg(all(feature = "freertos-r3-watchdog-refresh", any(
    feature = "freertos-r3-watchdog-quiescence", feature = "freertos-r3-watchdog-postack",
    feature = "freertos-r3-watchdog-late-disable", feature = "freertos-r3-reset-entry-selftest",
    feature = "freertos-r3-watchdog-expiry-entry", feature = "freertos-r3-watchdog-kernel-restart",
    feature = "freertos-r3-watchdog-warm-guard", feature = "freertos-r3-watchdog-kernel-restart-masked",
    feature = "freertos-r3-watchdog-warm-uart", feature = "freertos-r3-watchdog-warm-spi",
    feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined",
    feature = "freertos-r3-watchdog-warm-persistent",
)))]
compile_error!("WDL1 is cold one-shot only; no ACK, post-ACK arm, reset or warm features");

pub const NONCE_LINKED: bool = cfg!(any(feature = "freertos-r3-watchdog-refresh", feature = "freertos-r3-reset-entry-selftest", feature = "freertos-r3-watchdog-expiry-entry"));
pub const MAGIC: u32 = if cfg!(feature = "freertos-r3-watchdog-refresh") { u32::from_le_bytes(*b"WDL1") }
else if cfg!(feature = "freertos-r3-watchdog-warm-guard") { u32::from_le_bytes(*b"WDT9") }
else if cfg!(feature = "freertos-r3-watchdog-kernel-restart") { u32::from_le_bytes(*b"WDT8") }
else if cfg!(feature = "freertos-r3-watchdog-expiry-entry") {
    u32::from_le_bytes(*b"WDT7")
} else if cfg!(feature = "freertos-r3-reset-entry-selftest") {
    u32::from_le_bytes(*b"WDT6")
} else if cfg!(feature = "freertos-r3-watchdog-late-disable") {
    u32::from_le_bytes(*b"WDT5")
} else if cfg!(feature = "freertos-r3-watchdog-postack") {
    u32::from_le_bytes(*b"WDT4")
} else if cfg!(feature = "freertos-r3-watchdog-quiescence") {
    u32::from_le_bytes(*b"WDT3")
} else { u32::from_le_bytes(*b"WDT2") };
pub const VERSION: u32 = if cfg!(feature = "freertos-r3-watchdog-refresh") { 10 }
    else if cfg!(feature = "freertos-r3-watchdog-warm-guard") { 9 }
    else if cfg!(feature = "freertos-r3-watchdog-kernel-restart") { 8 }
    else if cfg!(feature = "freertos-r3-watchdog-expiry-entry") { 7 }
    else if cfg!(feature = "freertos-r3-reset-entry-selftest") { 6 }
    else if cfg!(feature = "freertos-r3-watchdog-late-disable") { 5 }
    else if cfg!(feature = "freertos-r3-watchdog-postack") { 4 }
    else if cfg!(feature = "freertos-r3-watchdog-quiescence") { 3 } else { 2 };
pub const REQUEST: u32 = if cfg!(feature = "freertos-r3-watchdog-refresh") { u32::from_le_bytes(*b"WQL1") }
else if cfg!(feature = "freertos-r3-watchdog-warm-guard") { u32::from_le_bytes(*b"WQ09") }
else if cfg!(feature = "freertos-r3-watchdog-kernel-restart") { u32::from_le_bytes(*b"WQ08") }
else if cfg!(feature = "freertos-r3-watchdog-expiry-entry") {
    u32::from_le_bytes(*b"WQ07")
} else if cfg!(feature = "freertos-r3-reset-entry-selftest") {
    u32::from_le_bytes(*b"WQ06")
} else if cfg!(feature = "freertos-r3-watchdog-late-disable") {
    u32::from_le_bytes(*b"WQ05")
} else if cfg!(feature = "freertos-r3-watchdog-postack") {
    u32::from_le_bytes(*b"WQ04")
} else if cfg!(feature = "freertos-r3-watchdog-quiescence") {
    u32::from_le_bytes(*b"WQ03")
} else { u32::from_le_bytes(*b"WQ02") };
pub const LOAD: u32 = 0x00ff_ffff;
pub const WINDOW_US: u32 = 256;
pub const SEED: u32 = 0x5744_5432;
pub const PROBE_WORDS: usize = if cfg!(feature = "freertos-r3-watchdog-refresh") { 13 } else { 12 };

pub const fn request_words() -> [u32; 8] {
    let nonce=if NONCE_LINKED {1} else {0};
    [REQUEST, VERSION, 1, 1, LOAD, WINDOW_US, nonce, SEED ^ VERSION ^ 1 ^ 1 ^ LOAD ^ WINDOW_US ^ nonce]
}
pub fn valid_request(words: [u32; 8]) -> bool {
    let mut expected=request_words();
    if NONCE_LINKED {
        if words[6]==0 || words[6]>0xffff { return false; }
        expected[7]^=expected[6]^words[6];expected[6]=words[6];
    }
    words==expected
}

/// This same bounded descent gates the sole additional LOAD store in WDL1.
#[inline(always)]
fn valid_descent(first: u32, second: u32, elapsed: u32, iterations: u32) -> bool {
    first & 0xff00_0000 == 0x4000_0000 && second & 0xff00_0000 == 0x4000_0000 &&
    (first & LOAD) > (second & LOAD) && (second & LOAD) > LOAD-65536 &&
    elapsed >= WINDOW_US && elapsed <= 1000 && iterations > 0 && iterations <= 100_000
}

// WDL1 has no expiry-entry dependency. Preserve its known-CTRL cleanup guard.
#[cfg(feature = "freertos-r3-watchdog-refresh")]
#[inline(always)]
const fn known_ctrl(ctrl: u32) -> bool { matches!(ctrl >> 24, 0 | 0x40) }

#[inline(always)]
const fn known_disabled_ctrl(ctrl: u32) -> bool { ctrl >> 24 == 0 }

/// Pure receipt checks are reused by the target after disable and PRIMASK restore.
/// Values: initial CTRL, TICKS CTRL/CYCLES, enabled samples, disabled CTRL,
/// elapsed us, iterations, REASON before/after, PRIMASK before/after.
/// WDL1 only appends the fresh post-LOAD CTRL at word116.
pub fn valid_probe(v: [u32; PROBE_WORDS]) -> bool {
    let valid = v[0] == 0 && v[1] == 3 && v[2] == 50 &&
    valid_descent(v[3], v[4], v[6], v[7]) &&
    known_disabled_ctrl(v[5]) &&
    v[8] == 2 && v[9] == v[8] && v[10] == 0 && v[11] == v[10];
    #[cfg(feature = "freertos-r3-watchdog-refresh")]
    let valid = valid && v[12] & 0xff00_0000 == 0x4000_0000 && (v[12] & LOAD) > (v[4] & LOAD);
    valid
}

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get, put, raw_low, task};
    use rp1_freertos as os;
    use core::{arch::asm, ffi::c_void};

    #[inline(always)]
    unsafe fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
    #[inline(always)]
    unsafe fn write(address: usize, value: u32) { unsafe { (address as *mut u32).write_volatile(value); } }

    // No asserts, formatting, RTOS API, allocation or panic-capable indexing
    // between ENABLE and explicit disable. Both time and iteration bounds.
    unsafe fn probe() -> [u32; PROBE_WORDS] {
        let (saved, restored): (u32, u32);
        unsafe { asm!("mrs {0}, PRIMASK", "cpsid i", out(reg) saved, options(nostack)); }
        let ctrl = unsafe { read(0x4015_4000) };
        let tick = unsafe { read(0x4017_400c) };
        let cycles = unsafe { read(0x4017_4010) };
        let reason = unsafe { read(0x4015_4008) };
        let mut first = 0;
        let mut second = 0;
        #[cfg(feature = "freertos-r3-watchdog-refresh")]
        let mut reloaded = 0;
        let mut final_ctrl = ctrl;
        let mut elapsed = 0;
        let mut iterations = 0;
        if saved == 0 && ctrl == 0 && tick == 3 && cycles == 50 && reason == 2 {
            unsafe {
                write(0x4015_4004, LOAD);
                write(0x4015_4000, 1 << 30);
                asm!("dsb sy", options(nostack));
                first = read(0x4015_4000);
            }
            let start = raw_low();
            for _ in 0..100_000 {
                iterations += 1;
                elapsed = raw_low().wrapping_sub(start);
                if elapsed >= WINDOW_US { break; }
            }
            unsafe {
                second = read(0x4015_4000);
                #[cfg(feature = "freertos-r3-watchdog-refresh")]
                let allow_disable = {
                    let mut fresh = second;
                    if valid_descent(first, second, elapsed, iterations) {
                        write(0x4015_4004, LOAD); // ONE same-LOAD refresh; no second ENABLE.
                        asm!("dsb sy", options(nostack));
                        reloaded = read(0x4015_4000);
                        fresh = reloaded;
                    }
                    known_ctrl(fresh) // Never authorize cleanup from stale second after LOAD.
                };
                #[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
                let allow_disable = crate::freertos_r1::boot_entry::known_ctrl(second);
                #[cfg(not(any(feature = "freertos-r3-watchdog-refresh", feature = "freertos-r3-watchdog-expiry-entry")))]
                let allow_disable = true;
                if allow_disable {
                write(0x4015_4000, 0); // Only previously-zero writable CTRL fields.
                asm!("dsb sy", options(nostack));
                }
                final_ctrl = read(0x4015_4000);
            }
        }
        #[cfg(feature = "freertos-r3-watchdog-refresh")]
        if !known_disabled_ctrl(final_ctrl) {
            // Unknown/failed cleanup or an already-enabled initial state:
            // do not unmask, return to RTOS, panic, or speculate another write.
            loop { unsafe { asm!("nop", options(nomem, nostack, preserves_flags)); } }
        }
        let final_reason = unsafe { read(0x4015_4008) };
        unsafe { asm!("msr PRIMASK, {0}", "isb", in(reg) saved, options(nostack));
            asm!("mrs {0}, PRIMASK", out(reg) restored, options(nostack)); }
        #[cfg(not(feature = "freertos-r3-watchdog-refresh"))]
        { [ctrl, tick, cycles, first, second, final_ctrl, elapsed, iterations,
           reason, final_reason, saved, restored] }
        #[cfg(feature = "freertos-r3-watchdog-refresh")]
        { [ctrl, tick, cycles, first, second, final_ctrl, elapsed, iterations,
           reason, final_reason, saved, restored, reloaded] }
    }

    pub unsafe extern "C" fn worker(_: *mut c_void) {
        put(97, VERSION); put(98, 0); put(99, 0);
        unsafe { os::delay(2000).unwrap(); }
        let progress = [get(64), get(80), get(49), get(50)];
        for (i, value) in progress.into_iter().enumerate() { put(128+i, value); }
        assert!(progress.into_iter().all(|n| n > 0));
        put(140, get(8)); put(142, get(9)); put(98, 1);
        unsafe { asm!("dmb sy", options(nostack)); }
        put(96, MAGIC); // READY publication last; host may issue one fixed request.
        let start = unsafe { os::tick().unwrap() };
        while get(176) != REQUEST {
            if unsafe { os::tick().unwrap() }.wrapping_sub(start) >= 12_000 {
                put(99, 1); put(98, 0xffff_ffff); panic!("watchdog request timeout; never armed");
            }
            unsafe { os::delay(1).unwrap(); }
        }
        unsafe { asm!("dmb sy", options(nostack)); }
        let words = core::array::from_fn(|i| get(176+i));
        assert!(valid_request(words) && get(176) == REQUEST);
        #[cfg(any(feature = "freertos-r3-watchdog-refresh", feature = "freertos-r3-reset-entry-selftest", feature = "freertos-r3-watchdog-expiry-entry"))]
        put(139,words[6]); // Nonce receipt only; no cookie is armed here.
        put(100, words[2]); put(101, raw_low()); put(98, 2); // firmware accepted
        let values = unsafe { probe() };
        // Retrospective ARM/countdown receipt: never pretend host read ARMED
        // while the sub-millisecond hardware interval was in progress.
        let passed = valid_probe(values);
        for (i, value) in values.into_iter().enumerate() { put(104+i, value); }
        put(102, raw_low()); put(103, u32::from(passed));
        if !passed { put(99, 2); put(98, 0xffff_ffff); panic!("bounded watchdog receipt failed"); }
        put(98, 3); // explicit disabled readback already captured, before any wait
        unsafe { os::delay(2000).unwrap(); }
        for (i, previous) in progress.into_iter().enumerate() {
            let value = get([64,80,49,50][i]); put(132+i, value); assert_ne!(value, previous);
        }
        put(141, get(8)); put(143, get(9));
        unsafe { put(144, task(7).stack_high_water().unwrap()); }
        assert_eq!(get(70) | get(86), 0);
        put(98, 4); // terminal receipt immutable; no further request/arm/reload
        #[cfg(feature = "freertos-r3-watchdog-quiescence")]
        unsafe { crate::freertos_r1::watchdog_quiescence::wait_for_quiesce(); }
        #[cfg(not(feature = "freertos-r3-watchdog-quiescence"))]
        loop { unsafe { os::delay(1000).unwrap(); } }
    }
}
#[cfg(target_arch = "arm")]
pub use target::worker;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(any(feature = "freertos-r3-watchdog-refresh", feature = "freertos-r3-reset-entry-selftest", feature = "freertos-r3-watchdog-expiry-entry"))]
    fn nonce_is_checked_with_checksum() {
        for nonce in [1,2,0xffff] {
            let mut words=request_words();words[7]^=words[6]^nonce;words[6]=nonce;
            assert!(valid_request(words));words[7]^=1;assert!(!valid_request(words));
        }
        for nonce in [0,0x10000,u32::MAX] {
            let mut words=request_words();words[7]^=words[6]^nonce;words[6]=nonce;
            assert!(!valid_request(words));
        }
    }
    #[test]
    fn exact_request_and_bounded_receipt() {
        let request = request_words(); assert!(valid_request(request));
        for i in 0..8 { let mut bad=request; bad[i]^=1; assert!(!valid_request(bad)); }
        let mut good=[0;PROBE_WORDS];
        good[..12].copy_from_slice(&[0,3,50,0x40ff_ffff,0x40ff_fefb,0x00ff_fefa,256,1000,2,2,0,0]);
        #[cfg(feature = "freertos-r3-watchdog-refresh")]
        { good[12]=0x40ff_ffff; }
        assert!(valid_probe(good));
        for (i,value) in [(0,1),(1,1),(2,0),(3,0x00ff_ffff),(4,0x40ff_ffff),
            (4,0xc0ff_fefb),(5,0x40ff_fefa),(6,0),(6,1001),(7,0),(7,100001),
            (8,0),(9,0),(10,1),(11,1)] {
            let mut bad=good;bad[i]=value;assert!(!valid_probe(bad));
        }
    }

    #[test]
    #[cfg(feature = "freertos-r3-watchdog-refresh")]
    fn refresh_requires_new_enabled_jump_and_refuses_bad_descent() {
        assert_eq!((MAGIC, VERSION, REQUEST, PROBE_WORDS),
            (u32::from_le_bytes(*b"WDL1"), 10, u32::from_le_bytes(*b"WQL1"), 13));
        let good=[0,3,50,0x40ff_ffff,0x40ff_fefb,0x00ff_fffe,256,1000,2,2,0,0,0x40ff_ffff];
        assert!(valid_probe(good));
        for fresh in [0, good[4], good[4]-1, 0x00ff_ffff, 0x80ff_ffff, 0xc0ff_ffff, 0x41ff_ffff] {
            let mut bad=good;bad[12]=fresh;assert!(!valid_probe(bad));
        }
        for (first,second,elapsed,iterations) in [
            (good[3],good[4],255,1000), (good[3],good[4],1001,1000),
            (good[3],good[4],256,0), (good[3],good[4],256,100001),
            (good[3],good[3],256,1000), (good[4],good[3],256,1000),
            (good[3],0x40fe_ffff,256,1000), (0x00ff_ffff,good[4],256,1000),
            (good[3],0xc0ff_fefb,256,1000),
        ] { assert!(!valid_descent(first,second,elapsed,iterations)); }
        assert!(valid_descent(good[3],good[4],256,1));
        assert!(valid_descent(good[3],good[4],1000,100_000));
        for top in 0..=255 {
            assert_eq!(known_ctrl((top << 24) | LOAD), top==0 || top==0x40);
            assert_eq!(known_disabled_ctrl((top << 24) | LOAD), top==0);
        }
    }
}

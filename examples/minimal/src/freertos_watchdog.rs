//! WDT2 bounded receipt candidate. No expiry, feed policy, reset or POWER write.
//! Normal R1 only; words96..175 are firmware-owned,176..183 host-owned.
pub const MAGIC: u32 = if cfg!(feature = "freertos-r3-reset-entry-selftest") {
    u32::from_le_bytes(*b"WDT6")
} else if cfg!(feature = "freertos-r3-watchdog-late-disable") {
    u32::from_le_bytes(*b"WDT5")
} else if cfg!(feature = "freertos-r3-watchdog-postack") {
    u32::from_le_bytes(*b"WDT4")
} else if cfg!(feature = "freertos-r3-watchdog-quiescence") {
    u32::from_le_bytes(*b"WDT3")
} else { u32::from_le_bytes(*b"WDT2") };
pub const VERSION: u32 = if cfg!(feature = "freertos-r3-reset-entry-selftest") { 6 }
    else if cfg!(feature = "freertos-r3-watchdog-late-disable") { 5 }
    else if cfg!(feature = "freertos-r3-watchdog-postack") { 4 }
    else if cfg!(feature = "freertos-r3-watchdog-quiescence") { 3 } else { 2 };
pub const REQUEST: u32 = if cfg!(feature = "freertos-r3-reset-entry-selftest") {
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

pub const fn request_words() -> [u32; 8] {
    let nonce=if cfg!(feature = "freertos-r3-reset-entry-selftest") {1} else {0};
    [REQUEST, VERSION, 1, 1, LOAD, WINDOW_US, nonce, SEED ^ VERSION ^ 1 ^ 1 ^ LOAD ^ WINDOW_US ^ nonce]
}
pub fn valid_request(words: [u32; 8]) -> bool {
    let mut expected=request_words();
    if cfg!(feature = "freertos-r3-reset-entry-selftest") {
        if words[6]==0 || words[6]>0xffff { return false; }
        expected[7]^=expected[6]^words[6];expected[6]=words[6];
    }
    words==expected
}

/// Pure receipt checks are reused by the target after explicit disable.
/// Values: initial CTRL, TICKS CTRL/CYCLES, enabled samples, disabled CTRL,
/// elapsed us, iterations, REASON before/after, PRIMASK before/after.
pub fn valid_probe(v: [u32; 12]) -> bool {
    v[0] == 0 && v[1] == 3 && v[2] == 50 &&
    v[3] & 0xff00_0000 == 0x4000_0000 && v[4] & 0xff00_0000 == 0x4000_0000 &&
    (v[3] & LOAD) > (v[4] & LOAD) && (v[4] & LOAD) > LOAD-65536 &&
    v[5] & 0xff00_0000 == 0 &&
    v[6] >= WINDOW_US && v[6] <= 1000 && v[7] > 0 && v[7] <= 100_000 &&
    v[8] == 2 && v[9] == v[8] && v[10] == 0 && v[11] == v[10]
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
    unsafe fn probe() -> [u32; 12] {
        let (saved, restored): (u32, u32);
        unsafe { asm!("mrs {0}, PRIMASK", "cpsid i", out(reg) saved, options(nostack)); }
        let ctrl = unsafe { read(0x4015_4000) };
        let tick = unsafe { read(0x4017_400c) };
        let cycles = unsafe { read(0x4017_4010) };
        let reason = unsafe { read(0x4015_4008) };
        let mut first = 0;
        let mut second = 0;
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
                write(0x4015_4000, 0); // Only previously-zero writable CTRL fields.
                asm!("dsb sy", options(nostack));
                final_ctrl = read(0x4015_4000);
            }
        }
        let final_reason = unsafe { read(0x4015_4008) };
        unsafe { asm!("msr PRIMASK, {0}", "isb", in(reg) saved, options(nostack));
            asm!("mrs {0}, PRIMASK", out(reg) restored, options(nostack)); }
        [ctrl, tick, cycles, first, second, final_ctrl, elapsed, iterations,
         reason, final_reason, saved, restored]
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
        #[cfg(feature = "freertos-r3-reset-entry-selftest")]
        put(139,words[6]); // Cookie magic remains zero until final ACK and branch.
        put(100, words[2]); put(101, raw_low()); put(98, 2); // firmware accepted
        let values = unsafe { probe() };
        // Retrospective ARM/countdown receipt: never pretend host read ARMED
        // while the sub-millisecond hardware interval was in progress.
        for (i, value) in values.into_iter().enumerate() { put(104+i, value); }
        let passed = valid_probe(values);
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
        put(98, 4); // terminal receipt immutable; no second request/arm/reload
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
    fn exact_request_and_bounded_receipt() {
        let request = request_words(); assert!(valid_request(request));
        for i in 0..8 { let mut bad=request; bad[i]^=1; assert!(!valid_request(bad)); }
        let good=[0,3,50,0x40ff_ffff,0x40ff_fefb,0x00ff_fefa,256,1000,2,2,0,0];
        assert!(valid_probe(good));
        for (i,value) in [(0,1),(1,1),(2,0),(3,0x00ff_ffff),(4,0x40ff_ffff),
            (4,0xc0ff_fefb),(5,0x40ff_fefa),(6,0),(6,1001),(7,0),(7,100001),
            (8,0),(9,0),(10,1),(11,1)] {
            let mut bad=good;bad[i]=value;assert!(!valid_probe(bad));
        }
    }
}

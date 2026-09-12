//! WDT4 diagnostic packets; not a restart identity or an authenticated protocol.
//! Separate from the fixed WDT3 32-bit disabled-handoff event.
pub const ARMED: u16 = 0xa21c;
pub const ZERO_ALIVE_DISABLED: u16 = 0xa31d;
pub const ERROR: u16 = 0xa41a;
pub const LATE_COUNT_DISABLED: u16 = 0xa618;
pub const LATE_CONTROL: bool = cfg!(feature = "freertos-r3-watchdog-late-disable");
pub const TIMEOUT_US: u32 = if LATE_CONTROL { 16_000_000 } else { 20_000_000 };
pub const fn valid_frame(word: u16) -> bool {
    matches!(word, ARMED | ZERO_ALIVE_DISABLED | ERROR | LATE_COUNT_DISABLED)
}
pub const fn high_ticks(word: u16, bit: u32) -> u32 {
    if word & (1 << bit) != 0 { 150 } else { 50 }
}
pub const fn frame_ticks(word: u16) -> u32 {
    let mut ticks = 500 + 400 + 16 * 50;
    let mut bit = 0;
    while bit < 16 { ticks += high_ticks(word, bit); bit += 1; }
    ticks
}
pub const fn timed_out(now: u32, start: u32) -> bool {
    now.wrapping_sub(start) >= TIMEOUT_US
}
pub const fn owns_marker(state: u32) -> bool { state >= 6 }
pub const fn valid_enabled(first: u32, second: u32) -> bool {
    first & 0xff00_0000 == 0x4000_0000 && second & 0xff00_0000 == 0x4000_0000 &&
    (first & 0x00ff_ffff) > (second & 0x00ff_ffff) && (second & 0x00ff_ffff) > 0x00ff_ffff-65536
}
pub const fn valid_zero(ctrl: u32, elapsed: u32) -> bool {
    // Observe ENABLE either retained or cleared by hardware; do not infer a
    // reset from either. Require the selected nominal16.78s countdown envelope.
    (ctrl == 0 || ctrl == 0x4000_0000) && elapsed >= 16_000_000 && elapsed < 20_000_000
}
pub const fn valid_late(ctrl: u32, elapsed: u32) -> bool {
    ctrl & 0xff00_0000 == 0x4000_0000 && (ctrl & 0x00ff_ffff) >= 1_500_000 &&
    (ctrl & 0x00ff_ffff) <= 2_000_000 && elapsed >= 15_000_000 && elapsed < 15_200_000
}

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get, put, raw_low, watchdog_quiescence::valid_ack};
    use core::arch::asm;
    use rp1_freertos as os;
    use rp1_hal::gpio::{ConfiguredPin, Output};
    const CTRL: usize = 0x4015_4000;
    const LOAD: u32 = 0x00ff_ffff;
    unsafe fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
    unsafe fn write(address: usize, value: u32) {
        unsafe { (address as *mut u32).write_volatile(value); asm!("dsb sy", options(nostack)); }
    }
    unsafe fn disable() -> bool {
        unsafe { write(CTRL,0); }
        let ctrl = unsafe { read(CTRL) }; put(165,ctrl);
        ctrl & 0xff00_0000 == 0
    }
    unsafe fn delay(ticks: u32) -> bool { unsafe { os::delay(ticks).is_ok() } }
    unsafe fn emit(marker: &mut ConfiguredPin<22,Output>, word: u16) -> bool {
        marker.set_low();
        if !unsafe { delay(500) } { return false; }
        for bit in (0..16).rev() {
            marker.set_high();
            if !unsafe { delay(high_ticks(word,bit)) } { marker.set_low(); return false; }
            marker.set_low();
            if !unsafe { delay(50) } { return false; }
        }
        marker.set_high(); let ok = unsafe { delay(400) }; marker.set_low(); ok
    }
    unsafe fn error(marker: &mut ConfiguredPin<22,Output>, code: u32, armed: bool) {
        // Never clear an unexpected initial CTRL. Only the known-zero control
        // fields of our own armed interval are restored by this returning path.
        let restored = if armed { unsafe { disable() } }
            else { (unsafe { read(CTRL) } & 0xff00_0000) == 0 };
        put(167,code); put(168,u32::from(restored)); put(98,9);
        let sent = unsafe { emit(marker,ERROR) }; put(169,u32::from(sent));
    }
    /// Sole GPIO22/long-arm owner after the validated disabled host handoff.
    /// No unwrap/assert/allocation while armed. RTOS delay itself is not an
    /// independent recovery guarantee: the external ESP timeout remains required.
    pub unsafe fn emit_pending(marker: &mut ConfiguredPin<22,Output>) -> bool {
        let state = get(98);
        if !owns_marker(state) { return false; }
        if state != 6 { return true; } // Terminal calls consume hook, never rearm/reemit.
        let mask: u32; unsafe { asm!("mrs {0}, PRIMASK", out(reg) mask, options(nostack)); }
        let initial = unsafe { read(CTRL) };
        let ticks = unsafe { read(0x4017_400c) };
        let cycles = unsafe { read(0x4017_4010) };
        let reason = unsafe { read(0x4015_4008) };
        let ack = core::array::from_fn(|i| get(176+i));
        put(152,initial); put(153,ticks); put(154,cycles); put(155,reason);
        if mask != 0 || initial & 0xff00_0000 != 0 || ticks != 3 || cycles != 50 || reason != 2 ||
            !valid_ack(ack) || get(145) != 1 || get(146) != 1 || get(99) != 0 {
            unsafe { error(marker,1,false); } return true;
        }
        put(98,7);
        let start = raw_low(); put(158,start); // Deadline includes the enabled packet.
        unsafe { write(0x4015_4004,LOAD); write(CTRL,1<<30); }
        let first = unsafe { read(CTRL) }; put(156,first);
        if !unsafe { delay(1) } { unsafe { error(marker,2,true); } return true; }
        let second = unsafe { read(CTRL) }; put(157,second);
        if !valid_enabled(first,second) { unsafe { error(marker,3,true); } return true; }
        if !unsafe { emit(marker,ARMED) } { unsafe { error(marker,4,true); } return true; }
        put(159,raw_low());
        for iteration in 0..20_000u32 {
            let now = raw_low(); let ctrl = unsafe { read(CTRL) };
            put(160,ctrl); put(161,now.wrapping_sub(start)); put(162,iteration);
            if timed_out(now,start) { unsafe { error(marker,5,true); } return true; }
            let terminal = if LATE_CONTROL { valid_late(ctrl,now.wrapping_sub(start)) }
                else { valid_zero(ctrl,now.wrapping_sub(start)) };
            if terminal {
                put(163,unsafe { read(0x4015_4008) });
                if !unsafe { disable() } { unsafe { error(marker,7,true); } return true; }
                let disabled_at = raw_low(); put(164,disabled_at);
                // WDT5 qualifies actual disable completion, not only an earlier
                // sampled timestamp that could precede a long preemption.
                if LATE_CONTROL && !valid_late(ctrl,disabled_at.wrapping_sub(start)) {
                    unsafe { error(marker,11,false); } return true;
                }
                let word = if LATE_CONTROL { LATE_COUNT_DISABLED } else { ZERO_ALIVE_DISABLED };
                let sent = unsafe { emit(marker,word) }; put(166,u32::from(sent));
                if !sent { unsafe { error(marker,10,false); } return true; }
                put(98,8); return true;
            }
            if ctrl & 0xff00_0000 != 0x4000_0000 || ctrl & LOAD == 0 {
                unsafe { error(marker,6,true); } return true;
            }
            if !unsafe { delay(1) } { unsafe { error(marker,8,true); } return true; }
        }
        unsafe { error(marker,9,true); } true
    }
}
#[cfg(target_arch = "arm")]
pub use target::emit_pending;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_frames_and_bounded_budget() {
        for word in [ARMED, ZERO_ALIVE_DISABLED, ERROR, LATE_COUNT_DISABLED] {
            assert!(valid_frame(word));
            let checksum = ((word >> 12) ^ (word >> 8) ^ (word >> 4) ^ 5) & 15;
            assert_eq!(word & 15, checksum);
            assert_eq!((0..16).rev().fold(0u16, |v,b| (v<<1)|u16::from(high_ticks(word,b)==150)),word);
            for bit in 0..16 { assert!(!valid_frame(word ^ (1<<bit))); }
        }
        assert_eq!([frame_ticks(ARMED),frame_ticks(ZERO_ALIVE_DISABLED),frame_ticks(ERROR)], [3100,3300,3100]);
        assert_eq!(frame_ticks(LATE_COUNT_DISABLED),3100);
        assert_eq!(71+1+2*(2*16+2),140); // Measured prefix + optional initial low + packets.
        for state in 0..6 { assert!(!owns_marker(state)); }
        for state in [6,7,8,9,u32::MAX] { assert!(owns_marker(state)); }
        assert!(valid_enabled(0x40ff_fffe,0x40ff_fc00));
        for (a,b) in [(0x40ff_fffe,0x40ff_fffe),(0x40ff_fffe,0x00ff_fc00),
            (0x40ff_fffe,0x40fe_ffff),(0xc0ff_fffe,0x40ff_fc00),(0x40ff_fffe,0xffff_ffff)] {
            assert!(!valid_enabled(a,b));
        }
        for ctrl in [0,0x4000_0000] { assert!(valid_zero(ctrl,16_777_215)); }
        for (ctrl,elapsed) in [(1,16_777_215),(0x8000_0000,16_777_215),
            (0,15_999_999),(0,20_000_000)] { assert!(!valid_zero(ctrl,elapsed)); }
        assert!(valid_late(0x4000_0000|1_777_215,15_000_000));
        for (ctrl,elapsed) in [(0x4000_0000|1_499_999,15_000_000),
            (0x4000_0000|2_000_001,15_000_000),(1_777_215,15_000_000),
            (0x4000_0000|1_777_215,14_999_999),(0x4000_0000|1_777_215,15_200_000)] {
            assert!(!valid_late(ctrl,elapsed));
        }
        for start in [0, u32::MAX-100] {
            assert!(!timed_out(start.wrapping_add(TIMEOUT_US-1),start));
            assert!(timed_out(start.wrapping_add(TIMEOUT_US),start));
        }
    }
}

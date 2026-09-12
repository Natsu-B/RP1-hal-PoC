//! Disabled watchdog handoff shared by WDT3 and opt-in WDT4 post-ACK logic.
//! Default WDT3's32-bit type1 packet is not counter-zero/expiry/reset evidence.
//! WDT4 uses the separate watchdog_postack monitor hook, not this packet emitter.
pub const ACK_MAGIC: u32 = if cfg!(feature = "freertos-r3-reset-entry-selftest") {
    u32::from_le_bytes(*b"QA06")
} else if cfg!(feature = "freertos-r3-watchdog-late-disable") {
    u32::from_le_bytes(*b"QA05")
} else if cfg!(feature = "freertos-r3-watchdog-postack") {
    u32::from_le_bytes(*b"QA04")
} else { u32::from_le_bytes(*b"QA03") };
pub const FRAME: u32 = 0xa501_01ff; // magic A5, type1, sequence1, XOR-with-5A checksum
pub const fn ack_words() -> [u32; 8] {
    let version = if cfg!(feature = "freertos-r3-reset-entry-selftest") { 6 }
        else if cfg!(feature = "freertos-r3-watchdog-late-disable") { 5 }
        else if cfg!(feature = "freertos-r3-watchdog-postack") { 4 } else { 3 };
    let nonce=if cfg!(feature = "freertos-r3-reset-entry-selftest") {1} else {0};
    [ACK_MAGIC, version, 1, 2, 0, 0, nonce, 0x5744_5432 ^ version ^ 1 ^ 2 ^ nonce]
}
pub fn valid_ack(words: [u32; 8]) -> bool {
    let mut expected=ack_words();
    if cfg!(feature = "freertos-r3-reset-entry-selftest") {
        if words[6]==0 || words[6]>0xffff { return false; }
        expected[7]^=expected[6]^words[6];expected[6]=words[6];
    }
    words==expected
}
pub const fn high_ticks(bit: u32) -> u32 { if FRAME & (1 << bit) != 0 { 150 } else { 50 } }

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get, put, raw_low};
    use rp1_freertos as os;
    use rp1_hal::gpio::{ConfiguredPin, Output};
    use core::arch::asm;

    // No watchdog register write in this module. The one-shot probe has already
    // explicitly disabled CTRL, restored PRIMASK and checked task progress.
    pub unsafe fn wait_for_quiesce() -> ! {
        let start = unsafe { os::tick().unwrap() };
        while get(176) != ACK_MAGIC {
            if unsafe { os::tick().unwrap() }.wrapping_sub(start) >= 12_000 {
                put(99, 3); panic!("WDT3 quiesce timeout; watchdog disabled");
            }
            unsafe { os::delay(1).unwrap(); }
        }
        unsafe { asm!("dmb sy", options(nostack)); }
        let words = core::array::from_fn(|i| get(176+i));
        assert!(valid_ack(words) && get(176) == ACK_MAGIC);
        #[cfg(feature = "freertos-r3-reset-entry-selftest")]
        assert_eq!(words[6],get(139)); // Bind ACK to the accepted request nonce.
        assert_eq!((get(98),get(100),get(103)), (4,1,1));
        assert_eq!(get(109) & 0xff00_0000, 0);
        assert_eq!(get(115), 0);
        put(145, 1); put(146, words[2]); put(147, raw_low()); put(148, FRAME);
        unsafe { asm!("dmb sy", options(nostack)); }
        put(98, 6); // transfer event state ownership to the existing monitor task
        loop { unsafe { os::delay(1000).unwrap(); } }
    }

    /// Called only by the existing task that owns GPIO22, never from an ISR.
    /// All delays are finite; kernel/context tasks continue while it blocks.
    pub unsafe fn emit_pending(marker: &mut ConfiguredPin<22, Output>) -> bool {
        if get(98) != 6 { return false; }
        unsafe { asm!("dmb sy", options(nostack)); }
        assert_eq!((get(145),get(146),get(148)), (1,1,FRAME));
        assert_eq!((get(99),get(151)), (0,0));
        put(98, 7); put(149, raw_low());
        marker.set_low(); unsafe { os::delay(500).unwrap(); }
        for bit in (0..32).rev() {
            marker.set_high(); unsafe { os::delay(high_ticks(bit)).unwrap(); }
            marker.set_low(); unsafe { os::delay(50).unwrap(); }
        }
        marker.set_high(); unsafe { os::delay(400).unwrap(); }
        marker.set_low(); put(150, raw_low()); put(151, 1);
        unsafe { asm!("dmb sy", options(nostack)); }
        put(98, 8); // terminal event sent; normal monitor heartbeat resumes
        true
    }
}
#[cfg(target_arch = "arm")]
pub use target::{emit_pending, wait_for_quiesce};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ack_and_packet_are_fixed_and_typed() {
        assert!(valid_ack(ack_words()));
        for i in 0..8 { let mut bad=ack_words();bad[i]^=1;assert!(!valid_ack(bad)); }
        let other = if cfg!(feature = "freertos-r3-watchdog-postack") { 3 } else { 4 };
        let other_magic = if other == 3 { *b"QA03" } else { *b"QA04" };
        assert!(!valid_ack([u32::from_le_bytes(other_magic),other,1,2,0,0,0,0x5744_5432^other^1^2]));
        let bytes=FRAME.to_be_bytes();assert_eq!(bytes,[0xa5,1,1,0xff]);
        assert_eq!(bytes[3],bytes[0]^bytes[1]^bytes[2]^0x5a);
        let decoded=(0..32).rev().fold(0,|word,bit|(word<<1)|u32::from(high_ticks(bit)==150));
        assert_eq!(decoded,FRAME);
        assert_eq!((0..32).map(high_ticks).sum::<u32>()+32*50+500+400,5500);
    }
}

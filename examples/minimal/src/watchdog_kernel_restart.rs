//! WDT8: fresh proc0 kernel only, not PCIe/peripheral recovery or automatic feed.
//! One cold arm, one warm epoch, no repeated watchdog task and no PCIe reinit.
#[cfg(not(target_arch = "arm"))]
#[path = "watchdog_reset_identity.rs"]
mod identity;
#[cfg(target_arch = "arm")]
use super::reset_identity as identity;

fn packet(nonce: u32, reason: u32) -> Option<u32> {
    if !matches!(reason, 1 | 3) { return None; }
    // Reuse the checked nonce/reason layout; B -> C also updates XOR nibble.
    identity::encode_entry(nonce, reason).map(|word| word ^ 0x7000_0007)
}

// Five actual monitor passes, >=5k ticks/switches, both non-yielding spinners,
// queue and mutex-inheritance progress, no fault or assembly context error.
fn ready(v: [u32; 10]) -> bool {
    v[0] >= 5 && v[1] >= 5000 && v[2] >= 5000 && v[3] != 0 && v[4] != 0
        && v[5] >= 20 && v[6] >= 10 && v[7] == 0 && v[8] == 0 && v[9] == 0
}

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{self, get, put, reset_entry};
    use core::{arch::asm, ptr};
    use rp1_hal::gpio::{ConfiguredPin, Output};
    use rp1_freertos as os;

    static mut EPOCH: [u32; 4] = [0; 4]; // Initialized only after fresh BSS clear.
    static mut SENT: bool = false;
    pub fn is_warm() -> bool { unsafe { ptr::addr_of!(EPOCH).read()[0] != 0 } }

    unsafe fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
    fn halt() -> ! { loop { unsafe { asm!("wfe", options(nomem, nostack)); } } }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rp1_freertos_warm_start() -> ! {
        let entry = unsafe { reset_entry::record() };
        let (ipsr, primask): (u32, u32);
        unsafe { asm!("mrs {0}, IPSR", "mrs {1}, PRIMASK", out(reg) ipsr,
            out(reg) primask, options(nomem, nostack)); }
        if !identity::valid_entry(entry) || packet(entry[1], entry[2]).is_none()
            || ipsr != 0 || primask != 1 { halt(); }
        // Require the established 64-line, peripheral-IRQ-free proc0 state.
        // No unknown peripheral reset, NVIC probing, or attempted ISR recovery.
        if unsafe { read(0xe000_e004) } & 0xf != 1 { halt(); }
        for bank in 0..2 {
            for base in [0xe000_e100, 0xe000_e200, 0xe000_e300] {
                if unsafe { read(base + bank * 4) } != 0 { halt(); }
            }
        }
        unsafe {
            (0xe000_e010 as *mut u32).write_volatile(0); // SysTick disabled.
            (0xe000_e018 as *mut u32).write_volatile(0); // Clear CURRENT/COUNTFLAG.
            (0xe000_ed04 as *mut u32).write_volatile((1 << 27) | (1 << 25));
            asm!("dsb sy", "isb", options(nostack)); // PENDSVCLR/PENDSTCLR.
            if read(0xe000_e010) & 3 != 0 || read(0xe000_ed04) & 0x1400_01ff != 0
                || read(0xe000_ed28) != 0 || read(0xe000_ed2c) != 0 { halt(); }
            if rp1_rt::warm_data::info().is_none() { halt(); }
            ptr::addr_of_mut!(EPOCH).write(entry);
            let mut p = rp1_hal::Peripherals::steal();
            let mut marker = p.gpio.pin::<22>().into_output();
            marker.set_low();
            freertos_r1::run(marker); // New queues/TCBs/PSP stacks and official SVC.
        }
    }

    pub unsafe fn emit_pending(marker: &mut ConfiguredPin<22, Output>) -> bool {
        if !is_warm() { return false; }
        // Always own GPIO on the warm epoch, including before/after the packet.
        if unsafe { ptr::addr_of!(SENT).read() } || get(14) < 5 { return true; }
        assert!(ready([get(14),get(8),get(9),get(64),get(80),get(49),get(50),
            get(3)|get(4),get(70),get(86)]));
        assert_eq!((get(19),get(20),get(39)),(0x1357_9bdf,0,1));
        let entry = unsafe { ptr::addr_of!(EPOCH).read() };
        let word = packet(entry[1],entry[2]).unwrap();
        let (len,digest) = unsafe { rp1_rt::warm_data::info().unwrap() };
        // Normal runtime telemetry remains available locally; host does not
        // access RP1 after its ACK in this selected external-observer cohort.
        for (i,v) in [u32::from_le_bytes(*b"KRN8"),entry[1],entry[2],len,digest,
            get(8),get(9),get(49),get(50)].into_iter().enumerate() { put(96+i,v); }
        unsafe { ptr::addr_of_mut!(SENT).write(true); }
        marker.set_low(); unsafe { os::delay(500).unwrap(); }
        for bit in (0..32).rev() {
            marker.set_high();
            unsafe { os::delay(if word & (1<<bit) != 0 {150} else {50}).unwrap(); }
            marker.set_low(); unsafe { os::delay(50).unwrap(); }
        }
        marker.set_high(); unsafe { os::delay(400).unwrap(); }
        marker.set_low();
        true // Continue kernel/monitor checks, no new arm and no extra packet.
    }
}
#[cfg(target_arch = "arm")]
pub use target::{is_warm, emit_pending};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_progress_and_distinct_typed_packet() {
        let ok=[5,5000,5000,1,1,20,10,0,0,0]; assert!(ready(ok));
        for i in 0..10 { let mut bad=ok; bad[i]=if i<7 {0} else {1}; assert!(!ready(bad)); }
        for nonce in [1,0x8001,0xffff] { for reason in [1,3] {
            let p=packet(nonce,reason).unwrap(); assert_eq!(p>>28,0xc);
            assert_eq!(identity::decode_entry(p),None);
            assert_eq!(identity::decode_entry(p^0x7000_0007),Some((nonce as u16,reason as u8)));
        } }
        for nonce in [0,0x10000,u32::MAX] { assert_eq!(packet(nonce,1),None); }
        for reason in [0,2,4,0x100,u32::MAX] { assert_eq!(packet(1,reason),None); }
    }
}

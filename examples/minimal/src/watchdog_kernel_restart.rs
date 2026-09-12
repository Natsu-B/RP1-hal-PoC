//! WDT8: fresh proc0 kernel only, not PCIe/peripheral recovery or automatic feed.
//! One cold arm, one warm epoch, no repeated watchdog task and no PCIe reinit.
#[cfg(not(target_arch = "arm"))]
#[path = "watchdog_reset_identity.rs"]
mod identity;
#[cfg(target_arch = "arm")]
use super::reset_identity as identity;

fn packet(nonce: u32, reason: u32) -> Option<u32> {
    if !matches!(reason, 1 | 3) { return None; }
    // Reuse the checked nonce/reason layout; B -> C/D/F/9 updates XOR nibble too.
    identity::encode_entry(nonce, reason).map(|word| word ^ packet_xor())
}

const fn packet_xor() -> u32 {
    if cfg!(feature = "freertos-r3-watchdog-warm-combined") {0x3000_0003}
    else if cfg!(feature = "freertos-r3-watchdog-warm-i2c") {0x2000_0002}
    else if cfg!(feature = "freertos-r3-watchdog-warm-spi") {0x4000_0004}
    else if cfg!(feature = "freertos-r3-watchdog-warm-uart") {0x6000_0006} else {0x7000_0007}
}

// Peer timing supplies stimuli only. F still requires the real owner proof;
// bounded deferral keeps all normal R1 checks running while the owner blocks.
#[cfg(any(test, feature = "freertos-r3-watchdog-warm-spi"))]
fn spi_monitor_gate(passes: u32, owner_ready: bool, sent: bool) -> Result<bool, u32> {
    if sent && !owner_ready { return Err(0x69); }
    if !owner_ready && passes >= 35 { return Err(0x68); }
    Ok(!sent && passes >= 5 && owner_ready)
}

#[cfg(any(test, feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))]
fn i2c_monitor_gate(passes: u32, owner_ready: bool, sent: bool) -> Result<bool, u32> {
    if sent && !owner_ready { return Err(0x79); }
    // Native peer retains its original 60s first-LOW wait; allow all bounded
    // phase waits, while the host independently enforces its 4s/5s lease.
    if !owner_ready && passes >= 80 { return Err(0x78); }
    Ok(owner_ready)
}

// Five actual monitor passes, >=5k ticks/switches, both non-yielding spinners,
// queue and mutex-inheritance progress, no fault or assembly context error.
fn ready(v: [u32; 10]) -> bool {
    v[0] >= 5 && v[1] >= 5000 && v[2] >= 5000 && v[3] != 0 && v[4] != 0
        && v[5] >= 20 && v[6] >= 10 && v[7] == 0 && v[8] == 0 && v[9] == 0
}

// E's byte is a failed guard, NOT watchdog REASON and never a success packet.
pub(crate) fn diagnostic_packet(nonce: u32, code: u32) -> Option<u32> {
    let uart_code = cfg!(any(feature = "freertos-r3-watchdog-warm-uart", feature = "freertos-r3-watchdog-warm-spi", feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))
        && (matches!(code, 0x40..=0x4f) && code != 0x45 || matches!(code, 0x50..=0x54))
        || cfg!(feature = "freertos-r3-watchdog-warm-uart") && code == 0x55;
    let spi_code = cfg!(any(feature = "freertos-r3-watchdog-warm-spi", feature = "freertos-r3-watchdog-warm-combined"))
        && matches!(code, 0x60..=0x6b) && code != 0x62;
    let i2c_code = cfg!(any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined")) && matches!(code, 0x70..=0x79);
    if !uart_code && !spi_code && !i2c_code && !matches!(code, 1..=5 | 0x10..=0x12 | 0x20..=0x23 | 0x30..=0x34) {
        return None;
    }
    identity::encode_entry(nonce, code).map(|word| word ^ 0x5000_0005)
}

// AS formal E23 identified this disabled/nonactive pending bit. This admission
// does NOT enable IRQ53 or clear/acknowledge its still-unidentified source.
#[cfg(any(test, feature = "freertos-r3-watchdog-kernel-restart-masked"))]
fn masked_pending_ok(pending: u32, active: u32) -> bool {
    matches!(pending, 0 | 0x0020_0000) && active == 0
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

    #[cfg(any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))]
    #[inline(never)]
    pub fn i2c_failure(code: u32) -> ! {
        // Terminal takeover on sole proc0: stop SysTick/PendSV/worker BEFORE
        // touching its marker. No worker/ISR can resume after this diagnostic.
        unsafe {
            asm!("cpsid i", "dsb sy", "isb", options(nostack));
            #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
            put(161,0);
            let out = 0x400e_0000 as *mut u32;
            out.write_volatile(out.read_volatile() & !(1 << 22));
            asm!("dsb sy", options(nostack));
            reset_entry::diagnostic_halt_with_entry(code,ptr::addr_of!(EPOCH).read())
        }
    }
    #[cfg(any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))]
    #[inline(never)]
    pub fn i2c_monitor_ready() -> bool {
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        {
            if !freertos_r1::warm_combined::healthy() {i2c_failure(0x79);}
            let ready=freertos_r1::warm_combined::ready();
            if !ready && get(14)>=80 {i2c_failure(0x78);}
            return ready;
        }
        #[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
        {
        #[cfg(feature = "freertos-r3-watchdog-warm-combined")]
        let owner_ready = freertos_r1::warm_combined::ready();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
        let owner_ready = freertos_r1::warm_i2c::ready();
        match i2c_monitor_gate(get(14),owner_ready,unsafe { ptr::addr_of!(SENT).read() }) {
            Ok(ready) => ready, Err(code) => i2c_failure(code),
        }
        }
    }

    unsafe fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
    fn halt() -> ! { loop { unsafe { asm!("wfe", options(nomem, nostack)); } } }

    fn reject(code: u32) -> ! {
        #[cfg(feature = "freertos-r3-watchdog-warm-guard")]
        unsafe { reset_entry::diagnostic_halt(code) }
        #[cfg(not(feature = "freertos-r3-watchdog-warm-guard"))]
        { let _ = code; halt() }
    }

    #[cfg(feature = "freertos-r3-watchdog-warm-guard")]
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rp1_freertos_warm_data_failed() -> ! {
        reject(1) // Before BSS clear: reporter uses no data/BSS/kernel state.
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn rp1_freertos_warm_start() -> ! {
        let entry = unsafe { reset_entry::record() };
        let (ipsr, primask): (u32, u32);
        unsafe { asm!("mrs {0}, IPSR", "mrs {1}, PRIMASK", out(reg) ipsr,
            out(reg) primask, options(nomem, nostack)); }
        if !identity::valid_entry(entry) { halt(); } // No trustworthy nonce.
        if packet(entry[1], entry[2]).is_none() { reject(2); }
        if ipsr != 0 { reject(3); }
        if primask != 1 { reject(4); }
        // Require the established 64-line, peripheral-IRQ-free proc0 state.
        // No unknown peripheral reset, NVIC probing, or attempted ISR recovery.
        if unsafe { read(0xe000_e004) } & 0xf != 1 { reject(5); }
        for bank in 0..2 {
            for (kind, &base) in [0xe000_e100, 0xe000_e200, 0xe000_e300].iter().enumerate() {
                let value = unsafe { read(base + bank * 4) };
                #[cfg(feature = "freertos-r3-watchdog-kernel-restart-masked")]
                if bank == 1 && kind == 1
                    && masked_pending_ok(value, unsafe { read(0xe000_e304) }) {
                    // Bank0 enable/pending/active and bank1 enable already0.
                    // The next iteration still independently checks bank1 active.
                    continue;
                }
                if value != 0 {
                    // Observe known masked IRQ53 separately; do NOT allow it.
                    reject(if bank == 1 && kind == 1 && value == 0x0020_0000
                        && unsafe { read(0xe000_e304) } == 0 {
                        0x23
                    } else { 0x10 + bank as u32 * 0x10 + kind as u32 });
                }
            }
        }
        unsafe {
            (0xe000_e010 as *mut u32).write_volatile(0); // SysTick disabled.
            (0xe000_e018 as *mut u32).write_volatile(0); // Clear CURRENT/COUNTFLAG.
            (0xe000_ed04 as *mut u32).write_volatile((1 << 27) | (1 << 25));
            asm!("dsb sy", "isb", options(nostack)); // PENDSVCLR/PENDSTCLR.
            if read(0xe000_e010) & 3 != 0 { reject(0x30); }
            if read(0xe000_ed04) & 0x1400_01ff != 0 { reject(0x31); }
            if read(0xe000_ed28) != 0 { reject(0x32); }
            if read(0xe000_ed2c) != 0 { reject(0x33); }
            if rp1_rt::warm_data::info().is_none() { reject(0x34); }
            ptr::addr_of_mut!(EPOCH).write(entry);
            let mut p = rp1_hal::Peripherals::steal();
            let mut marker = p.gpio.pin::<22>().into_output();
            marker.set_low();
            #[cfg(feature = "freertos-r3-watchdog-warm-uart")]
            {
                if let Err(code) = crate::freertos_r1::warm_uart_prepare::prepare() { reject(code); }
                crate::freertos_r1::warm_uart::set_host(p.uart0.init_tx_rx_115200_clock_ready());
            }
            #[cfg(feature = "freertos-r3-watchdog-warm-spi")]
            if let Err(code) = crate::freertos_r1::warm_spi::prepare(p.spi0, &mut p.gpio) { reject(code); }
            #[cfg(feature = "freertos-r3-watchdog-warm-i2c")]
            if let Err(code) = crate::freertos_r1::warm_i2c::prepare(p.i2c1, &mut p.gpio) { i2c_failure(code); }
            #[cfg(feature = "freertos-r3-watchdog-warm-combined")]
            {
                // I2C establishes the common clock once and the idle wiring.
                // Discard its GPIO9 input handle before SPI takes the same pin.
                if let Err(code) = freertos_r1::warm_i2c::prepare(p.i2c1, &mut p.gpio) { i2c_failure(code); }
                let Some(i2c) = freertos_r1::warm_i2c::take_host() else { i2c_failure(0x77); };
                if let Err(code) = freertos_r1::warm_spi::prepare(p.spi0, &mut p.gpio) { i2c_failure(code); }
                let Some(spi) = freertos_r1::warm_spi::take_host() else { i2c_failure(0x77); };
                // Keep RX disabled until the sole owner arms its UART exchange.
                freertos_r1::warm_combined::set_hosts(spi,i2c,p.uart0.init_tx_115200_clock_ready());
            }
            freertos_r1::run(marker); // New queues/TCBs/PSP stacks and official SVC.
        }
    }

    pub unsafe fn emit_pending(marker: &mut ConfiguredPin<22, Output>) -> bool {
        if !is_warm() { return false; }
        #[cfg(feature = "freertos-r3-watchdog-kernel-restart-masked")]
        unsafe {
            // Recheck on every warm monitor pass, including after the C packet.
            assert_eq!(read(0xe000_e104) & 0x0020_0000, 0);
            assert_eq!(read(0xe000_e304) & 0x0020_0000, 0);
        }
        // Always own GPIO on the warm epoch, including before/after the packet.
        let sent = unsafe { ptr::addr_of!(SENT).read() };
        #[cfg(any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))]
        if !i2c_monitor_ready() { return true; }
        #[cfg(feature = "freertos-r3-watchdog-warm-spi")]
        match spi_monitor_gate(get(14), crate::freertos_r1::warm_spi::ready(), sent) {
            Ok(false) => return true,
            Err(code) => unsafe {
                reset_entry::diagnostic_halt_with_entry(code, ptr::addr_of!(EPOCH).read())
            },
            Ok(true) => {},
        }
        if sent || get(14) < 5 { return true; }
        #[cfg(any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))]
        if !(0..7).all(|slot| get(32+slot) >= 32) || get(160) < 32 { i2c_failure(0x77); }
        assert!(ready([get(14),get(8),get(9),get(64),get(80),get(49),get(50),
            get(3)|get(4),get(70),get(86)]));
        assert_eq!((get(19),get(20),get(39)),(0x1357_9bdf,0,1));
        let entry = unsafe { ptr::addr_of!(EPOCH).read() };
        #[cfg(feature = "freertos-r3-watchdog-warm-uart")]
        if !crate::freertos_r1::warm_uart::ready() { reject(0x55); }
        let word = packet(entry[1],entry[2]).unwrap();
        let (len,digest) = unsafe { rp1_rt::warm_data::info().unwrap() };
        // Normal runtime telemetry remains available locally; host does not
        // access RP1 after its ACK in this selected external-observer cohort.
        for (i,v) in [u32::from_le_bytes(*b"KRN8"),entry[1],entry[2],len,digest,
            get(8),get(9),get(49),get(50)].iter().copied().enumerate() { put(96+i,v); }
        unsafe { ptr::addr_of_mut!(SENT).write(true); }
        // Multi-second packet serialization is not a fresh health witness.
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        put(161,0);
        marker.set_low(); unsafe { os::delay(500).unwrap(); }
        for bit in (0..32).rev() {
            marker.set_high();
            unsafe { os::delay(if word & (1<<bit) != 0 {150} else {50}).unwrap(); }
            marker.set_low(); unsafe { os::delay(50).unwrap(); }
        }
        marker.set_high(); unsafe { os::delay(400).unwrap(); }
        marker.set_low();
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        freertos_r1::warm_combined::handback();
        true // Continue kernel/monitor checks, no new arm and no extra packet.
    }
}
#[cfg(target_arch = "arm")]
pub use target::{is_warm, emit_pending};
#[cfg(all(target_arch = "arm", any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined")))]
pub use target::{i2c_failure, i2c_monitor_ready};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_progress_and_distinct_typed_packet() {
        let ok=[5,5000,5000,1,1,20,10,0,0,0]; assert!(ready(ok));
        for i in 0..10 { let mut bad=ok; bad[i]=if i<7 {0} else {1}; assert!(!ready(bad)); }
        for nonce in [1,0x8001,0xffff] { for reason in [1,3] {
            let p=packet(nonce,reason).unwrap(); assert_eq!(p>>28,0xb ^ (packet_xor() >> 28));
            assert_eq!(identity::decode_entry(p),None);
            assert_eq!(identity::decode_entry(p^packet_xor()),Some((nonce as u16,reason as u8)));
        } }
        for nonce in [0,0x10000,u32::MAX] { assert_eq!(packet(nonce,1),None); }
        for reason in [0,2,4,0x100,u32::MAX] { assert_eq!(packet(1,reason),None); }
    }

    #[test]
    fn typed_diagnostics_cannot_be_kernel_packets() {
        for nonce in [1,0x8001,0xffff] {
            for code in 0..=255 {
                let allowed = matches!(code, 1..=5 | 0x10..=0x12 | 0x20..=0x23 | 0x30..=0x34)
                    || cfg!(any(feature = "freertos-r3-watchdog-warm-uart", feature = "freertos-r3-watchdog-warm-spi", feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined"))
                        && (matches!(code, 0x40..=0x4f) && code!=0x45 || matches!(code, 0x50..=0x54))
                    || cfg!(feature = "freertos-r3-watchdog-warm-uart") && code == 0x55
                    || cfg!(any(feature = "freertos-r3-watchdog-warm-spi", feature = "freertos-r3-watchdog-warm-combined")) && matches!(code, 0x60..=0x6b) && code != 0x62
                    || cfg!(any(feature = "freertos-r3-watchdog-warm-i2c", feature = "freertos-r3-watchdog-warm-combined")) && matches!(code, 0x70..=0x79);
                assert_eq!(diagnostic_packet(nonce,code).is_some(), allowed);
                if let Some(p)=diagnostic_packet(nonce,code) {
                    assert_eq!(p>>28,0xe);
                    assert_eq!(identity::decode_entry(p),None);
                    assert_eq!(identity::decode_entry(p ^ 0x5000_0005),Some((nonce as u16,code as u8)));
                }
            }
        }
        for nonce in [0,0x10000,u32::MAX] { assert_eq!(diagnostic_packet(nonce,1),None); }
        for code in [256,u32::MAX] { assert_eq!(diagnostic_packet(1,code),None); }
        assert_eq!(diagnostic_packet(1,0x62),None);
        for code in [0x66,0x67,0x6a,0x6b] {
            assert_eq!(diagnostic_packet(1,code).is_some(),cfg!(any(feature = "freertos-r3-watchdog-warm-spi", feature = "freertos-r3-watchdog-warm-combined")));
        }
    }

    #[test]
    fn only_selected_inactive_pending_bit_is_admitted() {
        assert!(masked_pending_ok(0,0));
        assert!(masked_pending_ok(0x0020_0000,0));
        for bit in 0..32 {
            let mask=1<<bit;
            assert!(!masked_pending_ok(0,mask));
            assert!(!masked_pending_ok(0x0020_0000,mask));
            if bit != 21 {
                assert!(!masked_pending_ok(mask,0));
                assert!(!masked_pending_ok(mask|0x0020_0000,0));
            }
        }
        assert!(!masked_pending_ok(u32::MAX,0));
    }

    #[test]
    fn spi_wait_is_bounded_and_success_requires_five_passes_and_owner() {
        for passes in 0..=36 {
            assert_eq!(spi_monitor_gate(passes, true, false), Ok(passes >= 5));
            assert_eq!(spi_monitor_gate(passes, false, false), if passes < 35 {Ok(false)} else {Err(0x68)});
            assert_eq!(spi_monitor_gate(passes, true, true), Ok(false));
            assert_eq!(spi_monitor_gate(passes, false, true), Err(0x69));
        }
        #[cfg(feature = "freertos-r3-watchdog-warm-spi")]
        assert_eq!(packet(1,1).unwrap() >> 28, 0xf);
    }

    #[test]
    fn i2c_wait_and_revocation_are_separate_from_spi() {
        for passes in 0..=81 {
            assert_eq!(i2c_monitor_gate(passes,false,false),if passes<80 {Ok(false)} else {Err(0x78)});
            assert_eq!(i2c_monitor_gate(passes,false,true),Err(0x79));
            assert_eq!(i2c_monitor_gate(passes,true,false),Ok(true));
        }
        #[cfg(feature = "freertos-r3-watchdog-warm-i2c")]
        assert_eq!(packet(1,1).unwrap() >> 28,9);
    }
}

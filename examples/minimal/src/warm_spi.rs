//! AX: exact initial-mask correction for AW's sole fresh warm SPI0 owner.
//! Two existing ESP32 69963c01/02 frames; AW01 telemetry prefix is unchanged.
//! Reuses checked IRQ19 receive/finish/cancel; no UART data owner or recovery.
//! Host check: rustc --edition=2024 --test warm_spi.rs -o /tmp/warm-spi-test
//! Words124=AW01,125=phase[7:0]/completed[9:8]/idle checks[31:16].
//! 126..139: CTRL1/DONE1 before/after, initial body[version,CTRLR0,quiet-OR,
//! SR,RISR,route], configured MISO[CTRL,PAD,STATUS,RIO_OE]. Quiet-OR combines
//! SSIENR/SER/IMR/ISR/TXFLR/RXFLR/DMACR, all required zero after checked IMR0.
//! 140/150 receipts: generation,payload,IRQs,IPSR,elapsed,IRQmax,wake-us,
//! before/after canaries, final SR<<16|priority<<8|route. Word160 is owner HWM.
//! 161..176: pre/post-mask[VERSION,SSIENR,SER,IMR,SR], post-mask body[6].
//! E66=pretuple,E6b=pre-mask NVIC,E67=posttuple,E6a=post-mask quiet/NVIC.
//! Phase1=prepared,2=owner,3/5=waiting peer,4/6=received,7=ready,ff=failure
//! (failure code replaces idle checks). WDT96..123/fault184..255 unchanged.

const RESET: u32 = 1 << 10;
const FIRST: usize = 124;
const RECEIPT_FIRST: usize = 140;
const RECEIPT_WORDS: usize = 10;
const MASK_FIRST: usize = 161;
const PREP_WORDS: usize = 30;
const BEFORE: u32 = 0x5aa5_a55a;
const AFTER: u32 = 0xa55a_5aa5;
const _: () = assert!(RECEIPT_FIRST + 2 * RECEIPT_WORDS == 160);
const _: () = assert!(FIRST + 2 + 14 == RECEIPT_FIRST && MASK_FIRST + PREP_WORDS - 14 == 177);

fn initial_reset(ctrl: u32, done: u32) -> bool { ctrl & RESET != 0 && done & RESET == 0 }
fn released(ctrl: u32, done: u32) -> bool { ctrl & RESET == 0 && done & RESET != 0 }

// G4 prewrite tuple and G5's one-write posttuple; no other reset defaults inferred.
fn pre_mask(s: [u32; 5]) -> bool { s == [0x3430_322a, 0, 0, 0x3f, 6] }
fn post_mask(s: [u32; 5]) -> bool { s == [0x3430_322a, 0, 0, 0, 6] }

// Known quiet source/serial-idle and wrapper0/1 contracts, not guessed defaults.
// CTRLR0 is observed here; the established host constructor programs it later.
fn quiet(b: [u32; 6]) -> bool {
    b[0] == 0x3430_322a && b[2] == 0 && b[3] & 5 == 4
        && b[3] & !6 == 0 && b[4] & !1 == 0 && matches!(b[5], 0 | 1)
}
fn miso_safe(p: [u32; 4]) -> bool {
    p[0] == 0x80 && p[1] & 0xff == 0xfb && p[2] & (1 << 13) == 0 && p[3] & (1 << 9) == 0
}

#[repr(C)]
struct Buffer { before: u32, bytes: [u8; 4], after: u32 }
fn buffer_ok(generation: u32, b: &Buffer) -> bool {
    matches!(generation, 1 | 2) && b.before == BEFORE && b.after == AFTER
        && u32::from_be_bytes(b.bytes) == (0x6996_3c00 | generation)
}
fn receipt_ok(generation: u32, r: [u32; RECEIPT_WORDS]) -> bool {
    matches!(generation, 1 | 2) && r[0] == generation && r[1] == (0x6996_3c00 | generation)
        && (1..=5).contains(&r[2]) && r[3] == 35 && r[4] > 0
        && r[5] <= r[4] && r[6] <= r[4] && r[7] == BEFORE && r[8] == AFTER
        && matches!(r[9], 0x0004_c001 | 0x0006_c001)
}
fn complete(completed: u32, active: u32, irqs: u32, expected: u32) -> bool {
    completed == 3 && active == 0 && expected != 0 && irqs == expected
}

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get, put};
    use core::{ffi::c_void, ptr};
    use rp1_freertos as os;
    use rp1_hal::{gpio::Gpio, spi::{Spi0, Spi0Host}};

    static mut PREP: [u32; PREP_WORDS] = [0; PREP_WORDS];
    static mut HOST: Option<Spi0Host> = None;
    static mut HOST_STATE: u32 = 0; // 0=fresh,1=published,2=permanently taken.
    static mut IRQ_ENTRIES: u32 = 0; // Only the direct IRQ19 handler writes.
    static mut IRQ_IPSR: u32 = 0;
    static mut COMPLETED: u32 = 0;
    static mut READY_IRQS: u32 = 0; // Publish last, zero is never ready.

    fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
    fn mask_tuple() -> [u32; 5] {
        let b = rp1_hal::addr::SPI0_BASE;
        [read(b+0x5c), read(b+0x08), read(b+0x10), read(b+0x2c), read(b+0x28)]
    }
    fn body() -> [u32; 6] {
        let b = rp1_hal::addr::SPI0_BASE;
        [read(b+0x5c), read(b), [0x08,0x10,0x2c,0x30,0x20,0x24,0x4c]
            .iter().copied().fold(0, |v,o| v | read(b+o)), read(b+0x28), read(b+0x34), read(b+0x108)]
    }
    fn miso() -> [u32; 4] { [read(0x400d_004c),read(0x400f_0028),read(0x400d_0048),read(0x400e_0004)] }
    fn wires_idle() -> bool { read(0x400e_0008) & ((1<<7)|(1<<8)|(1<<11)) == ((1<<7)|(1<<8)) }
    fn nvic_quiet() -> bool {
        [0xe000_e100,0xe000_e200,0xe000_e300].iter().copied().all(|a| read(a) & (1<<19) == 0)
    }
    fn priority() -> u32 { unsafe { (0xe000_e413 as *const u8).read_volatile().into() } }
    fn saved(index: usize, value: u32) {
        #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
        unsafe { ptr::addr_of_mut!(PREP).cast::<u32>().add(index).write_volatile(value); }
        // AZ keeps the same before-write predicates, not AW's unused raw mirror.
        #[cfg(feature = "freertos-r3-watchdog-warm-combined")]
        let _ = (index,value);
    }

    /// Before scheduler, only after fresh BSS and global NVIC admission. SPI body
    /// is inaccessible until both selected reset checks and release have passed.
    /// Retain AV's unchanged common PLL + UART0 release prerequisite, but never
    /// initialize a UART data host or enable UART IRQ25 in this selected image.
    pub fn prepare(spi: Spi0, gpio: &mut Gpio) -> Result<(), u32> {
        let ctrl = read(0x4001_4004); let done = read(0x4001_401c);
        saved(0,ctrl); saved(1,done);
        if !initial_reset(ctrl,done) { return Err(0x60); }
        #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
        super::super::warm_uart_prepare::prepare()?;
        crate::release_spi0_reset_bank1_bit10().map_err(|_| 0x61u32)?;
        let ctrl = read(0x4001_4004); let done = read(0x4001_401c);
        saved(2,ctrl); saved(3,done);
        if !released(ctrl,done) { return Err(0x61); }
        let before = body();
        // BC has no PREP mirror. Keep all reads/predicates, omit only the
        // otherwise live no-op iterator calls left by the size-optimized build.
        #[cfg(not(feature = "freertos-r3-watchdog-warm-persistent"))]
        for (i,v) in before.into_iter().enumerate() { saved(4+i,v); }
        let pre = mask_tuple();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-persistent"))]
        for (i,v) in pre.into_iter().enumerate() { saved(14+i,v); }
        if !pre_mask(pre) { return Err(0x66); }
        if !nvic_quiet() { return Err(0x6b); }
        rp1_hal::spi::spi0_mask_tx_empty_irq();
        unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
        let post = mask_tuple();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-persistent"))]
        for (i,v) in post.into_iter().enumerate() { saved(19+i,v); }
        if !post_mask(post) { return Err(0x67); }
        let masked = body();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-persistent"))]
        for (i,v) in masked.into_iter().enumerate() { saved(24+i,v); }
        if !quiet(masked) || !nvic_quiet() { return Err(0x6a); }
        // The same released-input / CS-high / SCLK-low prerequisite as cold R1.
        let _miso = gpio.pin::<9>().into_input_pull_up();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
        let _sda = gpio.pin::<2>().into_input_pull_up();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
        let _scl = gpio.pin::<3>().into_input_pull_up();
        let mut cs0 = gpio.pin::<8>().into_output(); cs0.set_high();
        let mut cs1 = gpio.pin::<7>().into_output(); cs1.set_high();
        let mut sclk = gpio.pin::<11>().into_output(); sclk.set_low();
        if !_miso.is_high() || !wires_idle() { return Err(0x63); }
        let host = spi.into_host_mode0_100khz(gpio.pin::<8>(),gpio.pin::<9>(),gpio.pin::<10>(),gpio.pin::<11>())
            .map_err(|_| 0x63u32)?;
        let pad = crate::spi0_miso_input_observation::apply_guarded_bias().map_err(|_| 0x64u32)?;
        let pins = miso();
        #[cfg(not(feature = "freertos-r3-watchdog-warm-persistent"))]
        for (i,v) in pins.into_iter().enumerate() { saved(10+i,v); }
        if !miso_safe(pins) || pad != pins[1] || pins[2] & (1<<17) == 0 || !wires_idle() { return Err(0x64); }
        let after = body();
        let b = rp1_hal::addr::SPI0_BASE;
        if !quiet(after) || after[1] != 0x0007_0100 || after[5] != before[5]
            || read(b+0x14) != 2000 || read(b+0x18) != 0 || read(b+0x1c) != 0
            || !nvic_quiet() || unsafe { ptr::addr_of!(HOST_STATE).read_volatile() } != 0 {
            return Err(0x65);
        }
        unsafe { ptr::addr_of_mut!(HOST).write(Some(host)); ptr::addr_of_mut!(HOST_STATE).write_volatile(1); }
        Ok(())
    }

    #[cfg(feature = "freertos-r3-watchdog-warm-combined")]
    pub fn take_host() -> Option<Spi0Host> {
        unsafe {
            if ptr::addr_of!(HOST_STATE).read_volatile() != 1 { return None; }
            ptr::addr_of_mut!(HOST_STATE).write_volatile(2);
            ptr::addr_of_mut!(HOST).replace(None)
        }
    }

    pub fn publish() {
        put(FIRST,u32::from_le_bytes(*b"AW01")); put(FIRST+1,1);
        for i in 0..14 { put(FIRST+2+i,unsafe { ptr::addr_of!(PREP).cast::<u32>().add(i).read_volatile() }); }
        for i in 14..PREP_WORDS { put(MASK_FIRST+i-14,unsafe { ptr::addr_of!(PREP).cast::<u32>().add(i).read_volatile() }); }
    }
    fn irq_entries() -> u32 { unsafe { ptr::addr_of!(IRQ_ENTRIES).read_volatile() } }
    fn fail(code: u32) -> ! {
        unsafe { ptr::addr_of_mut!(READY_IRQS).write_volatile(0); }
        put(FIRST+1,(code<<16)|(get(FIRST+1)&0x300)|0xff);
        panic!("warm SPI contract failure");
    }
    fn check(ok: bool, code: u32) { if !ok { fail(code); } }
    fn phase(value: u32) { put(FIRST+1,value|(unsafe { ptr::addr_of!(COMPLETED).read_volatile() }<<8)); }
    fn miso_high() -> bool {
        let pins = miso(); check(miso_safe(pins) && wires_idle(),7);
        pins[2] & (1<<17) != 0
    }
    unsafe fn wait_level(high: bool, ticks: u32) {
        let deadline = unsafe { os::tick().unwrap() }.wrapping_add(ticks);
        while miso_high() != high {
            check(os::deadline_remaining(unsafe { os::tick().unwrap() },deadline).is_some(),8);
            unsafe { os::delay(1).unwrap(); }
        }
    }
    pub fn ready() -> bool {
        let expected = unsafe { ptr::addr_of!(READY_IRQS).read_volatile() };
        expected != 0 && complete(unsafe { ptr::addr_of!(COMPLETED).read_volatile() },
            unsafe { os::spi0::active_generation() },irq_entries(),expected)
    }

    /// Slot7/id8,priority5,512 words. The host supplies finite existing peer leases
    /// after quiescence+20s; this timing is NOT proof. MISO-low wait is bounded.
    pub unsafe extern "C" fn worker(arg: *mut c_void) {
        check(arg.is_null(),1);
        let (ipsr,control,psp): (u32,u32,u32);
        unsafe { core::arch::asm!("mrs {0}, IPSR","mrs {1}, CONTROL","mrs {2}, PSP",
            out(reg) ipsr,out(reg) control,out(reg) psp,options(nomem,nostack)); }
        check(ipsr == 0 && control & 3 == 2 && psp & 7 == 0,2);
        check(unsafe { ptr::addr_of!(HOST_STATE).read_volatile() } == 1,3);
        let Some(host) = (unsafe { ptr::addr_of_mut!(HOST).replace(None) }) else { fail(4); };
        unsafe { ptr::addr_of_mut!(HOST_STATE).write_volatile(2); }
        phase(2);
        check(irq_entries() == 0 && unsafe { os::spi0::active_generation() } == 0 && nvic_quiet(),5);
        let mut driver = unsafe { os::spi0::Driver::new(host) };
        check(priority() == 0xc0 && nvic_quiet(),6);
        let mut rx = Buffer { before:BEFORE,bytes:[0xc3;4],after:AFTER };
        let mut expected_irqs = 0;
        for generation in 1..=2 {
            unsafe { wait_level(true,3000); }
            phase(generation*2+1);
            unsafe { wait_level(false,30_000); }
            check(irq_entries() == expected_irqs && unsafe { os::spi0::active_generation() } == 0
                && nvic_quiet() && quiet(body()),9);
            rx.bytes.fill(0xc3);
            let result = unsafe { driver.receive(&[0;4],&mut rx.bytes,50) };
            let Some(r) = driver.last_receipt() else { fail(10); };
            let snapshot = unsafe { ptr::addr_of!(rx).read_volatile() };
            let after = body();
            let words = [r.generation,u32::from_be_bytes(snapshot.bytes),r.irq_entries,
                unsafe { ptr::addr_of!(IRQ_IPSR).read_volatile() },r.elapsed_us,r.irq_body_max_us,
                r.irq_end_to_task_us,snapshot.before,snapshot.after,(after[3]<<16)|(priority()<<8)|after[5]];
            for (i,v) in words.into_iter().enumerate() { put(RECEIPT_FIRST+(generation as usize-1)*RECEIPT_WORDS+i,v); }
            check(result.is_ok() && receipt_ok(generation,words) && buffer_ok(generation,&snapshot),11);
            check(quiet(after) && after[1] == 0x0007_0000 && after[5] == 1 && nvic_quiet(),12);
            expected_irqs += r.irq_entries;
            check(irq_entries() == expected_irqs && unsafe { os::spi0::active_generation() } == 0,13);
            check(!unsafe { os::spi0::cancel(generation) },14);
            unsafe { ptr::addr_of_mut!(COMPLETED).write_volatile((1<<generation)-1); }
            phase(generation*2+2);
            unsafe { wait_level(true,3000); os::delay(1000).unwrap(); }
        }
        check(complete(unsafe { ptr::addr_of!(COMPLETED).read_volatile() },
            unsafe { os::spi0::active_generation() },irq_entries(),expected_irqs),15);
        phase(7);
        unsafe { ptr::addr_of_mut!(READY_IRQS).write_volatile(expected_irqs); }
        let mut idle_checks = 0u32;
        loop {
            unsafe { os::delay(1000).unwrap(); }
            check(ready() && nvic_quiet() && quiet(body()) && miso_high(),16);
            check(driver.last_receipt().is_some_and(|r| r.generation == 2)
                && buffer_ok(2,&unsafe { ptr::addr_of!(rx).read_volatile() }),17);
            idle_checks = idle_checks.wrapping_add(1);
            put(FIRST+1,(idle_checks<<16)|0x307);
        }
    }

    #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn SPI0_IRQHandler() {
        let ipsr: u32;
        unsafe { core::arch::asm!("mrs {0}, IPSR",out(reg) ipsr,options(nomem,nostack));
            ptr::addr_of_mut!(IRQ_IPSR).write_volatile(ipsr);
            ptr::addr_of_mut!(IRQ_ENTRIES).write_volatile(irq_entries().saturating_add(1));
            os::spi0::on_interrupt(); }
    }
}
#[cfg(target_arch = "arm")]
pub use target::{prepare,publish,ready,worker};
#[cfg(all(target_arch = "arm", feature = "freertos-r3-watchdog-warm-combined"))]
pub use target::take_host;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_exact_pre_and_post_mask_tuples_are_admitted() {
        let pre = [0x3430_322a,0,0,0x3f,6];
        let post = [0x3430_322a,0,0,0,6];
        assert!(pre_mask(pre)); assert!(post_mask(post));
        assert!(!pre_mask(post)); assert!(!post_mask(pre));
        for i in 0..5 { for bit in 0..32 {
            let mut bad = pre; bad[i] ^= 1 << bit; assert!(!pre_mask(bad));
            let mut bad = post; bad[i] ^= 1 << bit; assert!(!post_mask(bad));
        } }
        // SR4 is allowed by the later broad quiet gate, never by this exact gate.
        let mut bad = pre; bad[4] = 4; assert!(!pre_mask(bad));
        let mut bad = post; bad[4] = 4; assert!(!post_mask(bad));
        assert_eq!(FIRST+2+13,139);
        assert_eq!(MASK_FIRST+PREP_WORDS-14-1,176);
    }
    #[test]
    fn only_safe_reset_quiet_body_and_released_miso_are_admitted() {
        for ctrl in [0,RESET,u32::MAX,!RESET] { for done in [0,RESET,u32::MAX,!RESET] {
            assert_eq!(initial_reset(ctrl,done),ctrl & RESET != 0 && done & RESET == 0);
            assert_eq!(released(ctrl,done),ctrl & RESET == 0 && done & RESET != 0);
            assert!(!(initial_reset(ctrl,done) && released(ctrl,done)));
        } }
        let good = [0x3430_322a,0,0,6,1,0]; assert!(quiet(good));
        for (i,v) in [(0,0),(2,1),(3,7),(3,2),(3,0x106),(4,2),(4,0x10),(5,2),(5,u32::MAX)] {
            let mut bad=good; bad[i]=v; assert!(!quiet(bad));
        }
        let mut alternate=good; alternate[5]=1; assert!(quiet(alternate));
        let pins=[0x80,0xfb,0,0]; assert!(miso_safe(pins));
        for (i,v) in [(0,0x85),(1,0x73),(2,1<<13),(3,1<<9)] {
            let mut bad=pins; bad[i]=v; assert!(!miso_safe(bad));
        }
    }
    #[test]
    fn two_exact_canary_checked_irq_receipts_then_no_active_or_extra_irq() {
        assert_eq!(RECEIPT_FIRST + 2 * RECEIPT_WORDS - 1,159);
        for generation in 1..=2 {
            let b=Buffer {before:BEFORE,bytes:(0x6996_3c00u32|generation).to_be_bytes(),after:AFTER};
            assert!(buffer_ok(generation,&b)); assert!(!buffer_ok(3-generation,&b));
            let good=[generation,u32::from_be_bytes(b.bytes),4,35,500,30,20,BEFORE,AFTER,0x0006_c001];
            assert!(receipt_ok(generation,good));
            for (i,v) in [(0,0),(1,0),(2,0),(2,6),(3,19),(4,0),(5,501),(6,501),(7,0),(8,0),(9,0x0007_c001),(9,0x0006_0001)] {
                let mut bad=good; bad[i]=v; assert!(!receipt_ok(generation,bad));
            }
        }
        let mut b=Buffer {before:BEFORE,bytes:0x6996_3c02u32.to_be_bytes(),after:AFTER};
        b.before^=1; assert!(!buffer_ok(2,&b)); b.before=BEFORE;
        b.after^=1; assert!(!buffer_ok(2,&b));
        assert!(complete(3,0,8,8));
        for v in [(0,0,0,0),(1,0,4,4),(2,0,4,4),(3,1,8,8),(3,0,7,8),(3,0,9,8)] {
            assert!(!complete(v.0,v.1,v.2,v.3));
        }
    }
}

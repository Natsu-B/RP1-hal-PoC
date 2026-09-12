//! AY: sole fresh warm I2C1/IRQ8 owner, unchanged finite native READYACK exchange.
//! AY01/phase124..125; prep126..139,156..159; receipts140..155,161..176;
//! owner HWM160; pulse widths177..181; direct IRQ count/IPSR182..183.
//! WDT96..123 and fault184..255 remain untouched. No normal-peer ABI changes.
const BEFORE: u32 = 0x5aa5_a55a;
const AFTER: u32 = 0xa55a_5aa5;
const RESET: u32 = 1 << 8;
const RECEIPTS: [usize; 2] = [140, 161];
fn initial_reset(ctrl: u32, done: u32) -> bool { ctrl & RESET != 0 && done & RESET == 0 }
fn released(ctrl: u32, done: u32) -> bool { ctrl & RESET == 0 && done & RESET != 0 }
// No IC_STATUS equality: only known identity, disabled/quiet state and FIFO counts.
fn initial_body(b: [u32; 13]) -> bool {
    b[..10] == [0x001f_1fea, 0x3230_322a, 0x4457_0140, 0, 0, 0, 0x48ff, 0, 0, 0]
        && b[11] == 0 && b[12] == 0
}
#[inline(never)]
fn input_admitted(pre: [u32; 7]) -> bool {
    pre[0] == 0x85 && pre[1] == 0xda && pre[2] & (1 << 13) == 0 && pre[3] == 0x0040_0980
        && pre[5] & (1 << 19) == 0 && pre[6] & (1 << 19) != 0
}
#[inline(never)]
fn receipt_ok(generation: u32, r: [u32; 16]) -> bool {
    matches!(generation, 1 | 2) && r[0] == generation && r[1] > 0 && r[1] <= 132
        && r[2] == if generation == 1 {0} else {2}
        && r[3] > 0 && r[4] <= r[3] && r[5] <= r[3]
        && r[6] == if generation == 1 {0x40} else {0}
        && r[7] == if generation == 1 {0x0080_0001} else {0}
        && r[8] == 0 && r[9] >= 4000 && r[10] >= 2 && r[11] < 10_000
        && r[12] == BEFORE && r[13] == if generation == 1 {0xc3c3_c3c3} else {0x314e_c3c3}
        && r[14] == AFTER && r[15] == 1
}

#[cfg(target_arch = "arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get, put, raw_low, MARKER, kernel_restart};
    use core::{ffi::c_void, ptr};
    use rp1_freertos as os;
    use rp1_hal::{gpio::{ConfiguredPin, Gpio, Input, Output}, i2c::{self, I2c1, I2c1Host},
        i2c_rx_state::{Error as RxError, OWNED_CAUSES}};
    static mut PREP: [u32; 18] = [0; 18];
    static mut HOST: Option<I2c1Host> = None;
    static mut PIN: Option<ConfiguredPin<9, Input>> = None;
    static mut READY_IRQS: u32 = 0;
    static mut IRQ_COUNT: u32 = 0;
    static mut IRQ_IPSR: u32 = 0;
    #[repr(C)]
    struct Buffer { before: u32, bytes: [u8; 4], after: u32 }
    fn read(a: usize) -> u32 { unsafe { (a as *const u32).read_volatile() } }
    fn barrier() { unsafe { core::arch::asm!("dsb sy", options(nostack)); } }
    fn save(i: usize, v: u32) {
        #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
        unsafe { ptr::addr_of_mut!(PREP).cast::<u32>().add(i).write_volatile(v); }
        // AZ has its own schema; retain all prewrite checks, not AY's raw mirror.
        #[cfg(feature = "freertos-r3-watchdog-warm-combined")]
        let _ = (i,v);
    }
    fn nvic_quiet() -> bool {
        (read(0xe000_e100) | read(0xe000_e200) | read(0xe000_e300)) & (1 << 8) == 0
    }
    fn body() -> [u32; 13] {
        let b = rp1_hal::addr::I2C1_BASE;
        [0xf4,0xf8,0xfc,0x108,0x6c,0x9c,0x30,0x34,0x2c,0x80,0x70,0x74,0x78].map(|o| read(b+o))
    }
    #[inline(never)]
    fn input() -> [u32; 7] {
        [read(0x400d_004c),read(0x400f_0028)&0xff,read(0x400d_0048),
            read(0x400e_0004),read(0x400e_0008),read(0x4001_4004),read(0x4001_401c)]
    }
    fn clean() -> bool {
        let s = i2c::i2c1_read1_snapshot();
        s.irq.enable_status == 0 && s.irq.interrupt_mask == 0 && s.rx_level == 0 && s.tx_level == 0
            && (s.irq.raw_interrupt_status | s.irq.masked_interrupt_status) & OWNED_CAUSES == 0
            && s.irq.masked_interrupt_status == 0 && s.irq.abort_source == 0 && nvic_quiet()
    }
    pub fn prepare(i2c: I2c1, gpio: &mut Gpio) -> Result<(), u32> {
        let ctrl = read(0x4001_4000); let done = read(0x4001_4018);
        save(0,ctrl); save(1,done);
        if !initial_reset(ctrl,done) { return Err(0x70); }
        super::super::warm_uart_prepare::prepare()?;
        crate::release_i2c1_reset_bank0_bit8().map_err(|_| 0x71u32)?;
        let ctrl = read(0x4001_4000); let done = read(0x4001_4018);
        save(2,ctrl); save(3,done);
        if !released(ctrl,done) { return Err(0x71); }
        // Non-clearing reads BEFORE constructor disable/mask/CLR_INTR writes.
        let b = body(); for (i,v) in b.into_iter().enumerate() { save(4+i,v); }
        if !initial_body(b) { return Err(0x72); }
        if !nvic_quiet() { return Err(0x73); }
        let pin = gpio.pin::<9>().into_input_pull_up();
        let _sda = gpio.pin::<2>().into_input_pull_up();
        let _scl = gpio.pin::<3>().into_input_pull_up();
        let mut cs0 = gpio.pin::<8>().into_output(); cs0.set_high();
        let mut cs1 = gpio.pin::<7>().into_output(); cs1.set_high();
        let mut sclk = gpio.pin::<11>().into_output(); sclk.set_low(); barrier();
        let pre = input(); save(17,pre[2]);
        if !input_admitted(pre) || !pin.is_high() || pre[4] & 0xc != 0xc { return Err(0x74); }
        let host = i2c.into_host_100khz(gpio.pin::<2>(),gpio.pin::<3>()).map_err(|_| 0x75u32)?;
        let b = rp1_hal::addr::I2C1_BASE;
        if !clean() || read(b) != 0x63 || read(b+0x14) != 781 || read(b+0x18) != 1186
            || read(b+0x7c) != 593 || read(b+0x108) != 0 { return Err(0x76); }
        unsafe {
            if ptr::addr_of!(HOST).read().is_some() || ptr::addr_of!(PIN).read().is_some() { return Err(0x77); }
            ptr::addr_of_mut!(HOST).write(Some(host)); ptr::addr_of_mut!(PIN).write(Some(pin));
        }
        Ok(())
    }
    #[cfg(feature = "freertos-r3-watchdog-warm-combined")]
    pub fn take_host() -> Option<I2c1Host> {
        unsafe {
            // End the READYACK-era input handle lifetime before SPI MISO owns9.
            let _pin = ptr::addr_of_mut!(PIN).replace(None)?;
            ptr::addr_of_mut!(HOST).replace(None)
        }
    }
    pub fn publish() {
        put(124,u32::from_le_bytes(*b"AY01")); put(125,1);
        for i in 0..18 { put(if i < 14 {126+i} else {156+i-14},
            unsafe { ptr::addr_of!(PREP).cast::<u32>().add(i).read_volatile() }); }
    }
    #[inline(never)]
    fn check(ok: bool) { if !ok { kernel_restart::i2c_failure(0x77); } }
    fn sample(b: &Buffer) -> [u32; 3] {
        unsafe { [ptr::addr_of!(b.before).read_volatile(),
            u32::from_be_bytes(ptr::addr_of!(b.bytes).read_volatile()),ptr::addr_of!(b.after).read_volatile()] }
    }
    fn store(generation: u32, r: os::i2c1::Receipt, b: &Buffer) {
        let s = sample(b);
        let words = [r.generation,r.irq_entries,r.received,r.elapsed_us,r.irq_body_max_us,
            r.irq_end_to_task_us,r.first_fatal_causes,r.first_abort_source,r.discarded_after_failure,
            r.cleanup_elapsed_us,r.quiet_samples,r.quiet_max_gap_us,s[0],s[1],s[2],r.higher_priority_wakes];
        for (i,v) in words.into_iter().enumerate() { put(RECEIPTS[generation as usize-1]+i,v); }
        check(receipt_ok(generation,words));
        check(unsafe { os::i2c1::active_generation() } == 0);
        check(!unsafe { os::i2c1::cancel(0) } && !unsafe { os::i2c1::cancel(generation) });
        check(unsafe { os::notification_take(true,0).unwrap() } == 0);
        let old = sample(b); unsafe { os::delay(2).unwrap(); } check(sample(b) == old);
    }
    fn idle_high() -> Option<bool> {
        check(clean()); let p = input(); check(input_admitted(p) && p[4] & 0xc == 0xc);
        let high = p[4] & (1 << 9) != 0;
        if p[2] & (7 << 17) != if high {7 << 17} else {0} {None} else {Some(high)}
    }
    unsafe fn wait(high: bool, ticks: u32) {
        let deadline = unsafe { os::tick().unwrap() }.wrapping_add(ticks); let mut stable = None;
        loop {
            check(os::deadline_remaining(unsafe { os::tick().unwrap() },deadline).is_some());
            if idle_high() == Some(high) {
                let first = *stable.get_or_insert(raw_low());
                if raw_low().wrapping_sub(first) >= 2000 { break; }
            } else { stable = None; }
            unsafe { os::delay(1).unwrap(); }
        }
    }
    #[inline(never)]
    unsafe fn pulse(marker: &mut ConfiguredPin<22,Output>, width: u32, index: usize) {
        let start = raw_low(); marker.set_high(); unsafe { os::delay(width).unwrap(); }
        marker.set_low(); barrier(); put(index,raw_low().wrapping_sub(start));
    }
    pub fn ready() -> bool {
        let expected = unsafe { ptr::addr_of!(READY_IRQS).read_volatile() };
        // Before handback the worker may legitimately have an active transfer;
        // do not inspect changing body/pin state until it publishes readiness.
        if expected == 0 { return false; }
        let p = input();
        expected != 0 && expected == unsafe { ptr::addr_of!(IRQ_COUNT).read_volatile() }
            && unsafe { ptr::addr_of!(IRQ_IPSR).read_volatile() } == 24 && get(125) == 4
            && [1,2].into_iter().all(|g| receipt_ok(g,core::array::from_fn(|i| get(RECEIPTS[g as usize-1]+i))))
            && unsafe { os::i2c1::active_generation() } == 0 && clean()
            && input_admitted(p) && p[4] & ((1 << 9) | 0xc) == ((1 << 9) | 0xc)
            && p[2] & (7 << 17) == 7 << 17
    }
    pub unsafe extern "C" fn worker(_: *mut c_void) {
        // Only this task takes GPIO22. Monitor holds None until the final handback.
        let mut marker = unsafe { ptr::addr_of_mut!(MARKER).replace(None).unwrap() };
        marker.set_low();
        let _pin = unsafe { ptr::addr_of_mut!(PIN).replace(None).unwrap() };
        let host = unsafe { ptr::addr_of_mut!(HOST).replace(None).unwrap() };
        let driver = unsafe { os::i2c1::Driver::new(host) };
        let mut buffer = Buffer { before: BEFORE, bytes: [0xc3;4], after: AFTER };
        unsafe { wait(true,3000); pulse(&mut marker,13,177); }
        put(125,10); unsafe { wait(false,60_000); } put(125,11);
        let result = unsafe { driver.receive(0x2d,&mut buffer.bytes[..2],50) };
        store(1,driver.last_receipt().unwrap(),&buffer);
        check(matches!(result,Err(os::i2c1::Error::Receive(RxError::Fatal { causes:0x40,abort_source:0x0080_0001 }))));
        unsafe { wait(false,1000); pulse(&mut marker,17,178); } put(125,12);
        unsafe { wait(true,3000); pulse(&mut marker,19,179); } put(125,13);
        unsafe { wait(false,3000); } put(125,14);
        let result = unsafe { driver.receive(0x2d,&mut buffer.bytes[..2],50) };
        store(2,driver.last_receipt().unwrap(),&buffer); check(result.is_ok());
        unsafe { wait(false,1000); pulse(&mut marker,23,180); } put(125,15);
        unsafe { wait(true,3000); pulse(&mut marker,29,181); }
        check(idle_high() == Some(true)); marker.set_low(); barrier();
        let count = unsafe { ptr::addr_of!(IRQ_COUNT).read_volatile() };
        check(count == get(141)+get(162) && count != 0);
        unsafe { ptr::addr_of_mut!(MARKER).write(Some(marker)); } barrier();
        put(125,4);
        unsafe { ptr::addr_of_mut!(READY_IRQS).write_volatile(count); } barrier();
        // Monitor alone owns both marker and post-result revocation from here.
        loop { unsafe { os::delay(1000).unwrap(); } }
    }
    #[cfg(not(feature = "freertos-r3-watchdog-warm-combined"))]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn I2C1_IRQHandler() {
        let ipsr: u32; unsafe { core::arch::asm!("mrs {}, IPSR",out(reg) ipsr,options(nomem,nostack)); }
        unsafe {
            ptr::addr_of_mut!(IRQ_IPSR).write_volatile(ipsr);
            let count = ptr::addr_of!(IRQ_COUNT).read_volatile().saturating_add(1);
            ptr::addr_of_mut!(IRQ_COUNT).write_volatile(count); put(182,count); put(183,ipsr);
            os::i2c1::on_interrupt();
        }
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
    fn guards_receipts_and_nonoverlapping_layout() {
        assert!(initial_reset(RESET,0)); assert!(released(0,RESET));
        assert!(!initial_reset(RESET,RESET)); assert!(!released(RESET,RESET));
        let body = [0x001f_1fea,0x3230_322a,0x4457_0140,0,0,0,0x48ff,0,0,0,0xfeed,0,0];
        assert!(initial_body(body));
        for i in 0..13 { let mut bad=body; bad[i]^=1; assert_eq!(initial_body(bad),i==10); }
        assert!(input_admitted([0x85,0xda,0,0x0040_0980,0,0,1<<19]));
        for g in 1..=2 {
            let r=[g,1,if g==1 {0}else{2},6000,10,10,if g==1 {0x40}else{0},
                if g==1 {0x0080_0001}else{0},0,4500,5,1000,BEFORE,
                if g==1 {0xc3c3_c3c3}else{0x314e_c3c3},AFTER,1];
            assert!(receipt_ok(g,r));
            for i in [0,2,6,7,8,12,13,14,15] { let mut bad=r; bad[i]^=1; assert!(!receipt_ok(g,bad)); }
        }
        let mut used=[false;256];
        for (start,len) in [(96,28),(124,2),(126,14),(140,16),(156,4),(160,1),(161,16),(177,5),(182,2),(184,72)] {
            for i in start..start+len { assert!(!used[i]); used[i]=true; }
        }
        assert_eq!(RECEIPTS,[140,161]); assert_eq!(1792+512,2304);
    }
}

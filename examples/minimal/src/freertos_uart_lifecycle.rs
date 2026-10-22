//! Hook-free timeout/cancel prelude; unchanged USB peer then serves wire1/2.
//! Wire sequences1/2 are driver generations3/4, not a reused cancellation tag.
use super::*;

pub unsafe fn monitor_wait(ticks:u32) {
    let deadline=unsafe { os::tick().unwrap() }.wrapping_add(ticks);
    while let Some(left)=os::deadline_remaining(unsafe { os::tick().unwrap() },deadline) {
        if unsafe { os::notification_take(true,left).unwrap() }==0 { continue; }
        let phase=get(120);assert!(phase==1 || phase==2);put(120,0);
        // exchange() blocks in preflight cleanup BEFORE publishing its ticket.
        // Poll only that software marker, yielding each tick, with a hard bound.
        let publication=unsafe { os::tick().unwrap() }.wrapping_add(30);
        loop {
            let generation=unsafe { os::uart0::active_generation() };
            if generation==phase { break; }
            assert_eq!(generation,0);
            assert!(os::deadline_remaining(unsafe { os::tick().unwrap() },publication).is_some());
            unsafe { os::delay(1).unwrap(); }
        }
        if phase==1 {
            // Read-only proof the no-response timeout actually passed preflight
            // and armed RX/IRQ, rather than timing out before publication.
            assert_eq!(unsafe { (0x4003_0030 as *const u32).read_volatile() }&0x301,0x301);
            assert_eq!(unsafe { (0x4003_0038 as *const u32).read_volatile() },0x50);
            assert_ne!(unsafe { (0xe000_e100 as *const u32).read_volatile() }&(1<<25),0);
            put(122,get(122)|128);continue;
        }
        assert!(!unsafe { os::uart0::cancel(0) });
        assert!(!unsafe { os::uart0::cancel(1) });put(122,get(122)|12);
        assert!(unsafe { os::uart0::cancel(2) });put(122,get(122)|16);put(121,2);
    }
}

fn unchanged(buffer:&Buffer) {
    assert_eq!(unsafe { ptr::addr_of!(buffer.bytes).read_volatile() },[0xc3;20]);
    assert_eq!(unsafe { ptr::addr_of!(buffer.before).read_volatile() },0x5aa5_a55a);
    assert_eq!(unsafe { ptr::addr_of!(buffer.after).read_volatile() },0xa55a_5aa5);
}
unsafe fn failed(driver:&os::uart0::Driver,buffer:&Buffer,generation:u32) {
    let r=driver.last_receipt().unwrap();assert_eq!(r.generation,generation);
    assert_eq!(r.received|r.irq_entries|r.ipsr|r.irq_body_max_us|r.irq_end_to_task_us|
        r.higher_priority_wakes|r.first_ris|r.first_mis|r.rsr_errors|r.first_error_dr|
        r.overflow_bytes|r.residual_bytes,0);
    assert!(r.cleanup_elapsed_us>=8000 && r.cleanup_elapsed_us<40_000 && r.quiet_samples>=4);
    assert!(r.elapsed_us>=r.cleanup_elapsed_us && r.elapsed_us<100_000);
    assert_eq!(r.final_cr&0x301,0x101);assert_eq!(r.final_imsc,0);assert_eq!(r.final_fr&0x18,0x10);
    for (i,v) in [r.generation,r.received,r.irq_entries,r.elapsed_us,r.cleanup_elapsed_us,
        r.rsr_errors,r.overflow_bytes,r.residual_bytes,r.quiet_samples,r.final_cr,r.final_imsc,r.final_fr]
        .into_iter().enumerate() { put(96+(generation as usize-1)*12+i,v); }
    unchanged(buffer);assert_eq!(unsafe { os::uart0::active_generation() },0);
    assert_eq!(unsafe { os::notification_take(true,0).unwrap() },0);
    let enabled=unsafe { (0xe000_e100 as *const u32).read_volatile() };
    let pending=unsafe { (0xe000_e200 as *const u32).read_volatile() };
    let active=unsafe { (0xe000_e300 as *const u32).read_volatile() };
    assert_eq!((enabled|pending|active)&(1<<25),0);
    increment(131);
}
pub(super) unsafe fn before(driver:&os::uart0::Driver,buffer:&mut Buffer) {
    put(128,u32::from_le_bytes(*b"UL01"));
    assert!(!unsafe { os::uart0::cancel(1) });put(122,1);
    // No prompt and no peer write: this request must block until its deadline.
    put(120,1);unsafe { task(0).notification_give().unwrap(); }
    let result=unsafe { driver.exchange(b"",&mut buffer.bytes[..19],20) };
    assert!(matches!(result,Err(os::uart0::Error::Timeout)));
    unsafe { failed(driver,buffer,1); }
    assert!(!unsafe { os::uart0::cancel(1) });put(122,get(122)|2);

    assert_ne!(get(122)&128,0);
    put(120,2);unsafe { task(0).notification_give().unwrap(); }
    let result=unsafe { driver.exchange(b"",&mut buffer.bytes[..19],1000) };
    assert!(matches!(result,Err(os::uart0::Error::Cancelled)));
    unsafe { failed(driver,buffer,2);os::delay(2).unwrap(); }
    assert_eq!(get(121),2);assert!(!unsafe { os::uart0::cancel(2) });put(122,get(122)|32);
    unchanged(buffer);put(122,get(122)|64);
}

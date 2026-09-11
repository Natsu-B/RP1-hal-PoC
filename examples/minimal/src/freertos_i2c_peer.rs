//! Present ESP0.6.9 READYACK lease: STOP/NACK, HIGH, RESTART/read2, release.
//! Marker ownership transfers to monitor only after final HIGH and done marker.
//! No firmware success before an actual IRQ wakes a blocked task with0x31,0x4e.
use super::*;
use rp1_hal::{i2c,i2c_rx_state::OWNED_CAUSES};
static mut PIN:Option<ConfiguredPin<9,rp1_hal::gpio::Input>>=None;
pub fn set_pin(pin:ConfiguredPin<9,rp1_hal::gpio::Input>) { unsafe { ptr::addr_of_mut!(PIN).write(Some(pin)); } }

/// Opt-in probe: preserve monitor's absolute deadline and GPIO22 ownership.
#[cfg(feature = "freertos-r2-i2c-cancel-window")]
pub unsafe fn monitor_wait(ticks:u32) {
    let deadline=unsafe { os::tick().unwrap() }.wrapping_add(ticks);
    while let Some(left)=os::deadline_remaining(unsafe { os::tick().unwrap() },deadline) {
        if unsafe { os::notification_take(true,left).unwrap() }==0 { continue; }
        assert_eq!(get(112),2);put(112,0);
        assert_eq!(unsafe { os::i2c1::active_generation() },2);
        assert!(!unsafe { os::i2c1::cancel(0) });
        assert!(!unsafe { os::i2c1::cancel(1) });put(180,3);
        assert!(unsafe { os::i2c1::cancel(2) });put(113,2);
    }
}

/// Terminal ISR has masked sources, but checked cleanup has NOT happened yet.
/// Words96..122/180..183 are exclusive to this image;184..255 stay fault-owned.
#[cfg(feature = "freertos-r2-i2c-cancel-window")]
#[unsafe(no_mangle)]
unsafe extern "C" fn rp1_i2c_cancel_window_probe(generation:u32) {
    assert_eq!(generation,2);assert_eq!(unsafe { os::i2c1::active_generation() },2);
    let s=i2c::i2c1_read1_snapshot();
    for (i,v) in [generation,s.irq.interrupt_mask,read(0xe000_e100)&(1<<8),
        s.irq.enable_status,s.rx_level,s.tx_level].into_iter().enumerate() { put(114+i,v); }
    assert_eq!(get(115)|get(116)|get(118)|get(119),0);
    let started=raw_low();put(120,unsafe { os::tick().unwrap() });put(112,2);
    unsafe { task(0).notification_give().unwrap();os::delay(2).unwrap(); }
    put(121,raw_low().wrapping_sub(started));
    put(122,unsafe { os::i2c1::active_generation() });
    assert_eq!(get(113),2);assert_eq!(get(122),2);
    assert!(get(121)>0 && get(121)<10_000);
}

fn read(address:usize)->u32 { unsafe { (address as *const u32).read_volatile() } }
fn input_admitted(pre:[u32;7])->bool {
    pre[0]==0x85 && pre[1]==0xda && pre[2]&(1<<13)==0 && pre[3]==0x0040_0980
        && pre[5]&(1<<19)==0 && pre[6]&(1<<19)!=0
}
fn idle_high()->Option<bool> {
    let s=i2c::i2c1_read1_snapshot();
    assert!(s.irq.enable_status==0 && s.irq.interrupt_mask==0 && s.rx_level==0 && s.tx_level==0);
    assert!((s.irq.raw_interrupt_status|s.irq.masked_interrupt_status)&OWNED_CAUSES==0 && s.irq.abort_source==0);
    assert!((read(0xe000_e100)|read(0xe000_e200)|read(0xe000_e300))&(1<<8)==0);
    assert!(read(0x400d_004c)==0x85 && read(0x400f_0028)&0xff==0xda);
    assert!(read(0x400e_0004)&(1<<9)==0);
    let status=read(0x400d_0048);let input=read(0x400e_0008);
    assert!(status&(1<<13)==0 && input&0xc==0xc);
    let high=input&(1<<9)!=0;
    // An edge between samples is not a grant. Restart the complete dwell.
    if status&(7<<17)!=if high {7<<17} else {0} { None } else { Some(high) }
}
unsafe fn wait(high:bool,ticks:u32) {
    let deadline=unsafe { os::tick().unwrap() }.wrapping_add(ticks);
    let mut stable=None;
    loop {
        assert!(os::deadline_remaining(unsafe { os::tick().unwrap() },deadline).is_some());
        if idle_high()==Some(high) {
            let first=*stable.get_or_insert(raw_low());
            if raw_low().wrapping_sub(first)>=2000 { break; }
        } else { stable=None; }
        unsafe { os::delay(1).unwrap(); }
    }
}
unsafe fn pulse(marker:&mut ConfiguredPin<22,Output>,width:u32,index:usize) {
    let start=raw_low();marker.set_high();unsafe { os::delay(width).unwrap(); }marker.set_low();
    put(index,raw_low().wrapping_sub(start));
}
pub unsafe fn run(driver:&os::i2c1::Driver)->! {
    let mut marker=unsafe { ptr::addr_of_mut!(MARKER).replace(None).unwrap() };
    marker.set_low();put(128,u32::from_le_bytes(*b"RI02"));put(133,0x2d);
    #[cfg(feature = "freertos-r2-i2c-cancel-window")]
    put(182,u32::from_le_bytes(*b"ICW1")); // Compatible READY; distinct final ABI.
    // R1 startup already configures this input plus idle CS0/CS1/SCLK outputs.
    // Take that same typed handle; do not reconfigure or assume a cold reset.
    let pre=[read(0x400d_004c),read(0x400f_0028),read(0x400d_0048),
        read(0x400e_0004),read(0x400e_0008),read(0x4001_4004),read(0x4001_401c)];
    for (i,v) in pre.into_iter().enumerate() { put(173+i,v); }
    assert!(input_admitted(pre));
    let _input=unsafe { ptr::addr_of_mut!(PIN).replace(None).unwrap() };
    let mut buffer=Buffer { before:0x5aa5_a55a,bytes:[0xc3;4],after:0xa55a_5aa5 };
    unsafe { wait(true,3000); pulse(&mut marker,13,168); }
    put(129,10); // READY: host may arm only after the whole marker pair.
    unsafe { wait(false,60_000); }
    put(129,11);
    let result=unsafe { driver.receive(0x2d,&mut buffer.bytes[..2],50) };
    let first=driver.last_receipt().unwrap();store(1,first,&buffer);
    assert!(matches!(result,Err(os::i2c1::Error::Receive(RxError::Fatal { causes:0x40,abort_source:0x0080_0001 }))));
    assert!(first.generation==1 && first.irq_entries>0 && first.higher_priority_wakes==1 && first.received==0);
    assert!(sample(&buffer)==[0x5aa5_a55a,0xc3c3_c3c3,0xa55a_5aa5]);
    put(131,1);
    unsafe { wait(false,1000); pulse(&mut marker,17,169); }
    put(129,12); // CLEAN1: host observes both edges then separator.
    unsafe { wait(true,3000); pulse(&mut marker,19,170); }
    put(129,13); // READY2: host observes both edges then reset/restart/preload.
    unsafe { wait(false,3000); }
    put(129,14);
    let result=unsafe { driver.receive(0x2d,&mut buffer.bytes[..2],50) };
    let second=driver.last_receipt().unwrap();store(2,second,&buffer);
    #[cfg(not(feature = "freertos-r2-i2c-cancel-window"))]
    assert!(result.is_ok());
    #[cfg(feature = "freertos-r2-i2c-cancel-window")]
    {
        assert!(matches!(result,Err(os::i2c1::Error::Cancelled)));
        assert_eq!(unsafe { os::i2c1::active_generation() },0);
        assert!(!unsafe { os::i2c1::cancel(2) });put(180,get(180)|4);
        assert_eq!(unsafe { os::notification_take(true,0).unwrap() },0);
        put(180,get(180)|16);
        let saved=sample(&buffer);unsafe { os::delay(2).unwrap(); }
        assert_eq!(sample(&buffer),saved);put(181,1);
    }
    assert!(second.generation==2 && second.irq_entries>0 && second.higher_priority_wakes==1);
    assert!(second.received==2 && second.first_fatal_causes==0 && second.first_abort_source==0);
    assert!(sample(&buffer)==[0x5aa5_a55a,0x314e_c3c3,0xa55a_5aa5]);
    put(131,2);
    unsafe { wait(false,1000); pulse(&mut marker,23,171); }
    put(129,15); // DONE2: host observes both edges then final release.
    unsafe { wait(true,3000); pulse(&mut marker,29,172); }
    #[cfg(feature = "freertos-r2-i2c-cancel-window")]
    {
        // Native lease is released before this separate unassigned-address probe.
        // Do NOT store(3): its legacy base168 would overwrite peer witnesses.
        buffer.bytes=[0xc3;4];
        let result=unsafe { driver.receive(0x2e,&mut buffer.bytes[..2],50) };
        let third=driver.last_receipt().unwrap();store_at(96,third,&buffer);
        assert!(matches!(result,Err(os::i2c1::Error::Receive(RxError::Fatal { causes:0x40,abort_source:0x0080_0001 }))));
        assert!(third.generation==3 && third.irq_entries>0 && third.higher_priority_wakes==1 && third.received==0);
        assert_eq!(sample(&buffer),[0x5aa5_a55a,0xc3c3_c3c3,0xa55a_5aa5]);
        assert_eq!(unsafe { os::i2c1::active_generation() },0);
        assert!(!unsafe { os::i2c1::cancel(3) });put(180,get(180)|8);
        assert_eq!(unsafe { os::notification_take(true,0).unwrap() },0);
        assert_eq!(idle_high(),Some(true));put(183,1);put(131,3);
    }
    unsafe { ptr::addr_of_mut!(MARKER).write(Some(marker));core::arch::asm!("dsb sy",options(nostack)); }
    put(129,4);
    loop { unsafe { os::delay(1000).unwrap(); } increment(132); }
}

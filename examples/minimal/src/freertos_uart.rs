//! UART0 task/IRQ integration using the existing external USB-UART peer.
//! Two actual19B replies; not yet a ring-overflow or mixed-peripheral cohort.
use super::*;
use rp1_hal::uart::Uart0Tx;
static mut HOST:Option<Uart0Tx>=None;
pub fn set_host(host:Uart0Tx) { unsafe { ptr::addr_of_mut!(HOST).write(Some(host)); } }

#[repr(C)]
struct Buffer { before:u32,bytes:[u8;20],after:u32 }

fn store(generation:u32,r:os::uart0::Receipt,buffer:&Buffer) {
    let base=136+(generation as usize-1)*24;
    for (i,v) in [r.generation,r.received,r.irq_entries,r.ipsr,r.elapsed_us,
        r.irq_body_max_us,r.irq_end_to_task_us,r.higher_priority_wakes,
        r.first_ris,r.first_mis,r.rsr_errors,r.first_error_dr,r.overflow_bytes,
        r.residual_bytes,r.cleanup_elapsed_us,r.quiet_samples,r.final_cr,
        r.final_imsc,r.final_fr].into_iter().enumerate() { put(base+i,v); }
    let bytes=unsafe { ptr::addr_of!(buffer.bytes).read_volatile() };
    for i in 0..5 { put(base+19+i,u32::from_be_bytes(bytes[i*4..i*4+4].try_into().unwrap())); }
    unsafe {
        put(184,ptr::addr_of!(buffer.before).read_volatile());
        put(185,ptr::addr_of!(buffer.after).read_volatile());
    }
}

pub unsafe extern "C" fn worker(_: *mut c_void) {
    let (ipsr,control,psp,msp):(u32,u32,u32,u32);
    unsafe { core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
        out(reg) ipsr,out(reg) control,out(reg) psp,out(reg) msp,options(nomem,nostack)); }
    for (i,v) in [ipsr,control,psp,msp].into_iter().enumerate() { put(124+i,v); }
    assert_eq!(ipsr,0);assert_eq!(control&3,2);assert_eq!(psp&7,0);
    put(128,u32::from_le_bytes(*b"RU01"));put(129,1);put(133,115200);
    let host=unsafe { ptr::addr_of_mut!(HOST).replace(None).unwrap() };
    let mut driver=unsafe { os::uart0::Driver::new(host) };
    let mut buffer=Buffer { before:0x5aa5_a55a,bytes:[0xc3;20],after:0xa55a_5aa5 };
    assert!(matches!(unsafe { driver.exchange(b"",&mut [],50) },Err(os::uart0::Error::InvalidArgument)));
    assert!(matches!(unsafe { driver.exchange(b"",&mut buffer.bytes,0) },Err(os::uart0::Error::InvalidArgument)));
    put(130,2);
    for (index,(ready,payload,ack)) in [
        (&b"RP1U0 RTOSREADY 0001\r\n"[..],&b"HOST2RP1 IRQ 0001\r\n"[..],&b"RP1U0 RTOSOK 0001\r\n"[..]),
        (&b"RP1U0 RTOSREADY 0002\r\n"[..],&b"HOST2RP1 IRQ 0002\r\n"[..],&b"RP1U0 RTOSOK 0002\r\n"[..]),
    ].into_iter().enumerate() {
        unsafe { os::delay(100).unwrap(); }
        buffer.bytes.fill(0xc3);put(129,2);
        // The adapter publishes/arms RX before sending READY, then blocks.
        let result=unsafe { driver.exchange(ready,&mut buffer.bytes[..19],2000) };
        let generation=index as u32+1;
        if let Some(r)=driver.last_receipt() { store(generation,r,&buffer); }
        let r=result.unwrap();
        assert_eq!(r.generation,generation);assert_eq!(r.received,19);
        assert!(r.irq_entries>0);assert_eq!(r.ipsr,41);assert!(r.higher_priority_wakes>0);
        assert_eq!(&buffer.bytes[..19],payload);assert_eq!(buffer.bytes[19],0xc3);
        assert_eq!(get(184),0x5aa5_a55a);assert_eq!(get(185),0xa55a_5aa5);
        assert_eq!(unsafe { os::uart0::active_generation() },0);
        assert!(!unsafe { os::uart0::cancel(generation) });
        unsafe { driver.write_all(ack,100).unwrap(); }
        increment(131);put(191,generation);put(129,3);
    }
    for (index,address) in [(186,0x4001_8054),(187,0x4001_8058),(188,0x4001_8060),
        (189,0x4002_0010)] { put(index,unsafe { (address as *const u32).read_volatile() }); }
    put(190,unsafe { (0xe000_e419 as *const u8).read_volatile() } as u32);
    put(129,4);
    loop { unsafe { os::delay(1000).unwrap(); } increment(132); }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn UART0_IRQHandler() { unsafe { os::uart0::on_interrupt(); } }

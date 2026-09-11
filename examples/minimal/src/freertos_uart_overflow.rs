//! Natural TX window: the owner does not pop its RX ring while sending a prompt.
//! One80B external burst into expected96B must overflow, then two real rearms.
use super::*;

const PREFIX:&[u8]=b"RP1U0 OVERFLOWREADY 0001\r\n";
const END:&[u8]=b"\r\nEND\r\n";
const fn prompt()->[u8;1024] {
    let mut bytes=[b'x';1024];let mut i=0;
    while i<PREFIX.len() {bytes[i]=PREFIX[i];i+=1;}
    i=0;while i<END.len() {bytes[1024-END.len()+i]=END[i];i+=1;}
    bytes
}
static PROMPT:[u8;1024]=prompt();
#[repr(C)]
struct Burst {before:u32,bytes:[u8;96],after:u32}

pub unsafe fn before(driver:&mut os::uart0::Driver) {
    put(128,u32::from_le_bytes(*b"UO01"));
    let mut buffer=Burst {before:0x5aa5_a55a,bytes:[0xc3;96],after:0xa55a_5aa5};
    let result=unsafe {driver.exchange(&PROMPT,&mut buffer.bytes,1000)};
    let r=driver.last_receipt().unwrap();
    for (i,v) in [r.generation,r.received,r.irq_entries,r.ipsr,r.elapsed_us,
        r.irq_body_max_us,r.irq_end_to_task_us,r.higher_priority_wakes,
        r.first_ris,r.first_mis,r.rsr_errors,r.first_error_dr,r.overflow_bytes,
        r.residual_bytes,r.cleanup_elapsed_us,r.quiet_samples,r.final_cr,
        r.final_imsc,r.final_fr].into_iter().enumerate() {put(96+i,v);}
    assert!(matches!(result,Err(os::uart0::Error::Overflow)));
    assert_eq!(r.generation,1);assert_eq!(r.received,64);
    assert!(r.irq_entries>0);assert_eq!(r.ipsr,41);
    assert!(r.first_mis&0x50!=0 && r.first_ris&0x50!=0);
    assert_eq!(r.rsr_errors|r.first_error_dr,0);
    assert!(r.overflow_bytes>0 && r.overflow_bytes<=16 && r.residual_bytes<=64);
    assert!(r.cleanup_elapsed_us>=8000 && r.cleanup_elapsed_us<40000);
    assert!(r.elapsed_us>=r.cleanup_elapsed_us && r.elapsed_us<200000);
    assert!(r.irq_body_max_us>0 && r.irq_body_max_us<10000);
    assert!(r.irq_end_to_task_us>0 && r.irq_end_to_task_us<10000);
    assert!(r.quiet_samples>=4 && r.final_cr&0x301==0x101 && r.final_imsc==0 && r.final_fr&0x18==0x10);
    assert_eq!(unsafe {os::uart0::active_generation()},0);
    assert!(!unsafe {os::uart0::cancel(1)});
    assert_eq!(unsafe {os::notification_take(true,0).unwrap()},0);
    for address in [0xe000_e100u32,0xe000_e200,0xe000_e300] {
        assert_eq!(unsafe {(address as *const u32).read_volatile()}&(1<<25),0);
    }
    for i in 0..64 {assert_eq!(buffer.bytes[i],i as u8);}
    assert!(buffer.bytes[64..].iter().all(|&v|v==0xc3));
    let saved=buffer.bytes;unsafe {os::delay(2).unwrap();}
    assert_eq!(unsafe {ptr::addr_of!(buffer.bytes).read_volatile()},saved);
    assert_eq!(unsafe {ptr::addr_of!(buffer.before).read_volatile()},0x5aa5_a55a);
    assert_eq!(unsafe {ptr::addr_of!(buffer.after).read_volatile()},0xa55a_5aa5);
    put(115,64);put(116,u32::from_be_bytes(saved[..4].try_into().unwrap()));
    put(117,u32::from_be_bytes(saved[60..64].try_into().unwrap()));put(118,32);
    put(119,buffer.before);put(120,buffer.after);put(121,7);
    unsafe {driver.write_all(b"\r\nRP1U0 OVERFLOWOK 0001\r\n",100).unwrap();}
    increment(131);
}

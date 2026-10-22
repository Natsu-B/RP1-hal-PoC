//! First I2C task integration: unassigned-address NACK/IRQ/cleanup/rearm.
//! Physical peer is ESP32 at0x2d. Never invoke its protected read callback here.
//! Not successful payload reception; that needs the peer's READYACK contract.
use super::*;
use rp1_hal::{i2c::I2c1Host, i2c_rx_state::Error as RxError};
static mut HOST:Option<I2c1Host>=None;
#[cfg(feature = "freertos-r2-i2c-peer")]
#[path = "freertos_i2c_peer.rs"]
mod peer;
#[cfg(feature = "freertos-r2-i2c-cancel-window")]
pub use peer::monitor_wait;
#[cfg(feature = "freertos-r2-i2c-peer")]
pub fn set_peer_pin(pin:ConfiguredPin<9,rp1_hal::gpio::Input>) { peer::set_pin(pin); }
pub fn set_host(host:I2c1Host) { unsafe { ptr::addr_of_mut!(HOST).write(Some(host)); } }
#[repr(C)]
struct Buffer { before:u32,bytes:[u8;4],after:u32 }
fn sample(buffer:&Buffer)->[u32;3] {
    unsafe { [ptr::addr_of!(buffer.before).read_volatile(),
        u32::from_be_bytes(ptr::addr_of!(buffer.bytes).read_volatile()),
        ptr::addr_of!(buffer.after).read_volatile()] }
}
pub unsafe extern "C" fn worker(_: *mut c_void) {
    let (ipsr,control,psp,msp):(u32,u32,u32,u32);
    unsafe { core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
        out(reg) ipsr,out(reg) control,out(reg) psp,out(reg) msp,options(nomem,nostack)); }
    for (i,v) in [ipsr,control,psp,msp].into_iter().enumerate() { put(124+i,v); }
    put(128,u32::from_le_bytes(*b"RI01"));put(129,1);put(133,0x2e);
    let host=unsafe { ptr::addr_of_mut!(HOST).replace(None).unwrap() };
    let driver=unsafe { os::i2c1::Driver::new(host) };
    #[cfg(feature = "freertos-r2-i2c-peer")]
    unsafe { peer::run(&driver); }
    #[cfg(not(feature = "freertos-r2-i2c-peer"))]
    unsafe { nack(&driver); }
}

fn store(generation:u32,r:os::i2c1::Receipt,buffer:&Buffer) {
    store_at(136+(generation as usize-1)*16,r,buffer);
}
fn store_at(base:usize,r:os::i2c1::Receipt,buffer:&Buffer) {
    for (i,v) in [r.generation,r.irq_entries,r.received,r.elapsed_us,
        r.irq_body_max_us,r.irq_end_to_task_us,r.first_fatal_causes,r.first_abort_source,
        r.discarded_after_failure,r.cleanup_elapsed_us,r.quiet_samples,r.quiet_max_gap_us,
        sample(buffer)[0],sample(buffer)[1],sample(buffer)[2],r.higher_priority_wakes].into_iter().enumerate() { put(base+i,v); }
}

#[cfg(not(feature = "freertos-r2-i2c-peer"))]
unsafe fn nack(driver:&os::i2c1::Driver)->! {
    let mut buffer=Buffer { before:0x5aa5_a55a,bytes:[0xc3;4],after:0xa55a_5aa5 };
    assert!(matches!(unsafe { driver.receive(0x2e,&mut [],50) },Err(os::i2c1::Error::InvalidArgument)));
    assert!(matches!(unsafe { driver.receive(0x2e,&mut buffer.bytes,0) },Err(os::i2c1::Error::InvalidArgument)));
    assert!(matches!(unsafe { driver.receive(0x80,&mut buffer.bytes,50) },Err(os::i2c1::Error::InvalidArgument)));
    put(130,3);
    for generation in 1..=2 {
        unsafe { os::delay(50).unwrap(); }
        put(129,2);
        let result=unsafe { driver.receive(0x2e,&mut buffer.bytes[..2],50) };
        if let Some(r)=driver.last_receipt() {
            store(generation,r,&buffer);
            assert_eq!(r.generation,generation);assert!(r.irq_entries>0);
            assert_eq!(r.higher_priority_wakes,1);
            assert_eq!(r.received,0);assert_eq!(r.first_fatal_causes,1<<6);
            // Preserve the entire observed two-command abort word. Bit23's
            // flush-count meaning is an IP-family inference, not RP1 proof.
            assert_eq!(r.first_abort_source,0x0080_0001);assert_eq!(r.discarded_after_failure,0);
        }
        assert!(matches!(result,Err(os::i2c1::Error::Receive(RxError::Fatal { causes:0x40,abort_source:0x0080_0001 }))));
        assert_eq!(sample(&buffer),[0x5aa5_a55a,0xc3c3_c3c3,0xa55a_5aa5]);
        assert_eq!(unsafe { os::i2c1::active_generation() },0);
        assert!(!unsafe { os::i2c1::cancel(generation) });
        increment(131);put(129,3);
    }
    put(129,4);
    loop { unsafe { os::delay(1000).unwrap(); } increment(132); }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn I2C1_IRQHandler() { unsafe { os::i2c1::on_interrupt(); } }

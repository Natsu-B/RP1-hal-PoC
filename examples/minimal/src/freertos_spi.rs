//! Two existing ESP32 low/hi-Z frames, received by a blocking FreeRTOS task.
//! This first integrated SPI stage is not continuous acquisition or200us proof.
use super::*;
use rp1_hal::spi::Spi0Host;
static mut HOST: Option<Spi0Host> = None;

pub fn set_host(host: Spi0Host) { unsafe { ptr::addr_of_mut!(HOST).write(Some(host)); } }

fn miso_high() -> bool {
    let ctrl = unsafe { (0x400d_004c as *const u32).read_volatile() };
    let pad = unsafe { (0x400f_0028 as *const u32).read_volatile() };
    let status = unsafe { (0x400d_0048 as *const u32).read_volatile() };
    let oe = unsafe { (0x400e_0004 as *const u32).read_volatile() };
    assert_eq!(ctrl, 0x80);
    assert_eq!(pad & 0xff, 0xfb);
    assert_eq!(status & (1 << 13), 0);
    assert_eq!(oe & (1 << 9), 0);
    status & (1 << 17) != 0
}

unsafe fn wait_level(high: bool, ticks: u32) {
    let deadline = unsafe { os::tick().unwrap() }.wrapping_add(ticks);
    while miso_high() != high {
        assert!(os::deadline_remaining(unsafe { os::tick().unwrap() }, deadline).is_some());
        unsafe { os::delay(1).unwrap(); }
    }
}

#[repr(C)]
struct Buffer { before: u32, bytes: [u8; 4], after: u32 }

pub unsafe extern "C" fn worker(_: *mut c_void) {
    let (ipsr, control, psp, msp): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
            out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
    }
    for (i,v) in [ipsr,control,psp,msp].into_iter().enumerate() { put(124+i,v); }
    let host = unsafe { ptr::addr_of_mut!(HOST).replace(None).unwrap() };
    let mut driver = unsafe { os::spi0::Driver::new(host) };
    put(128, u32::from_le_bytes(*b"RS01"));
    for id in 1..=2 {
        unsafe { wait_level(true, 3000); }
        put(130, id); put(129, 1); // guarded idle, ready for ESP miso_peer id
        unsafe { wait_level(false, 30_000); }
        put(129, 2);
        let mut rx = Buffer { before:0x5aa5_a55a, bytes:[0xc3;4], after:0xa55a_5aa5 };
        let receipt = unsafe { driver.receive(&[0;4], &mut rx.bytes, 50).unwrap() };
        let received = u32::from_be_bytes(rx.bytes);
        assert_eq!(received, 0x6996_3c00 | id);
        assert_eq!(rx.before,0x5aa5_a55a); assert_eq!(rx.after,0xa55a_5aa5);
        let index = 136 + (id as usize - 1)*8;
        for (i,v) in [received,receipt.generation,receipt.irq_entries,receipt.elapsed_us,
            receipt.irq_body_max_us,receipt.irq_end_to_task_us,rx.before,rx.after].into_iter().enumerate() {
            put(index+i,v);
        }
        increment(131); put(129,3);
        unsafe { wait_level(true,3000); os::delay(100).unwrap(); }
    }
    put(129,4);
    loop { unsafe { os::delay(1000).unwrap(); } increment(132); }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn SPI0_IRQHandler() {
    unsafe { os::spi0::on_interrupt(); }
}

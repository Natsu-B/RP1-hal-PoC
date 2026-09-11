//! Bounded mixed R2 admission: SPI/UART payload and I2C NACK, not three-bus RX.
//! Each permanent owner reserves its own notification0. No runtime PLL/reset.
//! Telemetry96..127 SPI,128..159 I2C,160..191 UART;192..255 remains fault-owned.
use super::*;
use rp1_hal::{spi::Spi0Host, i2c::I2c1Host, uart::Uart0Tx};
static mut SPI: Option<Spi0Host> = None;
static mut I2C: Option<I2c1Host> = None;
static mut UART: Option<Uart0Tx> = None;

pub fn set_hosts(spi: Spi0Host, i2c: I2c1Host, uart: Uart0Tx) { unsafe {
    ptr::addr_of_mut!(SPI).write(Some(spi));
    ptr::addr_of_mut!(I2C).write(Some(i2c));
    ptr::addr_of_mut!(UART).write(Some(uart));
} }

#[repr(C)]
struct Buffer<const N: usize> { before: u32, bytes: [u8; N], after: u32 }
impl<const N: usize> Buffer<N> {
    fn new() -> Self { Self { before: 0x5aa5_a55a, bytes: [0xc3; N], after: 0xa55a_5aa5 } }
    fn canaries(&self, base: usize) { unsafe {
        let before = ptr::addr_of!(self.before).read_volatile();
        let after = ptr::addr_of!(self.after).read_volatile();
        put(base+14, before); put(base+15, after);
        assert!(before == 0x5aa5_a55a && after == 0xa55a_5aa5);
    } }
}
fn enter(base: usize, magic: [u8; 4], owner: u32) { unsafe {
    let (ipsr, control, psp): (u32, u32, u32);
    core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP",
        out(reg) ipsr, out(reg) control, out(reg) psp, options(nomem, nostack));
    assert!(ipsr == 0 && control & 3 == 2 && psp & 7 == 0);
    assert!(os::current_task().unwrap().unwrap().id() == owner);
    put(match owner { 6 => 46, 7 => 47, 8 => 51, _ => unreachable!() }, psp);
    put(base, u32::from_le_bytes(magic)); put(base+1, 1);
    os::delay(5000).unwrap(); // Existing host peer has time to admit the boot.
} }
fn complete(base: usize) -> ! {
    unsafe { core::arch::asm!("dsb sy", options(nostack)); }
    put(base+1, 4);
    loop { unsafe { os::delay(1000).unwrap(); } increment(base+3); }
}
fn maxima(base: usize, irq: u32, elapsed: u32, body: u32, wake: u32) {
    put(base+4, get(base+4).checked_add(irq).unwrap());
    for (offset, value) in [(5,elapsed), (6,body), (7,wake)] {
        put(base+offset, get(base+offset).max(value));
    }
}
fn miso_high() -> bool { unsafe {
    assert!((0x400d_004c as *const u32).read_volatile() == 0x80);
    assert!((0x400f_0028 as *const u32).read_volatile() & 0xff == 0xfb);
    let status = (0x400d_0048 as *const u32).read_volatile();
    assert!(status & (1 << 13) == 0);
    assert!((0x400e_0004 as *const u32).read_volatile() & (1 << 9) == 0);
    status & (1 << 17) != 0
} }
unsafe fn wait_miso(high: bool, ticks: u32) { unsafe {
    let deadline = os::tick().unwrap().wrapping_add(ticks);
    while miso_high() != high {
        assert!(os::deadline_remaining(os::tick().unwrap(), deadline).is_some());
        os::delay(1).unwrap();
    }
} }

pub unsafe extern "C" fn spi_worker(_: *mut c_void) { unsafe {
    const B: usize = 96;
    enter(B, *b"SPM1", 6);
    let mut driver = os::spi0::Driver::new(ptr::addr_of_mut!(SPI).replace(None).unwrap());
    for id in 1..=2u32 {
        wait_miso(true, 3000); put(B+1, 1);
        wait_miso(false, 30_000); put(B+1, 2);
        let mut buffer = Buffer::<4>::new();
        let started = raw_low(); if id == 1 { put(B+8, started); }
        let result = driver.receive(&[0; 4], &mut buffer.bytes, 50);
        let ended = raw_low(); put(B+9, ended);
        let payload = u32::from_be_bytes(ptr::addr_of!(buffer.bytes).read_volatile());
        put(B+11+id as usize, payload); buffer.canaries(B);
        if let Some(r) = driver.last_receipt() {
            maxima(B, r.irq_entries, r.elapsed_us, r.irq_body_max_us, r.irq_end_to_task_us);
            put(B+10, r.generation);
            for (i,v) in [started,ended,r.generation,payload,r.irq_entries,r.elapsed_us,
                r.irq_body_max_us,r.irq_end_to_task_us].into_iter().enumerate() {
                put(B+16+(id as usize-1)*8+i,v);
            }
        }
        if result.is_err() { put(B+11, 1); }
        let r = result.ok().unwrap(); // Retain receipt; avoid unused Debug formatting.
        assert!(r.generation == id && r.irq_entries > 0 && payload == 0x6996_3c00 | id);
        assert!(os::spi0::active_generation() == 0 && !os::spi0::cancel(id));
        increment(B+2); put(B+1, 3);
        wait_miso(true, 3000); os::delay(100).unwrap();
    }
    complete(B)
} }

pub unsafe extern "C" fn i2c_worker(_: *mut c_void) { unsafe {
    use rp1_hal::i2c_rx_state::Error as RxError;
    const B: usize = 128;
    enter(B, *b"ICM1", 7);
    let mut driver = os::i2c1::Driver::new(ptr::addr_of_mut!(I2C).replace(None).unwrap());
    put(B+17, u32::MAX); put(B+31, 0x2e);
    // ponytail: fixed256-request cohort; extend only after same-image admission.
    for generation in 1..=256 {
        os::delay(100).unwrap(); put(B+1, 2);
        let mut buffer = Buffer::<4>::new();
        let started = raw_low(); if generation == 1 { put(B+8, started); }
        if os::spi0::active_generation() != 0 { increment(B+22); }
        if os::uart0::active_generation() != 0 { increment(B+23); }
        let result = driver.receive(0x2e, &mut buffer.bytes[..2], 50);
        put(B+9, raw_low()); buffer.canaries(B);
        let r = driver.last_receipt().unwrap();
        maxima(B, r.irq_entries, r.elapsed_us, r.irq_body_max_us, r.irq_end_to_task_us);
        put(B+10, r.generation); put(B+12, r.first_fatal_causes); put(B+13, r.first_abort_source);
        put(B+16, get(B+16).max(r.cleanup_elapsed_us));
        put(B+17, get(B+17).min(r.quiet_samples));
        put(B+18, get(B+18).max(r.quiet_max_gap_us));
        put(B+19, r.received); put(B+20, r.discarded_after_failure);
        put(B+21, get(B+21).checked_add(r.higher_priority_wakes).unwrap());
        let payload = u32::from_be_bytes(ptr::addr_of!(buffer.bytes).read_volatile());
        put(B+24, payload);
        let expected = matches!(result, Err(os::i2c1::Error::Receive(
            RxError::Fatal { causes: 0x40, abort_source: 0x0080_0001 })));
        if !expected { increment(B+11); }
        assert!(expected && r.generation == generation && r.irq_entries > 0);
        assert!(r.received == 0 && r.first_fatal_causes == 0x40 && r.first_abort_source == 0x0080_0001);
        assert!(r.discarded_after_failure == 0 && payload == 0xc3c3_c3c3);
        assert!(os::i2c1::active_generation() == 0 && !os::i2c1::cancel(generation));
        increment(B+2); put(B+1, 3);
    }
    for (offset,address) in [(25,0xe000_e408u32), (26,0xe000_e413), (27,0xe000_e419)] {
        put(B+offset, u32::from((address as *const u8).read_volatile()));
    }
    put(B+28, (rp1_hal::addr::SPI0_BASE.wrapping_add(0x108) as *const u32).read_volatile());
    complete(B)
} }

pub unsafe extern "C" fn uart_worker(_: *mut c_void) { unsafe {
    const B: usize = 160;
    enter(B, *b"UAM1", 8);
    let mut driver = os::uart0::Driver::new(ptr::addr_of_mut!(UART).replace(None).unwrap());
    let mut buffer = Buffer::<20>::new();
    for (index,(ready,payload,ack)) in [
        (&b"RP1U0 RTOSREADY 0001\r\n"[..], &b"HOST2RP1 IRQ 0001\r\n"[..], &b"RP1U0 RTOSOK 0001\r\n"[..]),
        (&b"RP1U0 RTOSREADY 0002\r\n"[..], &b"HOST2RP1 IRQ 0002\r\n"[..], &b"RP1U0 RTOSOK 0002\r\n"[..]),
    ].into_iter().enumerate() {
        os::delay(100).unwrap(); buffer.bytes.fill(0xc3); put(B+1, 2);
        let started = raw_low(); if index == 0 { put(B+8, started); }
        // Host first services a fresh SPI READY while UART RX is armed, then
        // sends real UART bytes. This proves overlapping requests, not edges.
        let result = driver.exchange(ready, &mut buffer.bytes[..19], 5000);
        let ended = raw_low(); put(B+9, ended); buffer.canaries(B);
        let bytes = ptr::addr_of!(buffer.bytes).read_volatile();
        for n in 0..5 { put(B+25+n, u32::from_be_bytes(bytes[n*4..n*4+4].try_into().unwrap())); }
        if let Some(r) = driver.last_receipt() {
            maxima(B, r.irq_entries, r.elapsed_us, r.irq_body_max_us, r.irq_end_to_task_us);
            put(B+10, r.generation); put(B+12, r.ipsr); put(B+13, r.received);
            put(B+16, get(B+16).checked_add(r.higher_priority_wakes).unwrap());
            put(B+17, r.rsr_errors | r.overflow_bytes | r.residual_bytes | r.first_error_dr);
            for (i,v) in [started,ended,r.generation].into_iter().enumerate() { put(B+18+index*3+i,v); }
            put(B+24, get(B+24).max(r.cleanup_elapsed_us)); put(B+30, r.final_imsc); put(B+31, r.final_cr);
        }
        if result.is_err() { put(B+11, 1); }
        let r = result.ok().unwrap();
        assert!(r.generation == index as u32+1 && r.received == 19 && r.ipsr == 41 && r.irq_entries > 0);
        assert!(&bytes[..19] == payload && bytes[19] == 0xc3);
        assert!(get(B+17) == 0 && os::uart0::active_generation() == 0 && !os::uart0::cancel(r.generation));
        driver.write_all(ack, 100).ok().unwrap(); increment(B+2); put(B+1, 3);
    }
    for (index,address) in [(52,0x4001_8054), (53,0x4001_8058), (54,0x4001_8060), (55,0x4002_0010)] {
        put(index, (address as *const u32).read_volatile());
    }
    complete(B)
} }

#[unsafe(no_mangle)]
unsafe extern "C" fn SPI0_IRQHandler() { unsafe { os::spi0::on_interrupt(); } }
#[unsafe(no_mangle)]
unsafe extern "C" fn I2C1_IRQHandler() { unsafe { os::i2c1::on_interrupt(); } }
#[unsafe(no_mangle)]
unsafe extern "C" fn UART0_IRQHandler() { unsafe { os::uart0::on_interrupt(); } }

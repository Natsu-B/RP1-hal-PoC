//! Single-client BAR2 RPC. SCMI/channel0 and RawTimer are never written.
//! Words: magic, version, boot_count, request, response, boot_raw_lo/hi,
//! status, receive_lo/hi, publish_lo/hi, reserved[4]. Little endian.
use core::ptr;
const MAGIC: u32 = u32::from_le_bytes(*b"TS01");

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".timesync")]
pub static mut RP1_TIMESYNC: [u32; 16] = [0; 16];

fn read(i: usize) -> u32 { unsafe { ptr::addr_of!(RP1_TIMESYNC).cast::<u32>().add(i).read_volatile() } }
fn write(i: usize, v: u32) { unsafe { ptr::addr_of_mut!(RP1_TIMESYNC).cast::<u32>().add(i).write_volatile(v); } }
fn barrier() { unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); } }

// Same H/L/H contract as RawTimer, bounded like the existing boot time anchor.
fn now() -> Option<u64> {
    for _ in 0..4 {
        let high = unsafe { (0x400a_c024 as *const u32).read_volatile() };
        let low = unsafe { (0x400a_c028 as *const u32).read_volatile() };
        let after = unsafe { (0x400a_c024 as *const u32).read_volatile() };
        if high == after { return Some((u64::from(high) << 32) | u64::from(low)); }
    }
    None
}
fn stamp(i: usize, t: u64) { write(i, t as u32); write(i + 1, (t >> 32) as u32); }

/// Sole cold startup before scheduler/Linux. NOLOAD survives an M3 reset;
/// ELF reload may clear it, so clients also bind Linux boot ID and boot_raw.
pub unsafe fn prepare() {
    let previous = if read(0) == MAGIC && read(1) == 1 { read(2) } else { 0 };
    write(0, 0); barrier();
    for i in 1..16 { write(i, 0); }
    let Some(t) = now() else { return; };
    let Some(epoch) = previous.checked_add(1) else { return; };
    write(1, 1); write(2, epoch); stamp(5, t);
    barrier(); write(0, MAGIC); barrier();
}

/// One request per existing 5-tick producer pass. One Linux owner writes only
/// request word3, then leaves it stable until response word4 matches.
pub unsafe fn service() {
    let seq = read(3);
    if seq == 0 || seq == read(4) || read(0) != MAGIC { return; }
    barrier();
    let r2 = now();
    write(7, 0);
    if let Some(t) = r2 { stamp(8, t); } else { write(7, 1); }
    barrier();
    if let Some(t) = now() { stamp(10, t); } else { write(7, 2); }
    barrier(); write(4, seq); barrier();
}

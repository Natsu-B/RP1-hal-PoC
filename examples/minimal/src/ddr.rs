//! Bounded commissioning RPC; selector observations run on proc0 with IRQs masked.
//! No other task can access DBI until its original selector is restored.
use core::ptr;
use rp1_hal::pcie_outbound::{ReservedDdrWindow, REGION0};
const MAGIC: u32 = u32::from_le_bytes(*b"DDR1");
const SELECTOR: usize = 0x4010_8000;
const DBI: usize = 0x4010_9000;
const OFFSETS: [usize; 8] = [0, 4, 8, 12, 16, 20, 24, 32];
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".ddr")]
pub static mut RP1_DDR: [u32; 64] = [0; 64];
fn get(i: usize) -> u32 { unsafe { ptr::addr_of!(RP1_DDR).cast::<u32>().add(i).read_volatile() } }
fn put(i: usize, v: u32) { unsafe { ptr::addr_of_mut!(RP1_DDR).cast::<u32>().add(i).write_volatile(v); } }
fn read(a: usize) -> u32 { unsafe { (a as *const u32).read_volatile() } }
fn write(a: usize, v: u32) { unsafe { (a as *mut u32).write_volatile(v); } }
fn barrier() { unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); } }
fn allowed_selector(s: u32) -> bool { matches!(s, 0x23 | 0x63 | 0x03 | 0x43 | 0x83 | 0xc3) }
fn region() -> [u32; 8] { OFFSETS.map(|off| read(DBI + off)) }
fn desired(candidate: u32) -> Option<[u32; 8]> {
    // Candidate 1 was rejected by the real read; never execute it again.
    if candidate == 2 { Some(REGION0) } else { None }
}
fn program(values: [u32; 8]) {
    write(DBI + 4, 0); barrier();
    for i in [2, 3, 4, 5, 6, 0] { write(DBI + OFFSETS[i], values[i]); }
    barrier(); write(DBI + 4, values[1]); barrier();
}
pub unsafe fn prepare() {
    for i in 0..64 { put(i, 0); }
    put(1, 2); barrier(); put(0, MAGIC); barrier();
}
pub unsafe fn service() {
    let seq = get(2);
    if seq == 0 || seq == get(3) || get(0) != MAGIC { return; }
    barrier();
    let (op, candidate) = (get(4), get(5));
    put(6, 1);
    if (op == 1 && allowed_selector(candidate)) || (op == 2 && desired(candidate).is_some()) || matches!(op, 3 | 4 | 5) {
        // Op1: REVERSIBLE_SELECTOR_OBSERVATION. Op2/5: region programming/restore.
        // proc1 is held by this build; the DBI monitor is a proc0 task.
        let mask: u32;
        unsafe { core::arch::asm!("mrs {0}, PRIMASK", "cpsid i", out(reg) mask, options(nostack)); }
        let saved = read(SELECTOR);
        put(7, saved);
        if saved == 0 {
            if op == 1 {
                write(SELECTOR, candidate); barrier();
                put(8, read(SELECTOR));
                for (view, base) in [DBI, DBI + 0x100].into_iter().enumerate() {
                    for (i, offset) in OFFSETS.into_iter().enumerate() { put(16 + view * 8 + i, read(base + offset)); }
                }
                put(6, u32::from(get(8) != candidate));
            } else {
                let mut others_disabled = true;
                for s in [0x43, 0x83, 0xc3] {
                    write(SELECTOR, s); barrier();
                    others_disabled &= read(SELECTOR) == s && read(DBI + 4) & 0x8000_0000 == 0;
                }
                write(SELECTOR, 3); barrier(); put(8, read(SELECTOR));
                let before = region();
                if get(8) == 3 && others_disabled {
                    if op == 2 && before[1] & 0x8000_0000 == 0 && get(10) == 0 {
                        for (i, v) in before.into_iter().enumerate() { put(48 + i, v); }
                        let values = desired(candidate).unwrap();
                        program(values);
                        for _ in 0..100 { if read(DBI + 4) & 0x8000_0000 != 0 { break; } }
                        let after = region();
                        for (i, v) in after.into_iter().enumerate() { put(16 + i, v); }
                        if after == values { put(10, candidate); put(6, 0); }
                        else { program(before); put(6, if region() == before { 3 } else { 5 }); }
                    } else if matches!(op, 3 | 4) && Some(before) == desired(get(10)) {
                        put(6, 0); // Data load happens after selector/IRQ restoration.
                    } else if op == 5 && Some(before) == desired(get(10)) {
                        let original = core::array::from_fn(|i| get(48 + i));
                        program(original);
                        if region() == original { put(10, 0); put(6, 0); }
                    }
                }
            }
            write(SELECTOR, saved); barrier();
            put(9, read(SELECTOR));
            if get(9) != saved { put(6, 2); }
        }
        unsafe { core::arch::asm!("msr PRIMASK, {0}", in(reg) mask, options(nostack)); }
    }
    if matches!(op, 3 | 4) && get(6) == 0 {
        let mut window = unsafe { ReservedDdrWindow::from_verified_region0() };
        put(11, seq); barrier(); // externally visible before the potentially stalled load
        let publish = window.read32(40).unwrap(); barrier();
        if publish == seq {
            let mut request = [0u32; 10];
            window.read_words(0, &mut request).unwrap();
            for (i, v) in request.into_iter().enumerate() { put(32 + i, v); }
            barrier(); put(42, window.read32(40).unwrap()); barrier();
            let checksum = request[4..8].iter().fold(0u32, |sum, v| sum.wrapping_add(*v));
            put(12, checksum);
            if get(42) != seq || request[0] != 0x51a7_beef || request[1] != 0x3144_5244 || request[2] != seq || request[3] != 16 || request[8] != checksum || request[9] != 0xcafe_72a9 { put(6, 4); }
            if op == 4 && get(6) == 0 {
                let payload = core::array::from_fn::<_, 4, _>(|i| request[4+i].rotate_left(7) ^ seq ^ 0xa5c3_791d);
                let sum = payload.iter().fold(0u32, |sum, v| sum.wrapping_add(*v));
                window.write_words(0x104, &[0x3252_4444, seq, 16, payload[0], payload[1], payload[2], payload[3], sum]).unwrap();
                barrier(); window.write32(0x128, seq).unwrap();
                let completion = window.complete_posted_write().unwrap();
                put(14, completion); put(15, seq);
                if completion != request[0] { put(6, 6); }
            }
        } else { put(42, publish); put(6, 4); }
        put(13, seq); barrier();
    }
    barrier(); put(3, seq); barrier();
}
#[cfg(test)]
mod tests {
    #[test] fn derived_selectors_only() {
        for n in 0..512 { assert_eq!(super::allowed_selector(n), [3, 0x23, 0x43, 0x63, 0x83, 0xc3].contains(&n)); }
        assert!(super::desired(0).is_none());
        assert!(super::desired(1).is_none());
        for candidate in [2] {
            let v = super::desired(candidate).unwrap();
            assert_eq!(v[4] - v[2] + 1, 65536);
            assert_eq!((u64::from(v[6]) << 32) | u64::from(v[5]), 0x10_2100_0000);
        }
    }
}

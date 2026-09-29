//! Fixed-window hardware bootstrap for standard remoteproc attachment.
//! No arbitrary-address mapping API. Only the DT-owned 0x21100000 reservation.
use core::ptr;
use crate::pcie_outbound::{barrier, REGION0};
const MAGIC: u32 = 0x3150_4d41;
const LOCAL: usize = 0x8200_0000;
const TABLE: usize = LOCAL + 0xc000;
const SELECTOR: usize = 0x4010_8000;
const DBI: usize = 0x4010_9000;
const OFFSETS: [usize; 8] = [0,4,8,12,16,20,24,32];
const REGION1: [u32; 8] = [0,0x8000_0000,0x0200_0000,0x80,0x0200_ffff,0x2110_0000,0x10,0];
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".openamp_control")]
pub static mut RP1_OPENAMP_CONTROL: [u32; 16] = [0;16];
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".openamp_telemetry")]
pub static mut RP1_OPENAMP_TELEMETRY: [u32; 64] = [0;64];
fn read(a: usize) -> u32 { unsafe { (a as *const u32).read_volatile() } }
fn write(a: usize, v: u32) { unsafe { (a as *mut u32).write_volatile(v); } }
fn get(i: usize) -> u32 { read(ptr::addr_of!(RP1_OPENAMP_CONTROL) as usize + i*4) }
fn put(i: usize, v: u32) { write(ptr::addr_of_mut!(RP1_OPENAMP_CONTROL) as usize + i*4,v); }
fn tele(i: usize, v: u32) { write(ptr::addr_of_mut!(RP1_OPENAMP_TELEMETRY) as usize + i*4,v); }
fn region() -> [u32;8] { OFFSETS.map(|off| read(DBI+off)) }
fn program(values: [u32;8]) {
    write(DBI+4,0); barrier();
    for i in [2,3,4,5,6,0] { write(DBI+OFFSETS[i],values[i]); }
    barrier(); write(DBI+4,values[1]); barrier();
}
const RESOURCE: [u32;22] = [1,1,0,0,20, 3,7,0,1,0,0,0x200,
    0x82000000,16,16,0,0, 0x82004000,16,16,1,0];

/// # Safety
/// Cold startup before Linux probes; only proc0 owns this transport.
pub unsafe fn prepare(boot_epoch: u64) {
    #[cfg(feature = "openamp-rpmsg")]
    unsafe { rp1_openamp_cold_reset(); }
    for i in 0..16 { put(i,0); }
    for i in 0..64 { tele(i,0); }
    put(0,1); put(1,2); put(2,TABLE as u32);
    put(12,boot_epoch as u32); put(13,(boot_epoch>>32) as u32);
    // Record MPU TYPE/CTRL and system CCR; attributes must be reviewed before IO.
    tele(60,read(0xe000_ed90)); tele(61,read(0xe000_ed94)); tele(62,read(0xe000_ed14));
    barrier(); put(6,MAGIC); barrier();
}

/// # Safety
/// One proc0 caller, Linux reservations/root mapping match the sealed DT.
/// Command 1 is issued only after Linux has admitted its reserved-memory map.
pub unsafe fn service() {
    put(7,get(7).wrapping_add(1));
    #[cfg(feature = "openamp-rpmsg")]
    if matches!(get(11),1|3) {
        let rc=unsafe { rp1_openamp_poll() };
        if rc!=0 { put(11,0); put(5,rc as u32); }
    }
    let command = get(3);
    if command == 0 || command == get(4) { return; }
    put(5,1); barrier();
    if command == 1 && get(8) == 0 {
        let mask: u32;
        unsafe { core::arch::asm!("mrs {0}, PRIMASK", "cpsid i", out(reg) mask, options(nostack)); }
        let saved = read(SELECTOR);
        let mut valid = saved == 0;
        let mut before = [0;8];
        if valid {
            for (n,s) in [3,0x43,0x83,0xc3].into_iter().enumerate() {
                write(SELECTOR,s); barrier();
                let values=region();
                for (i,v) in values.into_iter().enumerate() { tele(n*8+i,v); }
                valid &= read(SELECTOR)==s && if n==0 { values==REGION0 } else { values[1]&0x80000000==0 };
                if n==1 { before=values; }
            }
            if valid {
                write(SELECTOR,0x43); barrier(); program(REGION1);
                valid = region()==REGION1;
                if !valid { program(before); }
            }
            write(SELECTOR,saved); barrier(); valid &= read(SELECTOR)==saved;
        }
        unsafe { core::arch::asm!("msr PRIMASK, {0}", in(reg) mask, options(nostack)); }
        if valid {
            // Only our reserved table area; queues and buffers remain untouched.
            for i in 0..256 { write(TABLE+i*4, RESOURCE.get(i).copied().unwrap_or(0)); }
            barrier(); let receipt=read(TABLE); barrier();
            tele(32,receipt); put(8,1); put(5,u32::from(receipt!=1));
        }
    } else if command == 2 && get(8)==1 {
        // Observe the Linux-negotiated table only; no descriptor dereference yet.
        for i in 0..22 { tele(32+i,read(TABLE+i*4)); }
        barrier(); put(5,0);
    }
    #[cfg(feature = "openamp-rpmsg")]
    if command==3 && get(8)==1 && get(11)==0 && read(0xe000_ed94)==0 {
        // Descriptor 0 was provided by Linux; accept only our fixed pool.
        let base=read(LOCAL)&!0x3fff;
        if read(LOCAL+4)==0 && matches!(base,0x21108000|0x82008000) {
            let rc=unsafe { rp1_openamp_start(base) };
            put(5,rc as u32); put(11,u32::from(rc==0));
        }
    }
    #[cfg(feature = "openamp-rpmsg")]
    if command==4 && get(8)==1 {
        unsafe { rp1_openamp_quiesce(); }
        put(11,0); put(5,0);
    }
    // Bounded commissioning: suppress only queue0 host notification so its
    // finite TX buffers can be exhausted; SCMI and queue1 IRQs remain enabled.
    #[cfg(feature = "openamp-rpmsg")]
    if command==7 && get(11)==1 { put(11,3); put(5,0); }
    #[cfg(feature = "openamp-rpmsg")]
    if command==8 && get(11)==3 {
        put(11,1); rp1_openamp_notify(0); put(5,0);
    }
    barrier(); put(4,command); barrier();
}

#[cfg(feature = "openamp-rpmsg")]
unsafe extern "C" {
    fn rp1_openamp_cold_reset();
    fn rp1_openamp_quiesce();
    fn rp1_openamp_start(buffer_dma: u32) -> i32;
    fn rp1_openamp_poll() -> i32;
}

#[cfg(feature = "openamp-rpmsg")]
#[unsafe(no_mangle)]
pub extern "C" fn rp1_openamp_complete() {
    barrier(); let _=read(TABLE); barrier();
}
#[cfg(feature = "openamp-rpmsg")]
#[unsafe(no_mangle)]
pub extern "C" fn rp1_openamp_notify(queue: u32) {
    assert!(queue<2);
    rp1_openamp_complete();
    if queue==0 && get(11)==3 { return; }
    write(0x4000_a00c,1<<(queue+1)); barrier();
    put(14,get(14).wrapping_add(1));
}
#[cfg(feature = "openamp-rpmsg")]
#[unsafe(no_mangle)]
pub extern "C" fn rp1_openamp_delay(usec: u32) {
    assert!(usec<=1000);
    let start=read(0x400a_c028);
    while read(0x400a_c028).wrapping_sub(start)<usec { core::hint::spin_loop(); }
}

/// Demultiplex only OpenAMP channels in the shared proc0 mailbox ISR.
/// Queue work stays in the sole transport task; channel0 remains SCMI-owned.
pub fn ack_kicks() -> u32 {
    let events=read(0x4000_8008);
    let owned=events & 0xe;
    if owned != 0 {
        write(0x4000_b008,owned); barrier();
        put(9,get(9).wrapping_add(1)); put(10,owned);
    }
    events
}

#[cfg(test)]
mod tests {
    #[test] fn bounded_contract() {
        use super::*;
        assert_eq!(REGION1[4]-REGION1[2]+1,65536);
        assert_eq!(RESOURCE[4],20);
        assert_eq!(RESOURCE[11],2<<8);
        assert_eq!((RESOURCE[12],RESOURCE[17]),(0x82000000,0x82004000));
        assert_eq!((RESOURCE[13],RESOURCE[14]),(16,16));
    }
}

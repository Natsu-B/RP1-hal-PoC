//! Finite software-force commissioning, not physical sensor acquisition.
use core::{mem::MaybeUninit, ptr};
use rp1_abi::amp::Generation;
use rp1_hal::gpio::{ConfiguredPin, Input, InterruptDestination, InterruptInput, InterruptKind, InterruptToken};

const BIT: u32 = 1 << 9;
const LIMIT: u32 = 100_000;
#[unsafe(no_mangle)]
#[unsafe(link_section=".amp_proc1.irq_status")]
static mut RP1_AMP_GPIO_IRQ_STATUS: [u32;32] = [0;32];
#[unsafe(link_section=".amp_proc1.irq_handle")]
static mut HANDOFF: Option<(ConfiguredPin<9,Input>,InterruptToken<9>)> = None;
#[unsafe(link_section=".amp_proc1.irq_handle")]
static mut INPUT: MaybeUninit<InterruptInput<9>> = MaybeUninit::uninit();
#[unsafe(link_section=".amp_proc1.irq_handle")]
static mut LIVE: bool = false;
#[used]
#[unsafe(link_section=".amp_proc1.irq_contract")]
static CONTRACT: [u32;6] = [u32::from_le_bytes(*b"G1F2"),2,9,0,LIMIT,200];

#[inline(always)]
fn read(address:usize)->u32 { unsafe {(address as *const u32).read_volatile()} }
#[inline(always)]
fn write(address:usize,value:u32) { unsafe {(address as *mut u32).write_volatile(value);} }
#[inline(always)]
fn get(i:usize)->u32 { unsafe {ptr::addr_of!(RP1_AMP_GPIO_IRQ_STATUS).cast::<u32>().add(i).read_volatile()} }
#[inline(always)]
fn put(i:usize,value:u32) { unsafe {ptr::addr_of_mut!(RP1_AMP_GPIO_IRQ_STATUS).cast::<u32>().add(i).write_volatile(value);} }
#[inline(always)]
fn dsb() { unsafe {core::arch::asm!("dsb sy",options(nostack,preserves_flags));} }

// C0 initializes once before release. After release only C1 touches this handle.
#[inline(never)]
#[unsafe(link_section=".amp_proc1.irq_text")]
pub(crate) fn handoff_input(pin:ConfiguredPin<9,Input>,grant:InterruptToken<9>) {
    unsafe {
        ptr::addr_of_mut!(RP1_AMP_GPIO_IRQ_STATUS).write([0;32]);
        ptr::addr_of_mut!(HANDOFF).write(Some((pin,grant)));
        ptr::addr_of_mut!(LIVE).write(false);
    }
    dsb();
}

#[inline(never)]
#[unsafe(link_section=".amp_proc1.irq_text")]
pub(super) fn start(generation:Generation)->bool {
    // Candidate IRQ0 only. Never enable a guessed second route after failure.
    if read(0xe00f_f01c)!=1 || read(0xe000_e100)!=0 || read(0xe000_e104)!=0
        || read(0xe000_e200)&1!=0 || read(0x400d_0110)!=0 {
        return false;
    }
    // Unassigned pending lines remain masked and are never acknowledged here.
    let Some((pin,grant))=(unsafe {(&mut *ptr::addr_of_mut!(HANDOFF)).take()}) else {return false;};
    if !pin.is_high() || read(0x400e_0004)&BIT!=0 {return false;}
    let Ok(input)=pin.into_interrupt(grant,InterruptKind::EdgeLow) else {return false;};
    // Verify the generated destination; no Proc0 grant may masquerade as C1.
    if read(0x400d_0110)!=BIT {drop(input);return false;}
    unsafe {ptr::addr_of_mut!(INPUT).write(MaybeUninit::new(input));ptr::addr_of_mut!(LIVE).write(true);}
    for (i,value) in [(0,u32::from_le_bytes(*b"G1F2")),(1,2),(2,1),
        (3,generation.boot as u32),(4,(generation.boot>>32) as u32),
        (5,generation.session as u32),(6,(generation.session>>32) as u32),(20,u32::MAX)] {put(i,value);}
    dsb();write(0xe000_e280,1);write(0xe000_e100,1);dsb();
    unsafe {core::arch::asm!("cpsie i","isb sy",options(nostack,preserves_flags));}
    true
}

#[inline(never)]
#[unsafe(link_section=".amp_proc1.irq_text")]
pub(super) fn force()->bool {
    if get(9)!=get(7) || get(10)!=0 || get(11)!=0 || get(22)!=0 || get(23)!=0 {return false;}
    if get(7)>=LIMIT {return false;}
    let Some(timestamp)=super::hardware::now() else {return false;};
    let epoch=get(30);put(30,epoch.wrapping_add(1));
    put(18,timestamp as u32);put(19,(timestamp>>32) as u32);put(7,get(7)+1);
    dsb();put(30,epoch.wrapping_add(2));dsb();
    write(0x400d_2114,BIT);dsb();
    // A failure is handled on the next local iteration, never by RPMsg waiting.
    true
}

#[inline(never)]
#[unsafe(link_section=".amp_proc1.irq_text")]
pub(super) fn stop()->bool {
    // Drain at most one already-issued software request while its handler is
    // still enabled. No RPMsg/DDR wait, re-arm, reset or alternate IRQ route.
    if unsafe {ptr::addr_of!(LIVE).read()} {
        for _ in 0..128 {if get(9)==get(7) {break;}core::hint::spin_loop();}
    }
    unsafe {core::arch::asm!("cpsid i",options(nostack,preserves_flags));}
    write(0xe000_e180,1);dsb();
    if unsafe {ptr::addr_of!(LIVE).read()} {
        let input=unsafe {ptr::addr_of!(INPUT).read().assume_init()};
        let _=input.into_input();
        unsafe {ptr::addr_of_mut!(LIVE).write(false);}
        put(24,get(24).saturating_add(1));
    }
    write(0x400d_3114,BIT);dsb();write(0xe000_e280,1);dsb();
    let enable=read(0xe000_e100)|read(0xe000_e104);
    let pending=read(0xe000_e200);let pending1=read(0xe000_e204);
    let source=read(0x400d_0118)&BIT;let events=read(0x400d_004c)&(0xff<<20);
    put(25,enable);put(26,pending);put(27,source);put(28,events);put(29,pending1);put(2,2);dsb();
    enable==0 && pending&1==0 && source==0 && events==0
        && get(9)==get(7) && get(10)==0 && get(11)==0 && get(22)==0 && get(23)==0
}

unsafe extern "C" { pub(crate) fn AmpProc1GpioIrq(); }
core::arch::global_asm!(
    ".syntax unified", ".thumb",
    ".section .amp_proc1.irq_text,\"ax\",%progbits", ".balign 4",
    ".global AmpProc1GpioIrq", ".type AmpProc1GpioIrq,%function", ".thumb_func",
    "AmpProc1GpioIrq:", "cpsid i",
    "mrs r1,IPSR", "cmp r1,#16", "bne AmpProc1Fault",
    "ldr r0,=0xe00ff01c", "ldr r0,[r0]", "cmp r0,#1", "bne AmpProc1Fault",
    "ldr r0,=RP1_AMP_GPIO_IRQ_STATUS", "ldr r1,[r0,#124]", "adds r1,#1", "str r1,[r0,#124]", "dmb sy",
    "movs r1,#16", "str r1,[r0,#48]",
    "ldr r1,=0x400d0118", "ldr r2,[r1]", "str r2,[r0,#56]",
    "cmp.w r2,#512", "bne 8f",
    "ldr r1,=0x400d0114", "ldr r2,[r1]", "str r2,[r0,#60]", "tst.w r2,#512", "beq 7f",
    "ldr r1,[r0,#120]", "tst r1,#1", "bne 8f",
    "ldr r1,=0x400d0048", "ldr r2,[r1]", "str r2,[r0,#52]",
    "tst.w r2,#1048576", "bne 7f",
    // Four high-low-high attempts. R0-R3/R12 are already hardware-stacked.
    "ldr r1,=0x400ac024", "mov.w r12,#4",
    "1:", "ldr r2,[r1]", "ldr r3,[r1,#4]", "ldr r0,[r1]", "cmp r0,r2", "beq 2f",
    "subs.w r12,r12,#1", "bne 1b", "b 6f",
    "2:", "ldr r0,=RP1_AMP_GPIO_IRQ_STATUS", "str r3,[r0,#64]", "str r2,[r0,#68]",
    "ldr r1,[r0,#72]", "subs r3,r3,r1", "ldr r1,[r0,#76]", "sbcs r2,r2,r1", "cmp r2,#0", "bne 6f",
    "ldr r1,[r0,#80]", "cmp r3,r1", "it lo", "strlo r3,[r0,#80]",
    "ldr r1,[r0,#84]", "cmp r3,r1", "it hi", "strhi r3,[r0,#84]",
    "ldr r1,[r0,#32]", "adds r1,#1", "str r1,[r0,#32]",
    "ldr r1,[r0,#36]", "adds r1,#1", "str r1,[r0,#36]",
    // Only the owned force bit and owned pin edge latch are cleared.
    "ldr r1,=0x400d3114", "mov.w r2,#512", "str r2,[r1]",
    "ldr r1,=0x400d204c", "mov.w r2,#268435456", "str r2,[r1]", "dsb sy",
    "ldr r1,=0x400d0118", "ldr r2,[r1]", "str r2,[r0,#92]", "cmp r2,#0", "bne AmpProc1Fault",
    "dmb sy", "ldr r1,[r0,#124]", "adds r1,#1", "str r1,[r0,#124]",
    "cpsie i", "isb sy", "bx lr",
    "6:", "ldr r0,=RP1_AMP_GPIO_IRQ_STATUS", "movs r1,#1", "str r1,[r0,#88]", "b AmpProc1Fault",
    "7:", "ldr r1,[r0,#40]", "adds r1,#1", "str r1,[r0,#40]", "b AmpProc1Fault",
    "8:", "ldr r1,[r0,#44]", "adds r1,#1", "str r1,[r0,#44]", "b AmpProc1Fault",
    ".size AmpProc1GpioIrq,.-AmpProc1GpioIrq",
    ".ltorg",
);

// Deliberately no physical sample flags: this only commissions a candidate route.
const _: () = assert!(core::mem::size_of::<InterruptDestination>()==1);

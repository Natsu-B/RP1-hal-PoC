//! AZ: one warm slot7/id8/priority5/512-word owner, SPI -> UART -> I2C, twice.
//! Host check: rustc --edition=2024 --test warm_combined.rs -o /tmp/warm-combined-test
//! 124=AZ01,125=phase | six checked-completion bits<<8; owner HWM160 untouched.
//! Six seven-word receipts occupy126..159,161..168, in execution order:
//! header(A5/kind/length/generation), payload FNV1a, IRQs, IPSR|wake-hints<<8,
//! elapsed-us, cleanup-us (SPI IRQmax), quiet-samples|gap-us<<16 (SPI wake-us).
//! 169..171=SPI/UART/I2C direct IRQ totals;172..174=their last actual IPSRs.
//! 175..177=owner IPSR/CONTROL/PSP;178/179=13/29ms pulse elapsed-us;
//! 180..182=admitted SPI CTRLR0/I2C CON/UART CR;183=failing phase.
//! Full raw receipts/payloads are checked locally, not externally readable.
//! BC01 feature replaces finite receipts with three rolling eight-word receipts
//! 126..149: full generation followed by the seven AZ fields (header has no g).
//! 150=owner seqlock,151=g,152=operation,153=tick deadline,154=checked count,
//! 155=last checked tick,156..158=checked IRQ sums,159=type8 handback.
//! 161..163=monitor-only eligibility/tick/owner sequence;164/165=start/end tick.
//! Operations:0 checked quiet boundary,1 SPI HIGH wait,2 SPI LOW wait,3 SPI RX,
//! 4 UART exchange,5 I2C RX,6 UART ACK,7 type8 handoff,8 commissioning complete.
//! The commissioning cap is eight, NOT a persistent/30-minute acceptance claim.
const PERSISTENT:bool=cfg!(feature="freertos-r3-watchdog-warm-persistent");
const GENERATIONS:u32=if PERSISTENT {8}else{2};
const BEFORE: u32 = 0x5aa5_a55a;
const AFTER: u32 = 0xa55a_5aa5;
const IPSRS: [u32; 3] = [35,41,24];
const LIMITS: [u32; 3] = [5,84,132];
const UART_RX: [&[u8;19];2] = [b"HOST2RP1 IRQ 0001\r\n",b"HOST2RP1 IRQ 0002\r\n"];
// Fixed peer payload FNV1a values, checked against every generated payload below.
#[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
const DIGESTS: [[u32;2];3] = [[0x6db9_86cd,0x6ab9_8214],
    [0x9e17_d962,0x9990_6dc7],[0x41eb_6616,0x7553_f956]];
fn word(slot: usize, field: usize) -> usize { let i=126+slot*7+field; i+usize::from(i>=160) }
fn length(kind: usize, generation: u32) -> usize { match kind {0=>4,1=>19,_=>if generation&1==1 {2}else{31}} }
fn digit(g:u32,index:usize)->u8 {b'0'+((g/[1000,100,10,1][index])%10) as u8}
fn token<const N:usize>(mut text:[u8;N],g:u32)->[u8;N] {
    for i in 0..4 {text[N-6+i]=digit(g,i);} text
}
#[inline(never)]
fn byte(kind: usize, generation: u32, index: usize) -> u8 {
    if index>=length(kind,generation) {return 0xc3;}
    match kind {
        0=>[0x69,0x96,0x3c,((generation-1)%2+1) as u8][index],
        1=>if PERSISTENT && (13..17).contains(&index) {digit(generation,index-13)}
            else {UART_RX[if PERSISTENT {0}else{generation as usize-1}][index]},
        _=>match index {0=>((generation-1) as u8)^0x31,1=>(((generation-1)>>8) as u8)^0x4e,
            _=>(0xb4+index as u32*0x1d+(generation-1)*7) as u8},
    }
}
#[inline(never)]
fn digest(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5,|hash,&b| (hash^u32::from(b)).wrapping_mul(0x0100_0193))
}
#[repr(C)]
#[derive(Clone,Copy,PartialEq,Eq)]
struct Buffer { before:u32, bytes:[u8;32], after:u32 }
impl Buffer { fn new()->Self {Self {before:BEFORE,bytes:[0xc3;32],after:AFTER}} }
#[inline(never)]
fn buffer_ok(kind:usize,g:u32,b:&Buffer)->bool {
    kind<3 && (1..=GENERATIONS).contains(&g) && b.before==BEFORE && b.after==AFTER
        && b.bytes.iter().enumerate().all(|(i,&v)| v==byte(kind,g,i))
}
// Full local receipt: generation,len,IRQs,IPSR,elapsed,IRQmax,wake-us,wake-hints,
// error-OR,cleanup-us,quiet-samples,max-gap-us,kind-specific statusA/B/C.
#[inline(never)]
fn receipt_ok(k:usize,g:u32,r:&[u32;15])->bool {
    k<3 && (1..=GENERATIONS).contains(&g) && r[0]==g && r[1]==length(k,g) as u32
        && r[2]>0 && r[2]<=LIMITS[k] && r[3]==IPSRS[k] && r[4]>0
        && r[5]<=r[4] && r[6]<=r[4] && r[7]<=r[2] && r[8]==0
        && match k {
            0=>r[7]==0 && matches!(r[12],4|6) && r[13]==0x0007_0000 && r[14]==1,
            1=>r[7]>0 && r[9]>=8000 && r[10]>=4 && r[10]<=0xffff && r[11]==0
                && r[12]&0x50!=0 && (r[12]>>16)&0x50!=0 && (r[12]>>16)&!0x50==0
                && r[13]&0x301==0x101 && r[14]&0x18==0x10,
            _=>r[9]>=4000 && r[10]>=2 && r[10]<=0xffff && r[11]<10_000,
        }
}
fn compact(k:usize,r:&[u32;15],payload:u32)->[u32;7] {
    [0xa500_0000|((k as u32)<<16)|(r[1]<<8)|if PERSISTENT {0}else{r[0]},payload,r[2],r[3]|(r[7]<<8),r[4],
        if k==0 {r[5]} else {r[9]},if k==0 {r[6]} else {r[10]|(r[11]<<16)}]
}
#[inline(never)]
fn stored_ok(k:usize,g:u32,w:[u32;7])->bool {
    k<3 && (1..=GENERATIONS).contains(&g) && w[0]==0xa500_0000|((k as u32)<<16)|((length(k,g) as u32)<<8)|if PERSISTENT {0}else{g}
        && w[1]==expected_digest(k,g)
        && w[2]>0 && w[2]<=LIMITS[k] && w[3]&0xff==IPSRS[k] && w[3]>>8<=w[2] && w[4]>0
        && match k {0=>w[3]>>8==0 && w[5]<=w[4] && w[6]<=w[4],
            1=>w[3]>>8>0 && w[5]>=8000 && w[6]>=4 && w[6]<=0xffff,
            _=>w[5]>=4000 && w[6]&0xffff>=2 && w[6]>>16<10_000}
}
fn expected_digest(k:usize,g:u32)->u32 {
    #[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
    {DIGESTS[k][g as usize-1]}
    #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
    {(0..length(k,g)).fold(0x811c_9dc5,|h,i|(h^u32::from(byte(k,g,i))).wrapping_mul(0x0100_0193))}
}

#[cfg(feature="freertos-r3-watchdog-warm-persistent")]
fn state_ok(now:u32,s:[u32;5],active:[u32;3])->bool {
    let [g,op,deadline,n,last]=s;
    if !(1..=GENERATIONS).contains(&g) || op>8 || n>GENERATIONS*3 {return false;}
    let offset=n.wrapping_sub((g-1)*3);
    // Same count contract as the branch model; compact masks avoid duplicate
    // comparison code. The independent test checks both formulations.
    const COUNTS:[u8;9]=[14,3,1,1,2,4,8,8,8];
    const BUDGET:[u16;9]=[1000,3000,30_000,100,5100,100,200,20_000,0];
    let count_ok=offset<4 && (COUNTS[op as usize]>>offset)&1!=0
        && (op!=7 || g==2) && (op!=8 || g==GENERATIONS);
    let bus=op.wrapping_sub(3);
    let freshness=if g==3 && n==6 {55_000}else{35_000};
    count_ok && active.iter().enumerate().all(|(k,&a)|a==0 || bus==k as u32 && a==g)
        && (op==8 || deadline.wrapping_sub(now).wrapping_sub(1)<u32::from(BUDGET[op as usize]))
        && (op==8 || now.wrapping_sub(last)<=freshness)
}
#[cfg(feature="freertos-r3-watchdog-warm-persistent")]
fn coherent(before:u32,after:u32)->bool {before&1==0 && before==after}

#[cfg(target_arch="arm")]
mod target {
    use super::*;
    use crate::freertos_r1::{get,put,raw_low,MARKER,kernel_restart};
    use core::{ffi::c_void,mem::MaybeUninit,ptr};
    use rp1_freertos as os;
    use rp1_hal::{gpio::{ConfiguredPin,Output},spi::Spi0Host,i2c::{self,I2c1Host},uart::Uart0Tx};
    static mut HOSTS: Option<(Spi0Host,I2c1Host,Uart0Tx)> = None;
    static mut STATE:u32=0; // Fresh -> set -> permanently taken; never rearm.
    static mut READY:u32=0;
    // Only this worker initializes these once, before any IRQ can reference them.
    // I2C/UART stay at stable addresses and are borrowed only shared: their own
    // UnsafeCells serialize task/ISR accesses. No enclosing &mut Driver exists.
    static mut SPI:MaybeUninit<os::spi0::Driver>=MaybeUninit::uninit();
    static mut I2C:MaybeUninit<os::i2c1::Driver>=MaybeUninit::uninit();
    static mut UART:MaybeUninit<os::uart0::Driver>=MaybeUninit::uninit();
    fn read(a:usize)->u32 {unsafe {(a as *const u32).read_volatile()}}
    fn barrier() {unsafe {core::arch::asm!("dsb sy",options(nostack));}}
    #[inline(never)]
    fn fail()->! {
        if PERSISTENT {put(161,0);barrier();}
        unsafe {ptr::addr_of_mut!(READY).write_volatile(0);}
        put(183,get(125)); put(125,(get(125)&0x3f00)|0xff); kernel_restart::i2c_failure(0x77);
    }
    fn check(ok:bool) {if !ok {fail();}}
    fn phase(v:u32) {put(125,(get(125)&0x3f00)|v);}
    fn inactive()->bool {unsafe {os::spi0::active_generation()|os::i2c1::active_generation()|os::uart0::active_generation()==0}}
    fn nvic_quiet()->bool {
        [0xe000_e100,0xe000_e200,0xe000_e300].iter().copied()
            .all(|a| read(a)&((1<<19)|(1<<25)|(1<<8))==0)
    }
    fn spi_clean()->bool {
        let b=rp1_hal::addr::SPI0_BASE; let sr=read(b+0x28);
        read(b+0x5c)==0x3430_322a && [0x08,0x10,0x2c,0x30,0x20,0x24,0x4c]
            .iter().copied().all(|o|read(b+o)==0) && sr&5==4 && sr&!6==0
            && read(b+0x34)&!1==0 && matches!(read(b+0x108),0|1)
    }
    fn i2c_clean()->bool {
        let s=i2c::i2c1_read1_snapshot();
        s.irq.enable_status==0 && s.irq.interrupt_mask==0 && s.rx_level==0 && s.tx_level==0
            && (s.irq.raw_interrupt_status|s.irq.masked_interrupt_status)&rp1_hal::i2c_rx_state::OWNED_CAUSES==0
            && s.irq.masked_interrupt_status==0 && s.irq.abort_source==0
            && [2,3].iter().copied().all(|pin|read(0x400d_0004+pin*8)&0x1f==3 && read(0x400f_0004+pin*4)&0xff==0x7a)
            && read(0x400e_0004)&0xc==0 && read(0x400e_0008)&0xc==0xc
    }
    fn uart_clean()->bool {
        let b=rp1_hal::addr::UART0_BASE;
        read(b+0x30)&0x301==0x101 && read(b+0x38)==0 && read(b+0x18)&0x18==0x10
            && read(b+4)&0xf==0 && (read(b+0x3c)|read(b+0x40))&0x50==0 && read(b+0x40)==0
    }
    fn clean()->bool {inactive() && nvic_quiet() && spi_clean() && i2c_clean() && uart_clean()}
    fn miso()->Option<bool> {
        let s=read(0x400d_0048);
        (read(0x400d_004c)==0x80 && read(0x400f_0028)&0xff==0xfb && s&(1<<13)==0
            && read(0x400e_0004)&(1<<9)==0 && read(0x400e_0008)&((1<<7)|(1<<8)|(1<<11))==((1<<7)|(1<<8)))
            .then_some(s&(1<<17)!=0)
    }
    fn totals()->bool {(0..3).all(|k|get(169+k)==if PERSISTENT {get(156+k)}else{get(word(k,2))+get(word(k+3,2))})}
    fn admission() {check(clean() && totals()); check(unsafe {os::notification_take(true,0).unwrap()}==0);}
    pub fn set_hosts(spi:Spi0Host,i2c:I2c1Host,uart:Uart0Tx) {
        check(unsafe {ptr::addr_of!(STATE).read_volatile()}==0);
        unsafe {ptr::addr_of_mut!(HOSTS).write(Some((spi,i2c,uart)));ptr::addr_of_mut!(STATE).write_volatile(1);}
    }
    pub fn publish() {put(124,u32::from_le_bytes(if PERSISTENT {*b"BC01"}else{*b"AZ01"}));put(125,1);}
    #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
    fn update_begin() {put(150,get(150).checked_add(1).unwrap());barrier();}
    #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
    fn update_end() {barrier();put(150,get(150).checked_add(1).unwrap());}
    fn operation(g:u32,op:u32,ticks:u32) {
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        {update_begin();put(151,g);put(152,op);let now=unsafe {os::tick().unwrap()};
            put(153,now.wrapping_add(ticks));if op==8 {put(165,now);}update_end();}
        #[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
        {let _=(g,op,ticks);}
    }
    #[inline(never)]
    unsafe fn start_drivers() {unsafe {
        let (spi,i2c,uart)=ptr::addr_of_mut!(HOSTS).replace(None).unwrap();
        ptr::addr_of_mut!(SPI).write(MaybeUninit::new(os::spi0::Driver::new(spi)));
        ptr::addr_of_mut!(I2C).write(MaybeUninit::new(os::i2c1::Driver::new(i2c)));
        ptr::addr_of_mut!(UART).write(MaybeUninit::new(os::uart0::Driver::new(uart)));
    }}
    unsafe fn wait(high:bool,ticks:u32) {unsafe {
        let deadline=os::tick().unwrap().wrapping_add(ticks);
        loop {
            admission(); let level=miso(); check(level.is_some());
            check(os::deadline_remaining(os::tick().unwrap(),deadline).is_some());
            if level==Some(high) {break;} os::delay(1).unwrap();
        }
    }}
    #[inline(never)]
    unsafe fn finish(k:usize,g:u32,r:[u32;15],b:&Buffer) {unsafe {
        let snapshot=ptr::read_volatile(b); check(receipt_ok(k,g,&r) && buffer_ok(k,g,&snapshot));
        check(inactive() && !os::spi0::cancel(0) && !os::i2c1::cancel(0) && !os::uart0::cancel(0));
        check(!match k {0=>os::spi0::cancel(g),1=>os::uart0::cancel(g),_=>os::i2c1::cancel(g)});
        let words=compact(k,&r,digest(&snapshot.bytes[..length(k,g)]));
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        {
            // Validate the withdrawn operation and its late buffer guard BEFORE
            // publishing checked progress. During this wait the old op/deadline
            // remains valid with active_generation=0; no torn success record.
            for attempt in 0..2 {
                check(clean() && (0..3).all(|bus|get(169+bus)==get(156+bus)+if bus==k {r[2]}else{0}));
                check(ptr::read_volatile(b)==snapshot);if attempt==0 {os::delay(2).unwrap();}
            }
            check(ptr::read_volatile(b)==snapshot && clean());
            update_begin();put(126+k*8,g);
            for (i,&v) in words.iter().enumerate() {put(127+k*8+i,v);}
            put(156+k,get(156+k).checked_add(r[2]).unwrap());
            put(154,get(154)+1);put(155,os::tick().unwrap());
            put(152,0);put(153,os::tick().unwrap().wrapping_add(1000));update_end();
            admission();
        }
        #[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
        {
        let slot=(g as usize-1)*3+k;
        for (i,v) in words.into_iter().enumerate() {put(word(slot,i),v);}
        admission(); os::delay(2).unwrap(); check(ptr::read_volatile(b)==snapshot); admission();
        put(125,get(125)|(1<<(8+slot)));
        }
    }}
    #[inline(never)]
    unsafe fn spi(g:u32) {unsafe {
        operation(g,1,3000);wait(true,3000);phase(g*10+1);operation(g,2,30_000);wait(false,30_000); admission();
        let driver=(&mut *ptr::addr_of_mut!(SPI)).assume_init_mut(); let mut b=Buffer::new();
        operation(g,3,100);
        let r=match driver.receive(&[0;4],&mut b.bytes[..4],50) {Ok(r)=>r,Err(_)=>fail()};
        let base=rp1_hal::addr::SPI0_BASE;
        finish(0,g,[r.generation,4,r.irq_entries,get(172),r.elapsed_us,r.irq_body_max_us,
            r.irq_end_to_task_us,0,0,0,0,0,read(base+0x28),read(base),read(base+0x108)],&b);
        phase(g*10+2);operation(g,1,3000);wait(true,3000);
    }}
    #[inline(never)]
    unsafe fn uart(g:u32) {unsafe {
        admission();phase(g*10+3);let driver=(&*ptr::addr_of!(UART)).assume_init_ref();let mut b=Buffer::new();
        let prompt=token(*b"RP1U0 RTOSREADY 0001\r\n",g);operation(g,4,5100);
        let r=match driver.exchange(&prompt,&mut b.bytes[..19],5000) {Ok(r)=>r,Err(_)=>fail()};
        check(r.final_imsc==0 && r.ipsr==get(173));
        finish(1,g,[r.generation,r.received,r.irq_entries,r.ipsr,r.elapsed_us,r.irq_body_max_us,
            r.irq_end_to_task_us,r.higher_priority_wakes,r.rsr_errors|r.first_error_dr|r.overflow_bytes|r.residual_bytes,
            r.cleanup_elapsed_us,r.quiet_samples,0,r.first_ris|(r.first_mis<<16),r.final_cr,r.final_fr],&b);
    }}
    #[inline(never)]
    unsafe fn i2c(g:u32) {unsafe {
        admission();phase(g*10+4);let driver=(&*ptr::addr_of!(I2C)).assume_init_ref();let mut b=Buffer::new();
        operation(g,5,100);
        let r=match driver.receive(0x2d,&mut b.bytes[..length(2,g)],50) {Ok(r)=>r,Err(_)=>fail()};
        // Zero higher-priority wakes is legal: completion need not preempt the
        // interrupted task (or can precede this owner's notification wait).
        finish(2,g,[r.generation,r.received,r.irq_entries,get(174),r.elapsed_us,r.irq_body_max_us,
            r.irq_end_to_task_us,r.higher_priority_wakes,r.first_fatal_causes|r.first_abort_source|r.discarded_after_failure,
            r.cleanup_elapsed_us,r.quiet_samples,r.quiet_max_gap_us,0,0,0],&b);
    }}
    #[inline(never)]
    unsafe fn pulse(marker:&mut ConfiguredPin<22,Output>,ticks:u32,index:usize) {unsafe {
        let start=raw_low();marker.set_high();os::delay(ticks).unwrap();marker.set_low();barrier();put(index,raw_low().wrapping_sub(start));
    }}
    #[inline(never)]
    #[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
    pub fn ready()->bool {
        if unsafe {ptr::addr_of!(READY).read_volatile()}!=1 {return false;}
        get(125)==0x3f07 && totals() && (0..6).all(|s|stored_ok(s%3,(s/3+1) as u32,core::array::from_fn(|i|get(word(s,i)))))
            && (0..3).all(|k|get(172+k)==IPSRS[k]) && clean() && miso()==Some(true)
            && read(rp1_hal::addr::SPI0_BASE)==0x0007_0000 && read(rp1_hal::addr::SPI0_BASE+0x108)==1 && totals()
    }
    #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
    pub fn ready()->bool {unsafe {ptr::addr_of!(READY).read_volatile()==1}}
    #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
    pub fn handback() {put(159,1);barrier();}
    #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
    #[inline(never)]
    pub fn healthy()->bool {
        put(161,0);
        // Owner priority5 can preempt monitor priority4. Never mask interrupts
        // around MMIO or wait for a writer: at most three complete snapshots.
        for _ in 0..3 {
            let seq=get(150);if seq&1!=0 {continue;}
            barrier();let now=unsafe {os::tick().unwrap()};
            let s=core::array::from_fn(|i|get(151+i));let [_,op,_,n,_]=s;
            let active=unsafe {[os::spi0::active_generation(),os::uart0::active_generation(),os::i2c1::active_generation()]};
            let bus=match op {3=>Some(0),4|6=>Some(1),5=>Some(2),_=>None};
            let mut ok=state_ok(now,s,active) && get(183)==0;
            for k in 0..3 {
                let done=(n+2-k as u32)/3;
                ok &= get(126+k*8)==done && (done==0 || stored_ok(k,done,core::array::from_fn(|i|get(127+k*8+i))));
                let delta=get(169+k).wrapping_sub(get(156+k));
                ok &= if bus==Some(k) && op!=6 {delta<=LIMITS[k]}else{delta==0};
                ok &= done==0 && delta==0 || get(172+k)==IPSRS[k];
            }
            // Selected adapter owns its cleanup interval too; active=0 is
            // legal between withdrawal and checked receipt publication.
            // Factor only the repeated calls; order/short-circuiting and the
            // all-quiet path remain identical for bus0/1/2/None.
            ok &= if bus.is_some() {
                (bus==Some(0) || spi_clean()) && (bus==Some(1) || uart_clean())
                    && (bus==Some(2) || i2c_clean())
            } else {clean() && totals()};
            if matches!(op,7|8) {ok &= miso()==Some(true)
                && read(rp1_hal::addr::SPI0_BASE)==0x0007_0000 && read(rp1_hal::addr::SPI0_BASE+0x108)==1;}
            if op==6 {ok &= read(rp1_hal::addr::UART0_BASE+0x38)==0;}
            let allowed=match op {3=>1<<19,4=>1<<25,5=>1<<8,_=>0};
            ok &= [0xe000_e100,0xe000_e200,0xe000_e300].iter().copied()
                .all(|a|read(a)&(((1<<19)|(1<<25)|(1<<8))&!allowed)==0);
            barrier();if !coherent(seq,get(150)) {continue;}
            if !ok {return false;}
            put(162,now);put(163,seq);barrier();put(161,1);return true;
        }
        false
    }
    pub unsafe extern "C" fn worker(arg:*mut c_void) {unsafe {
        check(arg.is_null() && ptr::addr_of!(STATE).read_volatile()==1);
        check(os::current_task().unwrap().is_some_and(|task| task.id()==8));
        let (ipsr,control,psp):(u32,u32,u32);
        core::arch::asm!("mrs {0}, IPSR","mrs {1}, CONTROL","mrs {2}, PSP",out(reg)ipsr,out(reg)control,out(reg)psp,options(nomem,nostack));
        put(175,ipsr);put(176,control);put(177,psp);check(ipsr==0 && control&3==2 && psp&7==0);
        let mut marker=ptr::addr_of_mut!(MARKER).replace(None).unwrap();marker.set_low();barrier();
        ptr::addr_of_mut!(STATE).write_volatile(2);phase(2);admission();start_drivers();admission();
        for &irq in &[19usize,25,8] {check(((0xe000_e400+irq) as *const u8).read_volatile()==0xc0);}
        put(180,read(rp1_hal::addr::SPI0_BASE));put(181,read(rp1_hal::addr::I2C1_BASE));put(182,read(rp1_hal::addr::UART0_BASE+0x30));
        check(miso()==Some(true));pulse(&mut marker,13,178);
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        let mut marker=Some(marker);
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        {update_begin();put(164,os::tick().unwrap());put(155,get(164));update_end();}
        // ponytail: eight-generation commissioning cap; a longer constant needs
        // a separate same-image cohort, not an inferred persistent proof.
        for g in 1..=GENERATIONS {
            spi(g);uart(g);i2c(g);phase(g*10+5);
            let ack=token(*b"RP1U0 RTOSOK 0001\r\n",g);operation(g,6,200);
            check((&*ptr::addr_of!(UART)).assume_init_ref().write_all(&ack,100).is_ok());admission();
            #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
            if g==2 {
                let mut marker=marker.take().unwrap();
                pulse(&mut marker,29,179);admission();check(miso()==Some(true));
                marker.set_low();barrier();ptr::addr_of_mut!(MARKER).write(Some(marker));barrier();
                phase(7);operation(g,7,20_000);ptr::addr_of_mut!(READY).write_volatile(1);barrier();
                while get(159)==0 {
                    check(os::deadline_remaining(os::tick().unwrap(),get(153)).is_some());
                    admission();os::delay(1).unwrap();
                }
                // GPIO22 is now permanently monitor-owned; do not touch marker.
            }
        }
        #[cfg(feature="freertos-r3-watchdog-warm-persistent")]
        {
            check(miso()==Some(true));admission();phase(8);
            operation(GENERATIONS,8,0);
        }
        #[cfg(not(feature="freertos-r3-watchdog-warm-persistent"))]
        {
        check(get(125)&0x3f00==0x3f00 && miso()==Some(true));pulse(&mut marker,29,179);admission();
        check(miso()==Some(true));marker.set_low();barrier();ptr::addr_of_mut!(MARKER).write(Some(marker));barrier();
        phase(7);ptr::addr_of_mut!(READY).write_volatile(1);barrier();
        }
        // Monitor alone owns GPIO22 and revokes a post-result contract failure.
        loop {os::delay(1000).unwrap();}
    }}
    #[inline(always)]
    unsafe fn irq(k:usize) {unsafe {
        let ipsr:u32;core::arch::asm!("mrs {}, IPSR",out(reg)ipsr,options(nomem,nostack));
        put(169+k,get(169+k).saturating_add(1));put(172+k,ipsr);
    }}
    #[unsafe(no_mangle)] unsafe extern "C" fn SPI0_IRQHandler() {unsafe {irq(0);os::spi0::on_interrupt();}}
    #[unsafe(no_mangle)] unsafe extern "C" fn UART0_IRQHandler() {unsafe {irq(1);os::uart0::on_interrupt();}}
    #[unsafe(no_mangle)] unsafe extern "C" fn I2C1_IRQHandler() {unsafe {irq(2);os::i2c1::on_interrupt();}}
}
#[cfg(target_arch="arm")]
pub use target::{set_hosts,publish,ready,worker};
#[cfg(all(target_arch="arm",feature="freertos-r3-watchdog-warm-persistent"))]
pub use target::{healthy,handback};

#[cfg(all(test,not(feature="freertos-r3-watchdog-warm-persistent")))]
mod tests {
    use super::*;
    #[test]
    fn payloads_guards_receipts_and_layout() {
        let mut used=[false;256];
        for (first,len) in [(96,28),(124,2),(160,1),(169,15),(184,72)] {
            for i in first..first+len {assert!(!used[i]);used[i]=true;}
        }
        for s in 0..6 {for f in 0..7 {let i=word(s,f);assert!(!used[i]);used[i]=true;}}
        assert!(used[96..].iter().all(|v|*v));assert_eq!(1792+512,2304);
        assert_eq!([byte(2,1,0),byte(2,1,1)],[0x31,0x4e]);assert_eq!([byte(2,2,0),byte(2,2,1)],[0x30,0x4e]);
        for k in 0..3 {for g in 1..=2 {
            let mut b=Buffer::new();for i in 0..length(k,g) {b.bytes[i]=byte(k,g,i);}
            assert!(buffer_ok(k,g,&b));assert!(!buffer_ok(k,3-g,&b));
            for i in 0..32 {b.bytes[i]^=1;assert!(!buffer_ok(k,g,&b));b.bytes[i]^=1;}
            b.before^=1;assert!(!buffer_ok(k,g,&b));b.before=BEFORE;b.after^=1;assert!(!buffer_ok(k,g,&b));b.after=AFTER;
            let mut r=[g,length(k,g) as u32,1,IPSRS[k],10_000,10,20,0,0,0,0,0,4,0x0007_0000,1];
            if k>0 {r[9]=9000;r[10]=8;r[12]=0;r[13]=0;r[14]=0;}
            if k==1 {r[7]=1;r[12]=0x0050_0050;r[13]=0x101;r[14]=0x197;}
            assert!(receipt_ok(k,g,&r));
            for (i,v) in [(0,3-g),(1,0),(2,0),(2,LIMITS[k]+1),(3,0),(4,0),(5,10_001),(6,10_001),(7,2),(8,1)] {
                let mut bad=r;bad[i]=v;assert!(!receipt_ok(k,g,&bad),"kind {k} field {i}");
            }
            if k>0 {for (i,v) in [(9,3999),(10,1),(11,10_000)] {let mut bad=r;bad[i]=v;assert!(!receipt_ok(k,g,&bad));}}
            let hash=digest(&b.bytes[..length(k,g)]);assert_eq!(hash,DIGESTS[k][g as usize-1]);
            let w=compact(k,&r,hash);assert!(stored_ok(k,g,w));
            for i in 0..4 {let mut bad=w;bad[i]=0;assert!(!stored_ok(k,g,bad));}
        }}
    }
}

#[cfg(all(test,feature="freertos-r3-watchdog-warm-persistent"))]
mod persistent_test {
    fn state_reference(now:u32,s:[u32;5],active:[u32;3])->bool {
        let [g,op,deadline,n,last]=s;
        if !(1..=GENERATIONS).contains(&g) || n>GENERATIONS*3 {return false;}
        let base=(g-1)*3;
        let count_ok=match op {0=>n>base && n<=base+3,1=>n==base || n==base+1,2|3=>n==base,
            4=>n==base+1,5=>n==base+2,6=>n==base+3,7=>g==2 && n==6,
            8=>g==GENERATIONS && n==GENERATIONS*3,_=>false};
        let bus=match op {3=>Some(0),4=>Some(1),5=>Some(2),_=>None};
        let budget=match op {0=>1000,1=>3000,2=>30_000,3|5=>100,4=>5100,6=>200,7=>20_000,_=>0};
        let freshness=if g==3 && n==6 {55_000}else{35_000}; // Includes the sole type8 pause.
        count_ok && active.into_iter().enumerate().all(|(k,a)|a==0 || bus==Some(k) && a==g)
            && (op==8 || deadline.wrapping_sub(now)>0 && deadline.wrapping_sub(now)<=budget)
            && (op==8 || now.wrapping_sub(last)<=freshness)
    }

    #[test]
    fn compact_state_matches_branch_model() {
        for now in [100_000u32,u32::MAX-10] {
            for g in 0..=9 {for op in 0..=9 {for n in 0..=25 {
                for age in [0u32,35_000,35_001,55_000,55_001,u32::MAX] {
                    for remaining in [0u32,1,100,1000,3000,5100,20_000,30_000,30_001,u32::MAX] {
                        let s=[g,op,now.wrapping_add(remaining),n,now.wrapping_sub(age)];
                        for active in [[0;3],[g,0,0],[0,g,0],[0,0,g],[g,g,0],[99,0,0]] {
                            assert_eq!(state_ok(now,s,active),state_reference(now,s,active),"{s:?} {active:?}");
                        }
                    }
                }
            }}}
        }
    }
    use super::*;
    #[test]
    fn commissioning_payloads_accounting_deadlines_and_revocation() {
        assert_eq!(GENERATIONS,8);assert_eq!(1792+512,2304);
        assert_eq!(126+3*8,150);assert!(165<192);assert!(160>158 && 160<161);
        assert_eq!(token(*b"RP1U0 RTOSREADY 0001\r\n",3),*b"RP1U0 RTOSREADY 0003\r\n");
        assert_eq!(token(*b"RP1U0 RTOSOK 0001\r\n",384),*b"RP1U0 RTOSOK 0384\r\n");
        assert_eq!(byte(0,3,3),byte(0,1,3)); // Payload reuse is NOT generation reuse.
        assert_eq!(length(2,3),2);assert_eq!(length(2,4),31);
        assert_eq!([byte(2,3,0),byte(2,3,1)],[0x33,0x4e]);
        assert_eq!([byte(2,257,0),byte(2,257,1)],[0x31,0x4f]);
        let mut latest=[0;3];let mut sums=[0;3];let mut n=0;
        for g in 1..=GENERATIONS {for k in 0..3 {
            let mut b=Buffer::new();for i in 0..length(k,g) {b.bytes[i]=byte(k,g,i);}
            assert!(buffer_ok(k,g,&b));
            for i in 0..32 {b.bytes[i]^=1;assert!(!buffer_ok(k,g,&b));b.bytes[i]^=1;}
            b.before^=1;assert!(!buffer_ok(k,g,&b));b.before=BEFORE;
            b.after^=1;assert!(!buffer_ok(k,g,&b));b.after=AFTER;
            let mut r=[g,length(k,g) as u32,1,IPSRS[k],10_000,10,20,0,0,0,0,0,4,0x0007_0000,1];
            if k>0 {r[9]=9000;r[10]=8;r[12]=0;r[13]=0;r[14]=0;}
            if k==1 {r[7]=1;r[12]=0x0050_0050;r[13]=0x101;r[14]=0x197;}
            assert!(receipt_ok(k,g,&r));
            let mut bad=r;bad[0]=if g>2 {g-2}else{g+2};assert!(!receipt_ok(k,g,&bad));
            for (i,v) in [(1,0),(2,0),(2,LIMITS[k]+1),(3,0),(4,0),(5,10_001),(6,10_001),(7,2),(8,1)] {
                bad=r;bad[i]=v;assert!(!receipt_ok(k,g,&bad));
            }
            if k>0 {for (i,v) in [(9,3999),(10,1),(11,10_000)] {bad=r;bad[i]=v;assert!(!receipt_ok(k,g,&bad));}}
            let hash=digest(&b.bytes[..length(k,g)]);assert_eq!(hash,expected_digest(k,g));
            let w=compact(k,&r,hash);assert_eq!(w[0]&0xff,0);assert!(stored_ok(k,g,w));
            for i in 0..4 {let mut bad=w;bad[i]=0;assert!(!stored_ok(k,g,bad));}
            latest[k]=g;sums[k]+=r[2];n+=1;
            for bus in 0..3 {assert_eq!(latest[bus],(n+2-bus as u32)/3);}
        }}
        assert_eq!((n,latest,sums),(24,[8;3],[8;3]));
        let now=100_000;
        for (op,count,budget,bus) in [(0,7,1000,None),(1,6,3000,None),(2,6,30_000,None),
            (3,6,100,Some(0)),(4,7,5100,Some(1)),(5,8,100,Some(2)),(6,9,200,None)] {
            let s=[3,op,now+budget,count,now];let mut active=[0;3];
            if let Some(k)=bus {active[k]=3;}
            assert!(state_ok(now,s,active));assert!(state_ok(now,s,[0;3])); // Withdrawn, not yet published.
            assert!(!state_ok(now+budget,s,active));
            let mut bad=s;bad[2]+=1;assert!(!state_ok(now,bad,active));
            bad=s;bad[4]=now-if count==6 {55_001}else{35_001};assert!(!state_ok(now,bad,active));
            bad=s;bad[3]=24;assert!(!state_ok(now,bad,active));
            for k in 0..3 {let mut wrong=active;wrong[k]=2;assert!(!state_ok(now,s,wrong));}
        }
        assert!(state_ok(now,[3,1,now+3000,7,now],[0;3])); // SPI post-HIGH boundary.
        assert!(state_ok(now,[3,2,now+30_000,6,now-55_000],[0;3]));
        assert!(!state_ok(now,[4,2,now+30_000,9,now-35_001],[0;3]));
        assert!(state_ok(now,[2,7,now+20_000,6,now],[0;3]));
        assert!(!state_ok(now,[3,7,now+20_000,9,now],[0;3]));
        assert!(state_ok(now,[8,8,0,24,1],[0;3])); // Explicit bounded completion, not owner freshness.
        assert!(!state_ok(now,[8,8,0,23,1],[0;3]));
        assert!(!state_ok(now,[8,8,0,24,1],[0,8,0]));
        assert!(state_ok(u32::MAX-10,[3,3,39,6,u32::MAX-20],[3,0,0]));
        assert!(coherent(24,24));assert!(!coherent(25,25));assert!(!coherent(24,26));
        // A stale/failed/coherently invalid observation cannot become eligible.
        let mut eligible=true;
        for (before,after,s) in [(24,26,[3,3,now+50,6,now]),(24,24,[3,3,now,6,now])] {
            eligible=false; // Same revoke-before-check ordering as the monitor.
            if coherent(before,after) && state_ok(now,s,[3,0,0]) {eligible=true;}
            assert!(!eligible);
        }
    }
}

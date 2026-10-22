//! Phase1 finite proc1 commissioning: readonly timer, local control, uncollected
//! OBS1 queue and asynchronous RPMsg parameter ingress. No sensor IRQ,
//! peripheral transfer or reconnect claim. GPIO22 is a finite demonstration
//! output: nonzero parameter0 means HIGH while active; every safe state is LOW.
const PERIOD_US: u64 = 200;
const CYCLES: u64 = 100_000;
const FILL: u32 = 0xa5a5_a5a5;
const GUARD: u32 = 0x31c0_a55a;

#[derive(Default)]
struct Launch { attempted: bool }
impl Launch {
    fn admit(&mut self, core: u32, ctrl: u32, done: u32, boot: u64, session: u64) -> bool {
        if self.attempted { return false; }
        // A failed initial guard permanently closes this launch capability.
        self.attempted = true;
        core == 0 && ctrl & (1 << 31) != 0 && done & (1 << 31) == 0
            && boot != 0 && session != 0
    }
}

fn due(next: u64, now: u64) -> Option<(u64, u64)> {
    if now < next { return None; }
    let late = now - next;
    if late < PERIOD_US { return Some((next.checked_add(PERIOD_US)?, 0)); }
    let skipped = late / PERIOD_US;
    Some((next.checked_add((skipped + 1).checked_mul(PERIOD_US)?)?, skipped))
}

fn guard_due(remaining: &mut u32) -> bool {
    *remaining -= 1;
    if *remaining != 0 { return false; }
    *remaining = 1000;
    true
}

fn scan_duration(start: u64, end: u64) -> Option<u32> {
    u32::try_from(end.checked_sub(start)?).ok()
}

fn output_high(active: bool, parameter0: u32) -> bool { active && parameter0 != 0 }

#[cfg(target_arch = "arm")]
pub(crate) use hardware::handoff_marker;

#[cfg(all(target_arch = "arm", feature = "freertos-amp-proc1-gpio-irq"))]
mod gpio_irq;
#[cfg(all(target_arch = "arm", feature = "freertos-amp-proc1-profile"))]
mod profile;

/// Profiling hook; expands to nothing unless `freertos-amp-proc1-profile`.
macro_rules! prof {
    ($($call:tt)*) => {
        #[cfg(feature = "freertos-amp-proc1-profile")]
        super::profile::$($call)*;
    };
}
#[cfg(all(target_arch = "arm", feature = "freertos-amp-proc1-gpio-irq"))]
pub(crate) use gpio_irq::handoff_input;

#[cfg(target_arch = "arm")]
mod hardware {
    use super::*;
    use core::{mem::MaybeUninit, ptr, sync::atomic::{AtomicU32, Ordering}};
    use rp1_abi::{amp::{Error, Generation}, observation::Observation};
    use rp1_hal::{local_control::{ControlUpdate, Decision, LocalControl, SampleStamp},
        local_queue::{LocalQueue, Producer, Consumer}, gpio::{ConfiguredPin, Output}};

    // One C0 writer before release; C1 exclusively owns producer/controller/MSP.
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut TRACE: MaybeUninit<LocalQueue<Observation,8>> = MaybeUninit::uninit();
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut TRACE_PRODUCER: Option<Producer<'static,Observation,8>> = None;
    #[cfg_attr(not(feature="freertos-amp-observation"),unsafe(link_section = ".amp_proc1.storage"))]
    #[cfg_attr(feature="freertos-amp-observation",unsafe(link_section = ".amp_observation_state"))]
    static mut TRACE_CONSUMER: Option<Consumer<'static,Observation,8>> = None;
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut GENERATION: MaybeUninit<Generation> = MaybeUninit::uninit();
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut COMMANDS: MaybeUninit<LocalQueue<ControlUpdate,1>> = MaybeUninit::uninit();
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut COMMAND_PRODUCER: Option<Producer<'static,ControlUpdate,1>> = None;
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut COMMAND_CONSUMER: Option<Consumer<'static,ControlUpdate,1>> = None;
    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".amp_proc1.storage")]
    static RP1_AMP_COMMAND_RECEIPT: [AtomicU32;16] = [const {AtomicU32::new(0)};16];
    #[unsafe(link_section = ".amp_proc1.storage")]
    static STOP: AtomicU32 = AtomicU32::new(0);
    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".amp_proc1.storage")]
    static mut RP1_AMP_PROC1_STATUS: [u32;32] = [0;32];
    #[used]
    #[unsafe(link_section = ".amp_proc1.stack")]
    static mut PROC1_STACK: [u64;258] = [0;258];
    static mut LAUNCH: Launch = Launch { attempted: false };
    // The existing cold marker token is moved once; C0 never writes it again.
    static mut OUTPUT: Option<ConfiguredPin<22,Output>> = None;
    const CTRL: *mut u32 = 0x4001_4000 as *mut u32;
    const DONE: *const u32 = 0x4001_4018 as *const u32;
    const CORE: *const u32 = 0xe00f_f01c as *const u32;
    const READY: u32 = u32::from_le_bytes(*if cfg!(feature="freertos-amp-proc1-gpio-irq") {b"P1A9"} else {b"P1A5"});

    pub(crate) fn handoff_marker(mut marker: ConfiguredPin<22,Output>) {
        marker.set_low();
        unsafe { ptr::addr_of_mut!(OUTPUT).write(Some(marker)); }
        barrier();
    }

    unsafe extern "C" {
        static __amp_proc1_vectors: u8;
        static __amp_proc1_stack_low: u8;
        static __amp_proc1_stack_top: u8;
        static __amp_proc1_guard_low: u8;
        static __amp_proc1_guard_high: u8;
        fn AmpProc1Entry() -> !;
        #[cfg(not(feature="freertos-amp-proc1-gpio-irq"))]
        #[link_name="AmpProc1Fault"]
        fn AmpGpioVector() -> !;
    }
    #[cfg(feature="freertos-amp-proc1-gpio-irq")]
    use super::gpio_irq::AmpProc1GpioIrq as AmpGpioVector;
    core::arch::global_asm!(
        ".syntax unified", ".thumb",
        ".section .amp_proc1.vectors,\"a\",%progbits", ".balign 512",
        ".word __amp_proc1_stack_top", ".word AmpProc1Entry",
        ".rept 14", ".word AmpProc1Fault", ".endr",
        ".word {gpio_vector}",
        ".rept 63", ".word AmpProc1Fault", ".endr",
        ".section .text.amp_proc1.entry,\"ax\",%progbits", ".balign 4",
        ".global AmpProc1Entry", ".type AmpProc1Entry,%function", ".thumb_func",
        "AmpProc1Entry:", "cpsid i", "cmp r0,#1", "bne AmpProc1Fault",
        "ldr r1,=0xe00ff01c", "ldr r1,[r1]", "cmp r1,#1", "bne AmpProc1Fault",
        "mrs r1,IPSR", "cmp r1,#0", "bne AmpProc1Fault",
        "mrs r1,CONTROL", "cmp r1,#0", "bne AmpProc1Fault",
        "ldr r1,=__amp_proc1_vectors", "ldr r2,=0xe000ed08", "str r1,[r2]",
        "dsb sy", "isb sy",
        ".if {local_stack}",
        // Diagnostic: C1 fills its own proc-local DSRAM stack (C0 cannot reach this
        // instance) with the scan pattern, then runs from it. Registers only.
        "ldr r1,=0x10002000", "ldr r2,=0x10002800", "ldr r3,={fill}",
        "2:", "str r3,[r1],#4", "cmp r1,r2", "bne 2b", "dsb sy", "msr MSP,r2",
        ".else",
        "ldr r1,=__amp_proc1_stack_top", "msr MSP,r1",
        ".endif",
        "b.w AmpProc1Body", ".size AmpProc1Entry,.-AmpProc1Entry",
        ".global AmpProc1Fault", ".type AmpProc1Fault,%function", ".thumb_func",
        "AmpProc1Fault:", "cpsid i",
        ".if {gpio_irq_enabled}",
        "ldr r0,=0x400d3110", "mov.w r1,#512", "str r1,[r0]",
        "ldr r0,=0x400d3114", "str r1,[r0]",
        "ldr r0,=0xe000e180", "movs r1,#1", "str r1,[r0]", "dsb sy",
        ".endif",
        "ldr r0,=RP1_AMP_PROC1_STATUS", "movs r1,#4",
        "str r1,[r0,#4]", "mrs r1,IPSR", "str r1,[r0,#8]", "dmb sy",
        "ldr r0,=0x400e3000", "movs r1,#1", "lsls r1,r1,#22", "str r1,[r0]", "dsb sy",
        "1:", "dsb sy", "wfe", "b 1b", ".size AmpProc1Fault,.-AmpProc1Fault",
        gpio_vector = sym AmpGpioVector,
        gpio_irq_enabled = const (cfg!(feature="freertos-amp-proc1-gpio-irq")) as u32,
        local_stack = const (cfg!(feature="freertos-amp-proc1-local-stack")) as u32,
        fill = const FILL,
    );
    /// C1 stack actually in use: shared SRAM, or proc-local DSRAM in the diagnostic build.
    fn stack_low()->*const u32 {
        if cfg!(feature="freertos-amp-proc1-local-stack") {0x1000_2000 as *const u32}
        else {ptr::addr_of!(__amp_proc1_stack_low).cast::<u32>()}
    }
    fn stack_top()->u32 {
        if cfg!(feature="freertos-amp-proc1-local-stack") {0x1000_2800}
        else {ptr::addr_of!(__amp_proc1_stack_top) as u32}
    }

    fn get(i:usize)->u32 { unsafe { ptr::addr_of!(RP1_AMP_PROC1_STATUS).cast::<u32>().add(i).read_volatile() } }
    fn put(i:usize,v:u32) { unsafe { ptr::addr_of_mut!(RP1_AMP_PROC1_STATUS).cast::<u32>().add(i).write_volatile(v) } }
    fn barrier() { unsafe { core::arch::asm!("dmb sy",options(nostack,preserves_flags)); } }
    pub(super) fn now()->Option<u64> {
        for _ in 0..4 {
            let hi=unsafe {(0x400a_c024 as *const u32).read_volatile()};
            let lo=unsafe {(0x400a_c028 as *const u32).read_volatile()};
            if hi==unsafe {(0x400a_c024 as *const u32).read_volatile()} {return Some(u64::from(hi)<<32|u64::from(lo));}
        }
        None
    }
    fn guards()->bool {
        [ptr::addr_of!(__amp_proc1_guard_low),ptr::addr_of!(__amp_proc1_guard_high)]
            .into_iter().all(|p|unsafe {p.cast::<u32>().read_volatile()==GUARD && p.cast::<u32>().add(1).read_volatile()==!GUARD})
    }
    fn park(state:u32)->! {
        #[cfg(feature="freertos-amp-proc1-gpio-irq")]
        let state=if super::gpio_irq::stop() {state} else {4};
        let epoch=get(31)|1;put(31,epoch);barrier();
        let state=if clear_output() {state} else {4};
        put(1,state);barrier();put(31,epoch.wrapping_add(1));
        loop {unsafe {core::arch::asm!("dsb sy","wfe",options(nostack,preserves_flags));}}
    }

    fn clear_output()->bool {
        // Same owned GPIO22 CLR alias; no bank-wide RMW, clock or reset write.
        unsafe { (0x400e_3000 as *mut u32).write_volatile(1<<22);
            core::arch::asm!("dsb sy",options(nostack,preserves_flags)); }
        if get(30)!=0 {put(29,get(29).saturating_add(1));}
        put(30,0);
        unsafe {(0x400e_0000 as *const u32).read_volatile() & (1<<22)==0}
    }

    fn finish(state:u32)->! {
        // The full scan exceeded 200us in P1A2; run it only after control retires.
        put(31,get(31).wrapping_add(1));barrier();
        if !clear_output() {park(4);}
        if !guards() {park(4);}
        let Some(start)=now() else {park(4)};
        let mut free=0;
        while free<512 && unsafe {stack_low().add(free).read_volatile()}==FILL {free+=1;}
        let Some(duration)=now().and_then(|end|scan_duration(start,end)) else {park(4)};
        put(24,free as u32);put(25,1);put(26,duration);put(27,duration);put(28,duration);
        barrier();put(31,get(31).wrapping_add(1));
        park(if free<32 {4} else {state})
    }

    /// Called only by C0's sole OpenAMP worker after an admitted HELLO. This is
    /// initial deassertion from held reset, never a reset/restart/remove path.
    #[unsafe(no_mangle)]
    #[inline(never)]
    #[cfg_attr(feature="freertos-amp-proc1-gpio-irq",unsafe(link_section=".amp_proc1.irq_text"))]
    unsafe extern "C" fn rp1_control_session(boot:u64,session:u64)->u32 {
        let core=unsafe {CORE.read_volatile()};
        let ctrl=unsafe {CTRL.read_volatile()};let done=unsafe {DONE.read_volatile()};
        if !unsafe {(&mut *ptr::addr_of_mut!(LAUNCH)).admit(core,ctrl,done,boot,session)} {return 9;}
        if unsafe { (&*ptr::addr_of!(OUTPUT)).is_none() } {return 9;}
        unsafe {
            ptr::addr_of_mut!(RP1_AMP_PROC1_STATUS).write([0;32]);
            ptr::addr_of_mut!(GENERATION).write(MaybeUninit::new(Generation{boot,session}));
            ptr::addr_of_mut!(TRACE).write(MaybeUninit::new(LocalQueue::new()));
            let queue=&mut *ptr::addr_of_mut!(TRACE).cast::<LocalQueue<Observation,8>>();
            let (p,c)=queue.split();
            ptr::addr_of_mut!(TRACE_PRODUCER).write(Some(p));
            ptr::addr_of_mut!(TRACE_CONSUMER).write(Some(c));
            ptr::addr_of_mut!(COMMANDS).write(MaybeUninit::new(LocalQueue::new()));
            let queue=&mut *ptr::addr_of_mut!(COMMANDS).cast::<LocalQueue<ControlUpdate,1>>();
            let (p,c)=queue.split();
            ptr::addr_of_mut!(COMMAND_PRODUCER).write(Some(p));
            ptr::addr_of_mut!(COMMAND_CONSUMER).write(Some(c));
            for word in &RP1_AMP_COMMAND_RECEIPT {word.store(0,Ordering::Relaxed);}
            RP1_AMP_COMMAND_RECEIPT[0].store(u32::from_le_bytes(*b"CTR1"),Ordering::Relaxed);
            RP1_AMP_COMMAND_RECEIPT[1].store(1,Ordering::Relaxed);
            STOP.store(0,Ordering::Relaxed);
            for p in [ptr::addr_of!(__amp_proc1_guard_low),ptr::addr_of!(__amp_proc1_guard_high)] {
                p.cast_mut().cast::<u32>().write_volatile(GUARD);
                p.cast_mut().cast::<u32>().add(1).write_volatile(!GUARD);
            }
            for i in 0..512 {ptr::addr_of!(__amp_proc1_stack_low).cast_mut().cast::<u32>().add(i).write_volatile(FILL);}
            put(16,boot as u32);put(17,(boot>>32) as u32);put(18,session as u32);put(19,(session>>32) as u32);
            put(20,ptr::addr_of!(__amp_proc1_stack_low) as u32);put(21,ptr::addr_of!(__amp_proc1_stack_top) as u32);
            // Recheck before scratch/release; no retry or reassertion on failure.
            let ctrl=CTRL.read_volatile();
            if ctrl&(1<<31)==0 || DONE.read_volatile()&(1<<31)!=0 {return 9;}
            barrier();
            (0x4015_400c as *mut u32).write_volatile(0xb007_c0de);
            (0x4015_401c as *mut u32).write_volatile(ptr::addr_of!(__amp_proc1_stack_top) as u32);
            core::arch::asm!("dsb sy",options(nostack,preserves_flags));
            (0x4015_4014 as *mut u32).write_volatile((AmpProc1Entry as *const () as u32|1)^0x4ff8_3f2d);
            core::arch::asm!("dsb sy",options(nostack,preserves_flags));
            CTRL.write_volatile(ctrl&!(1<<31));
            core::arch::asm!("dsb sy","sev",options(nostack,preserves_flags));
        }
        let Some(start)=now() else {STOP.store(1,Ordering::Release);return 9;};
        for _ in 0..100_000 {
            if get(0)==READY && get(1)==1 {barrier();return 0;}
            if now().is_none_or(|t| t<start || t-start>=100_000) {break;}
        }
        STOP.store(1,Ordering::Release);9
    }

    #[unsafe(no_mangle)]
    #[inline(never)]
    extern "C" fn rp1_control_retire() {STOP.store(1,Ordering::Release);}

    /// Sole C0 worker takes the one consumer after admitted C1 launch. No reset
    /// or reconstruction while its producer remains on C1.
    #[cfg(feature="freertos-amp-observation")]
    #[unsafe(no_mangle)]
    unsafe fn rp1_control_take_trace() -> Option<Consumer<'static,Observation,8>> {
        if unsafe {CORE.read_volatile()}!=0 || STOP.load(Ordering::Acquire)!=0 || get(1)!=1 {return None;}
        unsafe {(&mut *ptr::addr_of_mut!(TRACE_CONSUMER)).take()}
    }

    fn status(error:Error)->u32 {
        match error {
            Error::Full=>5, Error::Generation|Error::Owner=>2, Error::Sequence=>3,
            Error::Time|Error::Expired=>4, Error::Bounds=>7, _=>1,
        }
    }

    /// C0's sole OpenAMP worker: copy a checked update, or observe a C1 receipt.
    /// Status0 on SET/STOP means QUEUED; QUERY_CONTROL carries the application receipt.
    #[unsafe(no_mangle)]
    #[inline(never)]
    unsafe extern "C" fn rp1_control_message(packet:*const u8,now:u64,out:*mut u32)->u32 {
        if unsafe {CORE.read_volatile()}!=0 || STOP.load(Ordering::Acquire)!=0 || get(1)!=1 {return 6;}
        let packet=unsafe {core::slice::from_raw_parts(packet,128)};
        if u32::from_le_bytes(packet[48..52].try_into().unwrap())==10 {
            for _ in 0..4 {
                let before=RP1_AMP_COMMAND_RECEIPT[15].load(Ordering::Acquire);
                if before&1!=0 {continue;}
                let mut words=core::array::from_fn::<_,16,_>(|i|RP1_AMP_COMMAND_RECEIPT[i].load(Ordering::Relaxed));
                barrier();
                if before==RP1_AMP_COMMAND_RECEIPT[15].load(Ordering::Acquire) && words[15]==before {
                    if let Some(p)=unsafe {(&*ptr::addr_of!(COMMAND_PRODUCER)).as_ref()} {words[8]=words[8].saturating_add(p.drops());}
                    unsafe {ptr::copy_nonoverlapping(words.as_ptr(),out,16);}
                    return 0;
                }
            }
            return 8;
        }
        let generation=unsafe {ptr::addr_of!(GENERATION).read().assume_init()};
        // Only the owned GPIO22 demonstration output is controlled by parameter0.
        let update=match ControlUpdate::decode(packet,generation,now,|p|
            if p.0.iter().all(|v|*v<=65535) {Ok(())} else {Err(Error::Bounds)}) {
            Ok(u)=>u,Err(e)=>return status(e),
        };
        let Some(producer)=(unsafe {(&mut *ptr::addr_of_mut!(COMMAND_PRODUCER)).as_mut()}) else {return 6;};
        if let Err(e)=producer.push(update) {return status(e);}
        let mut receipt=[0;16];receipt[0]=u32::from_le_bytes(*b"QUE1");receipt[1]=1;
        receipt[2]=update.sequence() as u32;receipt[3]=(update.sequence()>>32) as u32;
        unsafe {ptr::copy_nonoverlapping(receipt.as_ptr(),out,16);}
        0
    }

    fn command_receipt(control:&LocalControl,decision:Decision,sequence:u64,result:u32) {
        let c=control.counters();
        let (active,p)=match decision {Decision::Active{parameters,..}=>(1,parameters.0),_=>(0,[0;4])};
        let clamp=|v:u64|v.min(u64::from(u32::MAX)) as u32;
        let w=[u32::from_le_bytes(*b"CTR1"),1,sequence as u32,(sequence>>32) as u32,result,
            clamp(c.accepted),clamp(c.applied),clamp(c.rejected),clamp(c.queue_full),clamp(c.command_expired),
            active,p[0],p[1],p[2],p[3]];
        let epoch=RP1_AMP_COMMAND_RECEIPT[15].load(Ordering::Relaxed);
        RP1_AMP_COMMAND_RECEIPT[15].store(epoch.wrapping_add(1),Ordering::SeqCst);
        for i in 0..15 {RP1_AMP_COMMAND_RECEIPT[i].store(w[i],Ordering::Relaxed);}
        RP1_AMP_COMMAND_RECEIPT[15].store(epoch.wrapping_add(2),Ordering::Release);
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn AmpProc1Body()->! {
        let generation=unsafe {ptr::addr_of!(GENERATION).read().assume_init()};
        let Some(mut producer)=(unsafe {(&mut *ptr::addr_of_mut!(TRACE_PRODUCER)).take()}) else {park(4)};
        let Some(mut commands)=(unsafe {(&mut *ptr::addr_of_mut!(COMMAND_CONSUMER)).take()}) else {park(4)};
        let Some(mut output)=(unsafe {(&mut *ptr::addr_of_mut!(OUTPUT)).take()}) else {park(4)};
        let Ok(mut control)=LocalControl::new(generation,PERIOD_US) else {park(4)};
        let (msp,control_register,ipsr,vtor):(u32,u32,u32,u32);
        unsafe {core::arch::asm!("mrs {0},MSP","mrs {1},CONTROL","mrs {2},IPSR",
            out(reg)msp,out(reg)control_register,out(reg)ipsr,options(nostack,preserves_flags));
            vtor=(0xe000_ed08 as *const u32).read_volatile();}
        put(2,ipsr);put(3,control_register);put(4,msp);put(5,vtor);put(6,unsafe {CORE.read_volatile()});
        let Some(start)=now() else {park(4)};
        if control_register!=0 || ipsr!=0 || vtor!=ptr::addr_of!(__amp_proc1_vectors) as u32
            || msp&7!=0 || !(stack_low() as u32..stack_top()).contains(&msp)
            || !guards() {park(4);}
        let Some(mut next)=start.checked_add(PERIOD_US) else {park(4)};
        let (mut sequence,mut cycles,mut missed,mut last)=(0u64,0u64,0u64,start);
        let mut guard_remaining = 1000;
        let (mut command_sequence,mut command_result,mut previous)=(0u64,0u32,None);
        #[cfg(feature="freertos-amp-proc1-gpio-irq")]
        if !super::gpio_irq::start(generation) {park(4);}
        prof!(probe());
        put(10,u32::MAX);put(1,1);barrier();put(0,READY);
        loop {
            if STOP.load(Ordering::Acquire)!=0 {control.retire();finish(2);}
            let Some(t)=now() else {control.retire();park(4)};
            if t<last {control.retire();park(4);}
            if t<next {core::hint::spin_loop();continue;}
            prof!(begin((t-next).min(u64::from(u32::MAX)) as u32));
            let Some((following,skipped))=due(next,t) else {control.retire();park(4)};
            next=following;sequence=match sequence.checked_add(skipped+1) {Some(s)=>s,None=>park(4)};
            missed=missed.saturating_add(skipped);cycles+=1;
            put(31,get(31).wrapping_add(1));barrier();
            prof!(mark(0));
            let update=commands.pop();
            if let Some(update)=update {
                command_sequence=update.sequence();
                command_result=control.offer_update(update,t).err().map_or(0,status);
            }
            prof!(mark(1));
            let decision=control.step(t,SampleStamp{boot:generation.boot,sequence,acquired_us:t});
            prof!(mark(2));
            let high=match decision {Decision::Active{parameters,..}=>output_high(true,parameters.0[0]),_=>false};
            if u32::from(high)!=get(30) {
                if high {output.set_high();} else {output.set_low();}
                unsafe {core::arch::asm!("dsb sy",options(nostack,preserves_flags));}
                let latch=unsafe {(0x400e_0000 as *const u32).read_volatile()} & (1<<22);
                if (latch!=0)!=high {control.retire();park(4);}
                put(29,get(29).saturating_add(1));put(30,u32::from(high));
            }
            prof!(mark(3));
            if update.is_some() || previous!=Some(decision) {
                command_receipt(&control,decision,command_sequence,command_result);previous=Some(decision);
            }
            prof!(mark(4));
            let Some(calculation)=now() else {control.retire();park(4)};
            if calculation<t {control.retire();park(4);}
            prof!(mark(5));
            // Unmeasured edge is zero; flags=0 never attests hardware e2e.
            let make_sample=|| Observation{generation,sequence,edge_us:0,irq_us:t,transfer_us:t,
                calculation_us:calculation,actuator_us:calculation,sample_timestamp_rp1:t,
                time_sync_generation:0,time_sync_uncertainty_us:u32::MAX,flags:0};
            #[cfg(feature="freertos-amp-proc1-gpio-irq")]
            let _=producer.push_with(make_sample);
            #[cfg(not(feature="freertos-amp-proc1-gpio-irq"))]
            let _=producer.push(make_sample()); // Explicit drop-newest; first8 intact.
            prof!(mark(6));
            let elapsed=t-last;last=t;
            // One C1 writer, seqlock epoch brackets each multiword telemetry update.
            put(7,cycles as u32);put(8,missed as u32);put(9,producer.drops());
            put(10,get(10).min(elapsed.min(u64::from(u32::MAX)) as u32));
            put(11,get(11).max(elapsed.min(u64::from(u32::MAX)) as u32));
            put(12,sequence as u32);put(13,(sequence>>32) as u32);put(14,t as u32);put(15,(t>>32) as u32);
            put(22,u32::from(matches!(decision,Decision::Active{..})));put(23,control.counters().sample_sequence_gaps as u32);
            prof!(mark(7));
            if guard_due(&mut guard_remaining) {
                if !guards() || unsafe {stack_low().add(31).read_volatile()}!=FILL {
                    control.retire();park(4);
                }
            }
            prof!(mark(8));
            barrier();put(31,get(31).wrapping_add(1));
            prof!(mark(9));
            if cycles>=CYCLES || t-start>=CYCLES*PERIOD_US {control.retire();finish(3);}
            #[cfg(feature="freertos-amp-proc1-gpio-irq")]
            if !super::gpio_irq::force() {control.retire();park(4);}
            prof!(end(cycles as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_requires_active_nonzero_command_and_safe_states_are_low() {
        for value in [0,1,65535,u32::MAX] {
            assert!(!output_high(false,value));
            assert_eq!(output_high(true,value),value!=0);
        }
    }
    #[test]
    fn cold_guard_checks_before_writes_and_never_retries() {
        let mut a=Launch::default();assert!(a.admit(0,1<<31,0,17,29));
        assert!(!a.admit(0,1<<31,0,17,29));
        for (core,ctrl,done,boot,session) in [(1,1<<31,0,17,29),(0,0,0,17,29),
            (0,1<<31,1<<31,17,29),(0,1<<31,0,0,29),(0,1<<31,0,17,0)] {
            let mut a=Launch::default();assert!(!a.admit(core,ctrl,done,boot,session));
            assert!(!a.admit(0,1<<31,0,17,29));
        }
    }
    #[test]
    fn absolute_period_skips_old_deadlines_without_catchup_burst() {
        assert_eq!(due(200,199),None);assert_eq!(due(200,200),Some((400,0)));
        assert_eq!(due(200,399),Some((400,0)));assert_eq!(due(200,400),Some((600,1)));
        assert_eq!(due(200,901),Some((1000,3)));assert_eq!(due(u64::MAX-10,u64::MAX),None);
        let mut next=PERIOD_US;
        for i in 1..=CYCLES {let (n,lost)=due(next,i*PERIOD_US).unwrap();assert_eq!(lost,0);next=n;}
    }
    #[test]
    fn period_fast_path_matches_wide_reference_at_boundaries_and_random_offsets() {
        let reference = |next: u64, now: u64| {
            if now < next { return None; }
            let skipped = u128::from(now - next) / u128::from(PERIOD_US);
            let following = u128::from(next) + (skipped + 1) * u128::from(PERIOD_US);
            Some((u64::try_from(following).ok()?, u64::try_from(skipped).ok()?))
        };
        for next in [0,1,200,u64::from(u32::MAX),u64::MAX-400,u64::MAX-200,u64::MAX-199,u64::MAX] {
            for late in [0,1,199,200,201,399,400,u32::MAX as u64,u64::MAX] {
                if let Some(now) = next.checked_add(late) { assert_eq!(due(next,now),reference(next,now)); }
            }
            if next != 0 { assert_eq!(due(next,next-1),None); }
        }
        let mut random = 0x8fa2_17d9_654c_013bu64;
        for _ in 0..100_000 {
            random ^= random << 13; random ^= random >> 7; random ^= random << 17;
            let next = random;
            random ^= random << 13; random ^= random >> 7; random ^= random << 17;
            assert_eq!(due(next,random),reference(next,random));
        }
    }
    #[test]
    fn guard_countdown_tracks_executed_cycles_even_when_deadline_slots_are_skipped() {
        let mut remaining = 1000;
        let mut next = PERIOD_US;
        let mut sequence = 0;
        for cycle in 1..=CYCLES {
            let skipped = if cycle % 17 == 0 { 3 } else { 0 };
            let (following, lost) = due(next,next+skipped*PERIOD_US).unwrap();
            next = following; sequence += lost + 1;
            assert_eq!(guard_due(&mut remaining),cycle%1000 == 0);
            assert_eq!(remaining,1000-(cycle%1000) as u32);
        }
        assert!(sequence > CYCLES);
    }
    #[test]
    fn scan_timer_rejects_regression_and_width_overflow() {
        assert_eq!(scan_duration(10,10),Some(0));
        assert_eq!(scan_duration(10,12),Some(2));
        assert_eq!(scan_duration(10,9),None);
        assert_eq!(scan_duration(0,u64::from(u32::MAX)),Some(u32::MAX));
        assert_eq!(scan_duration(0,u64::from(u32::MAX)+1),None);
    }
}

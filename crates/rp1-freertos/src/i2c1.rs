//! Proc0 I2C1/IRQ8 single-owner receive. No global IRQ mask/VTOR/reset writer.
//! Task notification0 is reserved during a read; FIFO pops belong to the ISR.
use crate::{self as os, Task};
use core::{cell::{Cell, UnsafeCell}, ptr};
use rp1_hal::{i2c::{self, I2c1Host, I2c1Read1Snapshot},
    i2c_rx_irq_adapter::{self as adapter, ServiceEvidence},
    i2c_rx_state::{CleanupEvidence, Error as RxError, RxState, State, FATAL_CAUSES, OWNED_CAUSES, MAX_LEN}};

const BIT:u32=1<<8;
const ENABLE:*mut u32=0xe000_e100 as *mut u32;
const DISABLE:*mut u32=0xe000_e180 as *mut u32;
const PENDING:*mut u32=0xe000_e200 as *mut u32;
const CLEAR:*mut u32=0xe000_e280 as *mut u32;
const ACTIVE_IRQ:*mut u32=0xe000_e300 as *mut u32;
static mut ACTIVE:*mut Context=ptr::null_mut();
static mut GENERATION:u32=0;
static mut WAITER:u32=0;
static mut CANCEL:u32=0;

fn barrier() { unsafe { core::arch::asm!("dsb sy", "isb", options(nostack)); } }
fn mask() { unsafe { DISABLE.write_volatile(BIT); } barrier(); }
fn raw()->u32 { unsafe { (0x400a_c028 as *const u32).read_volatile() } }
fn causes(s:I2c1Read1Snapshot)->u32 { s.irq.raw_interrupt_status|s.irq.masked_interrupt_status }
fn clean(s:I2c1Read1Snapshot)->bool {
    s.irq.enable_status==0 && s.irq.interrupt_mask==0 && s.rx_level==0 && s.tx_level==0
        && causes(s)&OWNED_CAUSES==0 && s.irq.masked_interrupt_status==0 && s.irq.abort_source==0
}

#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Error { InvalidArgument, Cancelled, Timeout, Receive(RxError), IrqBudget }

#[derive(Clone,Copy,Debug,Default)]
pub struct Receipt {
    pub generation:u32, pub irq_entries:u32, pub received:u32, pub elapsed_us:u32,
    pub irq_body_max_us:u32, pub irq_end_to_task_us:u32,
    pub first_fatal_causes:u32, pub first_abort_source:u32,
    pub discarded_after_failure:u32, pub cleanup_elapsed_us:u32,
    pub quiet_samples:u32, pub quiet_max_gap_us:u32,
    /// Scheduling hints, not successful-notification count; zero is legal.
    pub higher_priority_wakes:u32,
}

struct Context {
    host:I2c1Host, engine:RxState, rx_depth:u32, receipt:Receipt,
    irq_end:u32, terminal_generation:u32, no_progress:u32, budget_failed:bool,
}
impl Context {
    fn retain(&mut self,s:I2c1Read1Snapshot) {
        let bits=causes(s)&FATAL_CAUSES;
        if bits!=0 || s.irq.abort_source!=0 {
            if self.receipt.first_fatal_causes==0 && self.receipt.first_abort_source==0 {
                self.receipt.first_fatal_causes=bits;
                self.receipt.first_abort_source=s.irq.abort_source;
            }
        }
    }
    fn service_evidence(&mut self,e:ServiceEvidence) {
        self.retain(e.entry);
        if let Some(s)=e.first_fatal { self.retain(s); }
        self.retain(e.last);
        if e.fault.is_some() || e.rejected.is_some() {
            self.budget_failed=true;
            if self.engine.state()==State::Active {
                self.engine.fail_adapter(self.engine.generation()).unwrap();
            }
        }
    }
}

/// Owns host, pins and engine. IRQ storage never points at the caller's buffer.
/// UnsafeCell explicitly identifies the serialized task/IRQ shared state.
/// Use shared references across interruptible calls, including outer callers;
/// UnsafeCell does not relax an enclosing &mut Driver's uniqueness.
pub struct Driver { context:UnsafeCell<Context>, last:Cell<Option<Receipt>> }
impl Driver {
    /// # Safety
    /// Proc0 task only; exclusive I2C1/IRQ8/pins ownership and known clocks/reset.
    /// Install on_interrupt directly in vector24. No other task/core/host writer.
    pub unsafe fn new(host:I2c1Host)->Self {
        assert_eq!(unsafe { ENABLE.read_volatile() }&BIT,0);
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        let observed=host.snapshot(0,0);
        let rx_depth=i2c::i2c1_rx_fifo_depth(observed.component_parameter);
        assert!(rx_depth==32 && observed.tx_fifo_depth==32);
        unsafe { (0xe000_e408 as *mut u8).write_volatile(0xc0); }
        barrier();
        assert_eq!(unsafe { (0xe000_e408 as *const u8).read_volatile() },0xc0);
        Self { context:UnsafeCell::new(Context { host,engine:RxState::new(rx_depth,observed.tx_fifo_depth).unwrap(),
            rx_depth:u32::from(rx_depth),receipt:Receipt::default(),irq_end:0,terminal_generation:0,
            no_progress:0,budget_failed:false }),last:Cell::new(None) }
    }

    pub fn last_receipt(&self)->Option<Receipt> { self.last.get() }

    /// Mask/disable/retain/ACK then sample disabled-source quiet over >=4ms.
    /// Sleep between samples: this is NOT the earlier proof's tight polling.
    /// Any cleanup failure halts; do not release host/engine/buffer for rearm.
    unsafe fn cleanup(&self,failed:bool) {
        mask();
        let c=unsafe { &mut *self.context.get() };
        let started=raw();
        i2c::i2c1_mask_read1_irq();c.retain(i2c::i2c1_read1_snapshot());
        i2c::i2c1_disable_read1_irq().expect("I2C disable failed; recovery required");
        let before_ack=i2c::i2c1_read1_snapshot();c.retain(before_ack);
        assert_eq!(before_ack.irq.enable_status,0);
        if before_ack.rx_level!=0 {
            assert!(failed && before_ack.rx_level<=c.rx_depth);
            c.receipt.discarded_after_failure=i2c::i2c1_read1_discard_residual(c.rx_depth);
        }
        i2c::i2c1_ack_read1_irq(before_ack);
        assert!(clean(i2c::i2c1_read1_snapshot()),"I2C post-ACK not clean");
        assert_eq!(unsafe { ENABLE.read_volatile()|ACTIVE_IRQ.read_volatile() }&BIT,0);
        unsafe { CLEAR.write_volatile(BIT); } barrier();
        let start=raw();let mut previous=start;let mut samples=0;let mut max_gap=0;
        let count=c.receipt.irq_entries;
        loop {
            let s=i2c::i2c1_read1_snapshot();let now=raw();
            assert!(clean(s));
            assert_eq!(unsafe { ENABLE.read_volatile()|PENDING.read_volatile()|ACTIVE_IRQ.read_volatile() }&BIT,0);
            assert_eq!(count,c.receipt.irq_entries);
            samples+=1;max_gap=max_gap.max(now.wrapping_sub(previous));previous=now;
            let elapsed=now.wrapping_sub(start);
            if elapsed>=4000 {
                assert!(elapsed<20_000 && max_gap<10_000);
                c.engine.record_cleanup(c.engine.generation(),CleanupEvidence {
                    enable_status:s.irq.enable_status,interrupt_mask:s.irq.interrupt_mask,
                    rx_level:s.rx_level,tx_level:s.tx_level,raw_status:s.irq.raw_interrupt_status,
                    masked_status:s.irq.masked_interrupt_status,abort_source:s.irq.abort_source,
                    owned_irq_enabled:false,owned_irq_pending:false,owned_irq_active:false,
                    irq_count_before:count,irq_count_after:c.receipt.irq_entries,
                    quiet_conditions_held:true,quiet_elapsed_ticks:elapsed,quiet_required_ticks:4000,
                    quiet_samples:samples,quiet_max_gap_ticks:max_gap,
                }).expect("I2C sampled cleanup rejected");
                break;
            }
            unsafe { os::delay(1).unwrap(); }
        }
        c.receipt.cleanup_elapsed_us=raw().wrapping_sub(started);
        c.receipt.quiet_samples=samples;c.receipt.quiet_max_gap_us=max_gap;
    }

    /// # Safety
    /// Running proc0 task, same owner contract as new, notification0 reserved.
    /// No enclosing exclusive Driver borrow or concurrent/reentrant driver call.
    /// Only after masked checked cleanup is the actual prefix copied to rx.
    pub unsafe fn receive(&self,address:u8,rx:&mut[u8],timeout:u32)->Result<Receipt,Error> {
        self.last.set(None);
        if address>0x7f || rx.is_empty() || rx.len()>MAX_LEN || timeout==0 || timeout>=0x8000_0000 {
            return Err(Error::InvalidArgument);
        }
        mask();
        assert!(unsafe { ptr::addr_of!(ACTIVE).read_volatile() }.is_null());
        unsafe { os::notification_take(true,0).unwrap(); }
        if !unsafe { &*self.context.get() }.engine.is_clean() { unsafe { self.cleanup(false); } }
        let c=unsafe { &mut *self.context.get() };
        c.receipt=Receipt::default();c.irq_end=0;c.terminal_generation=0;c.no_progress=0;c.budget_failed=false;
        assert!(clean(i2c::i2c1_read1_snapshot()));
        assert_eq!(unsafe { PENDING.read_volatile()|ACTIVE_IRQ.read_volatile() }&BIT,0);
        unsafe { c.host.enable_local_irq_route() }.expect("I2C route prerequisite");
        let generation=c.engine.arm_read(rx.len()).unwrap();c.receipt.generation=generation;
        let started=raw();let deadline=unsafe { os::tick().unwrap() }.wrapping_add(timeout);
        c.host.arm_rx_irq_preserving_causes(address).expect("I2C arm failed; recovery required");
        let waiter=unsafe { os::current_task().unwrap().unwrap() };
        unsafe {
            ptr::addr_of_mut!(CANCEL).write_volatile(0);
            ptr::addr_of_mut!(WAITER).write_volatile(waiter.id());
            ptr::addr_of_mut!(ACTIVE).write_volatile(self.context.get());
            // Commit marker last: a preempting canceller must never see a new
            // generation paired with the previous (zero) waiter.
            ptr::addr_of_mut!(GENERATION).write_volatile(generation);
        }
        let e=adapter::prime(&mut c.host,&mut c.engine,generation);c.service_evidence(e);
        let mut result=loop {
            let c=unsafe { &mut *self.context.get() };
            if c.terminal_generation==generation {
                c.receipt.irq_end_to_task_us=raw().wrapping_sub(c.irq_end);
            }
            if let Some(error)=c.engine.primary_error() {
                break Err(if error==RxError::Adapter && c.budget_failed { Error::IrqBudget }
                    else { Error::Receive(error) });
            }
            if c.budget_failed { break Err(Error::IrqBudget); }
            if c.engine.state()==State::Complete {
                assert_eq!(c.terminal_generation,generation,"completion without terminal IRQ");
                break Ok(());
            }
            if unsafe { ptr::addr_of!(CANCEL).read_volatile() }==generation {
                c.engine.cancel(generation).unwrap();break Err(Error::Cancelled);
            }
            let Some(left)=os::deadline_remaining(unsafe { os::tick().unwrap() },deadline) else {
                let s=i2c::i2c1_read1_snapshot();c.retain(s);
                if causes(s)&FATAL_CAUSES!=0 || s.irq.abort_source!=0 {
                    c.engine.observe(generation,causes(s),s.irq.abort_source).unwrap();
                    break Err(Error::Receive(c.engine.primary_error().unwrap()));
                }
                c.engine.timeout(generation).unwrap();break Err(Error::Timeout);
            };
            unsafe { ENABLE.write_volatile(BIT); } barrier();
            unsafe { os::notification_take(true,left).unwrap(); } mask();
        };
        // Stop accepting cancellation BEFORE cleanup blocks. A terminal request
        // cannot be changed by a new cancel during its sampled quiet interval.
        mask();
        #[cfg(feature = "i2c1-cancel-window-probe")]
        if result.is_ok() && generation == 2 {
            // Test-only terminal-complete / PRE-cleanup scheduling window.
            // Caller buffer still belongs to this owner and is not copied yet.
            unsafe extern "C" { fn rp1_i2c_cancel_window_probe(generation:u32); }
            unsafe { rp1_i2c_cancel_window_probe(generation); }
        }
        unsafe {
            ptr::addr_of_mut!(GENERATION).write_volatile(0);
            ptr::addr_of_mut!(ACTIVE).write_volatile(ptr::null_mut());
            ptr::addr_of_mut!(WAITER).write_volatile(0);
        }
        // The C cancellation transaction cannot cross this withdrawal. Retain
        // any cancel accepted after Complete was observed but before it closed.
        if result.is_ok() && unsafe { ptr::addr_of!(CANCEL).read_volatile() } == generation {
            result = Err(Error::Cancelled);
        }
        barrier();unsafe { os::notification_take(true,0).unwrap(); }
        unsafe { self.cleanup(result.is_err()); }
        unsafe { os::notification_take(true,0).unwrap(); }
        let c=unsafe { &mut *self.context.get() };
        // A fatal cause first observed during cleanup cannot become successful
        // payload completion merely because its selective ACK cleared the bit.
        if matches!(result, Ok(()) | Err(Error::Cancelled)) && (c.receipt.first_fatal_causes!=0 || c.receipt.first_abort_source!=0) {
            result=Err(Error::Receive(RxError::Fatal {
                causes:c.receipt.first_fatal_causes,abort_source:c.receipt.first_abort_source,
            }));
        }
        let bytes=c.engine.bytes();rx[..bytes.len()].copy_from_slice(bytes);
        c.receipt.received=bytes.len() as u32;c.receipt.elapsed_us=raw().wrapping_sub(started);
        self.last.set(Some(c.receipt));result.map(|()| c.receipt)
    }
}

/// # Safety
/// Proc0 task context only. Zero means no published request.
pub unsafe fn active_generation()->u32 { unsafe { ptr::addr_of!(GENERATION).read_volatile() } }
/// # Safety
/// Proc0 task context; shared notification0 is owned by this driver.
pub unsafe fn cancel(generation:u32)->bool {
    os::value(unsafe { os::ffi::rp1_freertos_cancel_notification(generation,ptr::addr_of!(GENERATION),
        ptr::addr_of_mut!(CANCEL),ptr::addr_of!(WAITER)) }).expect("I2C cancellation failed")==1
}

/// # Safety
/// Direct proc0 IRQ8/IPSR24 at logical6. Never call as a foreground poll.
pub unsafe fn on_interrupt() {
    let started=raw();let ipsr:u32;
    unsafe { core::arch::asm!("mrs {}, IPSR",out(reg) ipsr,options(nomem,nostack)); }
    assert_eq!(ipsr,24);
    let p=unsafe { ptr::addr_of!(ACTIVE).read_volatile() };
    if p.is_null() { mask();return; }
    let c=unsafe { &mut *p };let generation=c.engine.generation();
    c.receipt.irq_entries=c.receipt.irq_entries.saturating_add(1);
    if c.receipt.irq_entries>4*MAX_LEN as u32+8 || c.engine.state()!=State::Active {
        c.budget_failed=true;
        if c.engine.state()==State::Active { c.engine.fail_adapter(generation).unwrap(); }
    } else {
        let before=(c.engine.counts(),c.engine.stop_seen());
        let e=adapter::service_irq(&mut c.host,&mut c.engine,generation);c.service_evidence(e);
        if before==(c.engine.counts(),c.engine.stop_seen()) { c.no_progress+=1; } else { c.no_progress=0; }
        if c.no_progress>=4 && c.engine.state()==State::Active {
            c.budget_failed=true;c.engine.fail_adapter(generation).unwrap();
        }
    }
    if c.budget_failed || c.engine.state()!=State::Active {
        i2c::i2c1_mask_read1_irq();mask();c.terminal_generation=generation;
        let waiter=unsafe { ptr::addr_of!(WAITER).read_volatile() };
        if waiter!=0 && unsafe { Task(waiter).notification_give_from_isr_woken().unwrap() } {
            c.receipt.higher_priority_wakes+=1;
        }
    }
    c.irq_end=raw();c.receipt.irq_body_max_us=c.receipt.irq_body_max_us.max(c.irq_end.wrapping_sub(started));
}

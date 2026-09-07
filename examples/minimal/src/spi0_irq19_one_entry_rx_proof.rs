#![cfg_attr(not(target_arch = "arm"), allow(dead_code, unused_imports))]

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering, compiler_fence};

#[cfg(not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
pub const MAGIC: u32 = u32::from_le_bytes(*b"S0I2");
#[cfg(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))]
pub const MAGIC: u32 = u32::from_le_bytes(*b"S0D2");
pub const IRQ_NUMBER: u32 = 19;
pub const VECTOR_INDEX: u32 = 35;
pub const WORDS: usize = 16;
const UNAVAILABLE: u32 = u32::MAX;
const RXFI: u32 = 1 << 4;
const ERRORS: u32 = 0x0e;
const RX_MASK: u32 = RXFI | ERRORS;
const IRQ_BIT: u32 = 1 << IRQ_NUMBER;
const INHERITED_PENDING1: u32 = 1 << 21;
const WAIT_US: u64 = 4_000;

#[cfg(all(target_arch = "arm", any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
static FIFO_LEN: AtomicU32 = AtomicU32::new(0);

#[cfg(target_arch = "arm")]
#[inline(always)]
fn expected_len() -> u32 {
    #[cfg(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))]
    {
        FIFO_LEN.load(Ordering::Relaxed)
    }
    #[cfg(not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
    {
        1
    }
}

const FLAG_PREPARED: u32 = 1 << 0;
const FLAG_WRAPPER: u32 = 1 << 1;
const FLAG_SOURCE: u32 = 1 << 2;
const FLAG_ENABLED: u32 = 1 << 3;
const FLAG_ENTERED: u32 = 1 << 4;
const FLAG_MASKED: u32 = 1 << 5;
const FLAG_FINISHED: u32 = 1 << 6;
const FLAG_PRIMASK_RESTORED: u32 = 1 << 7;
const FLAG_VTOR_SAME: u32 = 1 << 8;
const FLAG_UNRELATED_UNCHANGED: u32 = 1 << 9;
const FLAG_NO_STORM: u32 = 1 << 10;
const PASS_FLAGS: u32 = FLAG_PREPARED
    | FLAG_WRAPPER
    | FLAG_SOURCE
    | FLAG_ENABLED
    | FLAG_ENTERED
    | FLAG_MASKED
    | FLAG_FINISHED
    | FLAG_PRIMASK_RESTORED
    | FLAG_VTOR_SAME
    | FLAG_UNRELATED_UNCHANGED
    | FLAG_NO_STORM;

const FAIL_SETUP: u32 = 0x380;
const FAIL_PREPARE: u32 = 0x381;
const FAIL_SPI: u32 = 0x382;
const FAIL_WRAPPER_PRE: u32 = 0x383;
const FAIL_WRAPPER_POST: u32 = 0x384;
const FAIL_SOURCE_TIMEOUT: u32 = 0x385;
const FAIL_ENABLE_PRESTATE: u32 = 0x386;
const FAIL_IRQ_TIMEOUT: u32 = 0x387;
const FAIL_FINISH: u32 = 0x388;
const FAIL_FINAL: u32 = 0x389;
const FAIL_HANDLER_REPLAY: u32 = 0x38a;
const FAIL_HANDLER_IPSR: u32 = 0x38b;
const FAIL_HANDLER_SOURCE: u32 = 0x38c;
const FAIL_HANDLER_NO_TRANSFER: u32 = 0x38d;
const FAIL_ABORT_CLEANUP: u32 = 0x38e;
const FAIL_HANDLER_STATE: u32 = 0x390;

#[cfg(target_arch = "arm")]
static SLOT: TransferSlot = TransferSlot::new();
#[cfg(target_arch = "arm")]
static COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static FIRST_IPSR: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static FIRST_RISR: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static FIRST_ISR: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static FIRST_RXFLR: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "arm")]
static RECEIVED: AtomicU32 = AtomicU32::new(UNAVAILABLE);
#[cfg(target_arch = "arm")]
static HANDLER_ROUTE: AtomicU32 = AtomicU32::new(UNAVAILABLE);
#[cfg(target_arch = "arm")]
static HANDLER_ERROR: AtomicU32 = AtomicU32::new(0);

#[cfg(target_arch = "arm")]
pub(super) struct TransferSlot {
    ptr: UnsafeCell<usize>,
    armed: AtomicU32,
}

#[cfg(target_arch = "arm")]
unsafe impl Sync for TransferSlot {}

#[cfg(target_arch = "arm")]
impl TransferSlot {
    pub(super) const fn new() -> Self {
        Self {
            ptr: UnsafeCell::new(0),
            armed: AtomicU32::new(0),
        }
    }

    /// Unsafe bridge invariant: while `armed == 1`, foreground does not borrow
    /// or access the transfer. Host, TX/RX buffers and the transfer live until
    /// IRQ19 is masked, barriers complete, and `withdraw()` runs. The ISR is
    /// the only mutator during that interval; it masks IRQ19 before service.
    pub(super) unsafe fn publish(&self, transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>) {
        unsafe { *self.ptr.get() = transfer as *mut _ as usize };
        compiler_fence(Ordering::Release);
        self.armed.store(1, Ordering::Release);
    }

    pub(super) unsafe fn withdraw(&self) {
        self.armed.store(0, Ordering::Release);
        compiler_fence(Ordering::SeqCst);
        unsafe { *self.ptr.get() = 0 };
    }

    pub(super) unsafe fn get_for_isr(&self) -> Option<*mut rp1_hal::spi::Spi0IrqTransfer<'_>> {
        if self.armed.load(Ordering::Acquire) != 1 {
            return None;
        }
        compiler_fence(Ordering::Acquire);
        let ptr = unsafe { *self.ptr.get() } as *mut rp1_hal::spi::Spi0IrqTransfer<'_>;
        (!ptr.is_null()).then_some(ptr)
    }
}

#[cfg(target_arch = "arm")]
#[derive(Clone, Copy)]
struct Spi {
    irq: rp1_hal::spi::Spi0IrqSnapshot,
    rx: u32,
    control: u32,
    selected: u32,
    baud: u32,
    rx_threshold: u32,
}

#[cfg(target_arch = "arm")]
#[derive(Clone, Copy)]
struct Telemetry {
    flags: u32,
    stage: u32,
    wrapper_pre: u32,
    wrapper_post: u32,
    wrapper_final: u32,
    before_route: u32,
    source_route: u32,
    final_route: u32,
}

#[cfg(target_arch = "arm")]
impl Telemetry {
    fn new() -> Self {
        Self {
            flags: 0,
            stage: 0,
            wrapper_pre: UNAVAILABLE,
            wrapper_post: UNAVAILABLE,
            wrapper_final: UNAVAILABLE,
            before_route: UNAVAILABLE,
            source_route: UNAVAILABLE,
            final_route: UNAVAILABLE,
        }
    }
}

#[cfg(target_arch = "arm")]
fn read32(offset: usize) -> u32 {
    unsafe { core::ptr::read_volatile((rp1_hal::addr::SPI0_BASE + offset) as *const u32) }
}

#[cfg(target_arch = "arm")]
fn snapshot() -> Spi {
    Spi {
        irq: rp1_hal::spi::spi0_irq_snapshot(),
        rx: read32(0x24),
        control: read32(0x00),
        selected: read32(0x10),
        baud: read32(0x14),
        rx_threshold: read32(0x1c),
    }
}

#[cfg(target_arch = "arm")]
fn read_wrapper() -> u32 {
    read32(0x108)
}

#[cfg(all(target_arch = "arm", any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
pub fn fifo_api_snapshot() -> ([u32; 14], [u32; 8]) {
    let s = snapshot();
    let wrapper = read_wrapper();
    let r = rp1_rt::spi0_irq_route_snapshot();
    (
        [
            s.irq.version,
            s.irq.enable,
            s.irq.tx_fifo_threshold,
            s.irq.interrupt_mask,
            s.irq.raw_interrupt_status,
            s.irq.masked_interrupt_status,
            s.irq.tx_fifo_level,
            s.irq.status,
            s.rx,
            s.control,
            s.selected,
            s.baud,
            s.rx_threshold,
            wrapper,
        ],
        [
            r.vtor, r.iser0, r.iser1, r.ispr0, r.ispr1, r.iabr0, r.iabr1, r.primask,
        ],
    )
}

#[cfg(target_arch = "arm")]
fn prepared_state(s: Spi) -> bool {
    s.control == 7 << 16
        && s.baud == 2_000
        && s.rx_threshold == 0
        && s.irq.enable == 1
        && s.selected == 0
        && s.irq.interrupt_mask == 0
        && s.irq.masked_interrupt_status == 0
        && s.irq.raw_interrupt_status & (ERRORS | RXFI) == 0
        && s.rx == 0
        && s.irq.tx_fifo_level == expected_len()
}

#[cfg(target_arch = "arm")]
fn source_state(s: Spi) -> bool {
    s.control == 7 << 16
        && s.baud == 2_000
        && s.rx_threshold == 0
        && s.irq.enable == 1
        && s.selected == 1
        && s.irq.interrupt_mask == RX_MASK
        && s.irq.raw_interrupt_status == RXFI | 1
        && s.irq.masked_interrupt_status == RXFI
        && s.rx == expected_len()
        && s.irq.tx_fifo_level == 0
        && s.irq.status & 5 == 4
}

fn no_storm_ok(timer_progressed: bool, count_after_mask: u32, count_after_interval: u32) -> bool {
    timer_progressed && count_after_mask == 1 && count_after_interval == 1
}

#[cfg(all(target_arch = "arm", any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
fn wait_fifo_source(mut ready: impl FnMut() -> bool) -> bool {
    const LOW: *const u32 = 0x400a_c028 as *const u32;
    let start = unsafe { core::ptr::read_volatile(LOW) };
    // Existing timer only; first bound wins, including finite stalled-timer cap.
    for _ in 0..12_500_000 {
        if unsafe { core::ptr::read_volatile(LOW) }.wrapping_sub(start) >= 50_000 {
            return false;
        }
        if ready() {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

fn primask_unchanged(actual: u32, expected: u32) -> bool {
    expected <= 1 && actual == expected
}

// ponytail: one IRQ19 writer while active; this is not a multi-writer CAS.
// Keep plain atomic accesses: RP1 shared-SRAM STREX success is not guaranteed.
fn record_first_error(error: &AtomicU32, code: u32) {
    if error.load(Ordering::Relaxed) == 0 {
        error.store(code, Ordering::Relaxed);
    }
}

fn handler_precheck(
    old_count: u32,
    ipsr: u32,
    masked_status: u32,
    has_transfer: bool,
) -> Result<(), u32> {
    if old_count != 0 {
        return Err(FAIL_HANDLER_REPLAY);
    }
    if ipsr != VECTOR_INDEX {
        return Err(FAIL_HANDLER_IPSR);
    }
    if masked_status == 0 || masked_status & !RX_MASK != 0 {
        return Err(FAIL_HANDLER_SOURCE);
    }
    if !has_transfer {
        return Err(FAIL_HANDLER_NO_TRANSFER);
    }
    Ok(())
}

#[cfg(target_arch = "arm")]
fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    const LOW: *const u32 = 0x400a_c028 as *const u32;
    let start = unsafe { core::ptr::read_volatile(LOW) };
    for _ in 0..1_000_000 {
        if f() {
            return true;
        }
        if unsafe { core::ptr::read_volatile(LOW) }.wrapping_sub(start) >= WAIT_US as u32 {
            return false;
        }
        core::hint::spin_loop();
    }
    false
}

#[cfg(target_arch = "arm")]
fn wait_stable_count(count_after_mask: u32) -> bool {
    const LOW: *const u32 = 0x400a_c028 as *const u32;
    let start = unsafe { core::ptr::read_volatile(LOW) };
    for _ in 0..1_000_000 {
        if unsafe { core::ptr::read_volatile(LOW) }.wrapping_sub(start) >= WAIT_US as u32 {
            return no_storm_ok(true, count_after_mask, COUNT.load(Ordering::Relaxed));
        }
        core::hint::spin_loop();
    }
    false
}

#[cfg(target_arch = "arm")]
fn route_pack(r: rp1_rt::Spi0IrqRouteSnapshot) -> u32 {
    u32::from(r.iser0 & IRQ_BIT != 0)
        | (u32::from(r.ispr0 & IRQ_BIT != 0) << 1)
        | (u32::from(r.iabr0 & IRQ_BIT != 0) << 2)
        | ((r.primask & 1) << 3)
        | (u32::from(r.vtor == 0x2000_0000) << 4)
        | (u32::from(r.iser0 & !IRQ_BIT != 0) << 8)
        | (u32::from(r.iser1 != 0) << 9)
        | (u32::from(r.ispr0 & !IRQ_BIT != 0) << 10)
        | (u32::from(r.ispr1 & !INHERITED_PENDING1 != 0) << 11)
        | (u32::from(r.iabr0 & !IRQ_BIT != 0) << 12)
        | (u32::from(r.iabr1 != 0) << 13)
        | (u32::from(r.ispr1 & INHERITED_PENDING1 != 0) << 14)
}

#[cfg(target_arch = "arm")]
fn route_exact(r: rp1_rt::Spi0IrqRouteSnapshot, expected_primask: u32) -> bool {
    primask_unchanged(r.primask, expected_primask)
        && r.vtor == 0x2000_0000
        && r.iser0 == 0
        && r.iser1 == 0
        && r.ispr0 == 0
        && r.ispr1 == INHERITED_PENDING1
        && r.iabr0 == 0
        && r.iabr1 == 0
}

#[cfg(target_arch = "arm")]
fn source_route_exact(r: rp1_rt::Spi0IrqRouteSnapshot, expected_primask: u32) -> bool {
    primask_unchanged(r.primask, expected_primask)
        && r.vtor == 0x2000_0000
        && r.iser0 == 0
        && r.iser1 == 0
        && r.ispr0 == IRQ_BIT
        && r.ispr1 == INHERITED_PENDING1
        && r.iabr0 == 0
        && r.iabr1 == 0
}

#[cfg(target_arch = "arm")]
fn publish(decision: u32, t: Telemetry) -> u32 {
    const _: () = assert!(WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    let words = [
        0,
        decision,
        t.wrapper_pre,
        t.wrapper_post,
        COUNT.load(Ordering::Relaxed),
        FIRST_IPSR.load(Ordering::Relaxed),
        FIRST_RISR.load(Ordering::Relaxed),
        FIRST_ISR.load(Ordering::Relaxed),
        FIRST_RXFLR.load(Ordering::Relaxed),
        RECEIVED.load(Ordering::Relaxed),
        t.wrapper_final,
        t.before_route,
        t.source_route,
        HANDLER_ROUTE.load(Ordering::Relaxed),
        t.final_route,
        t.flags | ((t.stage & 3) << 16),
    ];
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    decision
}

#[cfg(target_arch = "arm")]
fn abort_record(transfer: &mut rp1_hal::spi::Spi0IrqTransfer<'_>, fallback: u32) -> u32 {
    if transfer.abort().is_ok() {
        fallback
    } else {
        FAIL_ABORT_CLEANUP
    }
}

#[cfg(target_arch = "arm")]
fn state_code(state: rp1_hal::spi::Spi0RxState) -> u32 {
    match state {
        rp1_hal::spi::Spi0RxState::Prepared => 1,
        rp1_hal::spi::Spi0RxState::Active => 2,
        rp1_hal::spi::Spi0RxState::RxComplete => 3,
        rp1_hal::spi::Spi0RxState::Complete => 4,
        rp1_hal::spi::Spi0RxState::Failed(_) => 5,
    }
}

#[cfg(target_arch = "arm")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SPI0_IRQHandler() {
    #[cfg(feature = "spi0-rx-overflow-irq-proof")]
    if crate::spi0_rx_overflow_irq_proof::error_round_active() {
        unsafe { crate::spi0_rx_overflow_irq_proof::on_irq() };
        return;
    }
    let ipsr: u32;
    unsafe {
        core::arch::asm!("mrs {}, IPSR", out(reg) ipsr, options(nomem, nostack, preserves_flags));
        rp1_rt::mask_spi0_irq19_one_entry();
    }

    let route = rp1_rt::spi0_irq_route_snapshot();
    let spi = snapshot();
    // Foreground resets only before enable; IRQ19 cannot preempt itself.
    // An exclusive RMW retry here would make handler progress unbounded.
    let old = COUNT.load(Ordering::Relaxed);
    COUNT.store(old.wrapping_add(1), Ordering::Relaxed);
    // Entry count, not the sampled value, owns the first record. A real zero
    // source on the first (failed) entry must survive a later replay.
    if old == 0 {
        FIRST_IPSR.store(ipsr, Ordering::Relaxed);
        FIRST_RISR.store(spi.irq.raw_interrupt_status, Ordering::Relaxed);
        FIRST_ISR.store(spi.irq.masked_interrupt_status, Ordering::Relaxed);
        FIRST_RXFLR.store(spi.rx, Ordering::Relaxed);
        HANDLER_ROUTE.store(route_pack(route), Ordering::Relaxed);
    }

    let ptr = unsafe { SLOT.get_for_isr() };
    if let Err(code) = handler_precheck(old, ipsr, spi.irq.masked_interrupt_status, ptr.is_some()) {
        record_first_error(&HANDLER_ERROR, code);
        return;
    }

    let transfer = unsafe { &mut *ptr.unwrap() };
    let state = transfer.on_interrupt();
    let received = transfer.received();
    RECEIVED.store(
        received.first().map_or(UNAVAILABLE, |byte| {
            ((received.len() as u32) << 16) | u32::from(*byte)
        }),
        Ordering::Relaxed,
    );
    if !matches!(state, rp1_hal::spi::Spi0RxState::RxComplete) {
        record_first_error(&HANDLER_ERROR, FAIL_HANDLER_STATE + state_code(state));
    }
}

#[cfg(target_arch = "arm")]
pub fn publish_setup_error(code: u32) {
    let mut t = Telemetry::new();
    t.before_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
    publish(code, t);
}

#[cfg(all(target_arch = "arm", not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof", feature = "spi0-retained-rx-ser-proof", feature = "spi0-rx-overflow-irq-proof"))))]
pub fn run(host: &mut rp1_hal::spi::Spi0Host) -> u32 {
    run_with_wrapper::<false>(host)
}

#[cfg(all(target_arch = "arm", feature = "spi0-retained-rx-ser-proof"))]
pub fn run_retained_rearm(host: &mut rp1_hal::spi::Spi0Host, rx: &mut [u8; 1]) -> u32 {
    run_with_wrapper::<false>(host, rx)
}

#[cfg(all(target_arch = "arm", feature = "spi0-rx-overflow-irq-proof"))]
pub fn run_overflow_rearm(host: &mut rp1_hal::spi::Spi0Host, rx: &mut [u8; 1]) -> u32 {
    run_with_wrapper::<true>(host, rx)
}

#[cfg(all(
    target_arch = "arm",
    feature = "spi0-low-high-irq-rearm-proof",
    not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))
))]
pub fn run_rearmed(host: &mut rp1_hal::spi::Spi0Host) -> u32 {
    run_with_wrapper::<true>(host)
}

#[cfg(all(target_arch = "arm", any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
pub fn run_fifo_low(host: &mut rp1_hal::spi::Spi0Host, rx: &mut [u8]) -> u32 {
    run_with_wrapper::<false>(host, rx)
}

#[cfg(all(target_arch = "arm", any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
pub fn run_fifo_high(host: &mut rp1_hal::spi::Spi0Host, rx: &mut [u8]) -> u32 {
    run_with_wrapper::<true>(host, rx)
}

#[cfg(target_arch = "arm")]
#[inline(always)]
fn run_with_wrapper<const REARMED: bool>(
    host: &mut rp1_hal::spi::Spi0Host,
    #[cfg(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))] rx: &mut [u8],
    #[cfg(any(feature = "spi0-retained-rx-ser-proof", feature = "spi0-rx-overflow-irq-proof"))] rx: &mut [u8; 1],
) -> u32 {
    unsafe { SLOT.withdraw() };
    COUNT.store(0, Ordering::Relaxed);
    FIRST_IPSR.store(0, Ordering::Relaxed);
    FIRST_RISR.store(0, Ordering::Relaxed);
    FIRST_ISR.store(0, Ordering::Relaxed);
    FIRST_RXFLR.store(0, Ordering::Relaxed);
    RECEIVED.store(UNAVAILABLE, Ordering::Relaxed);
    HANDLER_ROUTE.store(UNAVAILABLE, Ordering::Relaxed);
    HANDLER_ERROR.store(0, Ordering::Relaxed);

    let mut t = Telemetry::new();
    #[cfg(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))]
    {
        if !(2..=255).contains(&rx.len()) {
            return publish(FAIL_SETUP, t);
        }
        FIFO_LEN.store(rx.len() as u32, Ordering::Relaxed);
    }
    publish(FAIL_SETUP, t);
    let before = rp1_rt::spi0_irq_route_snapshot();
    t.before_route = route_pack(before);
    if !route_exact(before, before.primask) {
        return publish(FAIL_PREPARE, t);
    }
    let saved = match unsafe { rp1_rt::prepare_spi0_irq19_one_entry(before.primask) } {
        Some(saved) => saved,
        None => return publish(FAIL_PREPARE, t),
    };
    t.flags |= FLAG_PREPARED;

    #[cfg(not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof", feature = "spi0-retained-rx-ser-proof", feature = "spi0-rx-overflow-irq-proof")))]
    let mut rx = [0; 1];
    let mut transfer = match {
        #[cfg(not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof", feature = "spi0-retained-rx-ser-proof", feature = "spi0-rx-overflow-irq-proof")))]
        {
            host.prepare_irq_transfer(&[0xa5], &mut rx)
        }
        #[cfg(any(feature = "spi0-retained-rx-ser-proof", feature = "spi0-rx-overflow-irq-proof"))]
        {
            host.prepare_irq_transfer(&[0xa5], rx)
        }
        #[cfg(feature = "spi0-fifo-capacity-irq-proof")]
        {
            host.prepare_irq_transfer(&[0xa5; 256][..rx.len()], rx)
        }
        #[cfg(feature = "spi0-varied-peer-irq-proof")]
        {
            host.prepare_irq_transfer(&[0; 4], rx)
        }
    } {
        Ok(transfer) => transfer,
        Err(_) => {
            unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
            t.final_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
            return publish(FAIL_SPI, t);
        }
    };

    if !prepared_state(snapshot())
        || !route_exact(rp1_rt::spi0_irq_route_snapshot(), before.primask)
    {
        let code = abort_record(&mut transfer, FAIL_SPI);
        unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
        t.final_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
        return publish(code, t);
    }
    t.wrapper_pre = read_wrapper();
    t.stage = 1;
    publish(FAIL_SETUP, t);
    if t.wrapper_pre != REARMED as u32 {
        let code = abort_record(&mut transfer, FAIL_WRAPPER_PRE);
        unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
        t.final_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
        return publish(code, t);
    }
    if !REARMED {
        unsafe {
            core::ptr::write_volatile((rp1_hal::addr::SPI0_BASE + 0x108) as *mut u32, 1);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        }
    }
    t.stage = 2;
    publish(FAIL_SETUP, t);
    t.wrapper_post = read_wrapper();
    if t.wrapper_post != 1
        || !prepared_state(snapshot())
        || !route_exact(rp1_rt::spi0_irq_route_snapshot(), before.primask)
    {
        let code = abort_record(&mut transfer, FAIL_WRAPPER_POST);
        unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
        t.final_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
        return publish(code, t);
    }
    t.stage = 3;
    t.flags |= FLAG_WRAPPER;

    if transfer.start().is_err()
        || !{
            #[cfg(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))]
            {
                wait_fifo_source(|| source_state(snapshot()) && read_wrapper() == 1)
            }
            #[cfg(not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
            {
                wait_until(|| source_state(snapshot()) && read_wrapper() == 1)
            }
        }
    {
        unsafe { rp1_rt::mask_spi0_irq19_one_entry() };
        let code = abort_record(&mut transfer, FAIL_SOURCE_TIMEOUT);
        unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
        t.wrapper_final = read_wrapper();
        t.final_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
        return publish(code, t);
    }
    t.flags |= FLAG_SOURCE;

    let source_route = rp1_rt::spi0_irq_route_snapshot();
    t.source_route = route_pack(source_route);
    if !source_route_exact(source_route, before.primask) {
        unsafe { rp1_rt::mask_spi0_irq19_one_entry() };
        let code = abort_record(&mut transfer, FAIL_ENABLE_PRESTATE);
        unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
        t.wrapper_final = read_wrapper();
        t.final_route = route_pack(rp1_rt::spi0_irq_route_snapshot());
        return publish(code, t);
    }

    unsafe { SLOT.publish(&mut transfer) };
    unsafe { rp1_rt::enable_spi0_irq19_one_entry_after_source_asserted(saved) };
    t.flags |= FLAG_ENABLED;

    let entered = wait_until(|| COUNT.load(Ordering::Relaxed) != 0);
    unsafe { rp1_rt::mask_spi0_irq19_one_entry() };
    t.flags |= FLAG_MASKED;
    unsafe { SLOT.withdraw() };

    let count_after_mask = COUNT.load(Ordering::Relaxed);
    if count_after_mask != 0 {
        t.flags |= FLAG_ENTERED;
    }
    if wait_stable_count(count_after_mask) {
        t.flags |= FLAG_NO_STORM;
    }

    let finish_ok = if entered && HANDLER_ERROR.load(Ordering::Relaxed) == 0 {
        transfer.finish().is_ok()
    } else {
        transfer.abort().is_ok()
    };
    if finish_ok {
        t.flags |= FLAG_FINISHED;
    }
    t.wrapper_final = read_wrapper();

    unsafe { rp1_rt::restore_spi0_irq19_one_entry(saved) };
    let final_route = rp1_rt::spi0_irq_route_snapshot();
    t.final_route = route_pack(final_route);
    if final_route.primask == saved.before.primask {
        t.flags |= FLAG_PRIMASK_RESTORED;
    }
    if final_route.vtor == saved.before.vtor {
        t.flags |= FLAG_VTOR_SAME;
    }
    if route_exact(final_route, before.primask) {
        t.flags |= FLAG_UNRELATED_UNCHANGED;
    }

    let decision = if !entered {
        FAIL_IRQ_TIMEOUT
    } else if HANDLER_ERROR.load(Ordering::Relaxed) != 0 {
        HANDLER_ERROR.load(Ordering::Relaxed)
    } else if !finish_ok {
        FAIL_FINISH
    } else if t.flags & PASS_FLAGS == PASS_FLAGS
        && t.wrapper_pre == REARMED as u32
        && t.wrapper_post == 1
        && t.wrapper_final == 1
        && COUNT.load(Ordering::Relaxed) == 1
        && FIRST_IPSR.load(Ordering::Relaxed) == VECTOR_INDEX
        && FIRST_ISR.load(Ordering::Relaxed) == RXFI
        && FIRST_RISR.load(Ordering::Relaxed) == RXFI | 1
        && FIRST_RXFLR.load(Ordering::Relaxed) == expected_len()
        && RECEIVED.load(Ordering::Relaxed) >> 16 == expected_len()
        && route_exact(final_route, before.primask)
    {
        1
    } else {
        FAIL_FINAL
    };
    publish(decision, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_is_irq19_vector35_s0i2_sixteen_words() {
        #[cfg(not(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof")))]
        assert_eq!(MAGIC, u32::from_le_bytes(*b"S0I2"));
        #[cfg(any(feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"))]
        assert_eq!(MAGIC, u32::from_le_bytes(*b"S0D2"));
        assert_eq!(IRQ_NUMBER, 19);
        assert_eq!(VECTOR_INDEX, 35);
        assert_eq!(WORDS, 16);
    }

    #[test]
    fn no_storm_requires_timer_progress_and_exactly_one_entry() {
        assert!(no_storm_ok(true, 1, 1));
        assert!(!no_storm_ok(false, 1, 1));
        assert!(!no_storm_ok(true, 0, 0));
        assert!(!no_storm_ok(true, 1, 2));
    }

    #[test]
    fn route_context_rejects_changed_or_invalid_primask() {
        assert!(primask_unchanged(0, 0));
        assert!(primask_unchanged(1, 1));
        assert!(!primask_unchanged(0, 1));
        assert!(!primask_unchanged(1, 0));
        assert!(!primask_unchanged(2, 2));
        assert!(!primask_unchanged(3, 0));
    }

    #[test]
    fn single_writer_error_retains_first_failure() {
        let error = AtomicU32::new(0);
        record_first_error(&error, FAIL_HANDLER_SOURCE);
        record_first_error(&error, FAIL_HANDLER_REPLAY);
        assert_eq!(error.load(Ordering::Relaxed), FAIL_HANDLER_SOURCE);
    }

    #[test]
    fn handler_precheck_rejects_replay_before_service() {
        assert_eq!(
            handler_precheck(1, VECTOR_INDEX, RXFI, true),
            Err(FAIL_HANDLER_REPLAY)
        );
        assert_eq!(handler_precheck(0, 0, RXFI, true), Err(FAIL_HANDLER_IPSR));
        assert_eq!(
            handler_precheck(0, VECTOR_INDEX, 0, true),
            Err(FAIL_HANDLER_SOURCE)
        );
        assert_eq!(
            handler_precheck(0, VECTOR_INDEX, RXFI, false),
            Err(FAIL_HANDLER_NO_TRANSFER)
        );
        assert_eq!(handler_precheck(0, VECTOR_INDEX, RXFI, true), Ok(()));
    }
}

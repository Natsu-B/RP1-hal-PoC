//! Cold-only SCMI IRQ commissioning. No clock writes, RTOS calls or polling.
//! Does not prove the candidate IRQ57 route or Linux endpoint reset survival.
use core::ptr;
use rp1_hal::{clock_adopt::ReadOnlyUartApb, clock_profile_generated::CLOCKS,
    scmi_clock::{ClockHardware, Server}, scmi_mailbox::{self, Counters, MailboxIo, Rp1Mailbox}};

const IRQ: usize = rp1_rt::SCMI_CANDIDATE_IRQ;
const BIT: u32 = 1 << (IRQ - 32);
const IPSR: u32 = rp1_rt::SCMI_CANDIDATE_VECTOR as u32;
const PRIORITY: u8 = 0xc0; // RP1 three priority bits; no FromISR API used here.

struct State { server: Server<'static>, io: Rp1Mailbox, count: Counters }
static mut STATE: Option<State> = None;

/// Single IRQ writer after prepare, read-only host observer. Sequence is even
/// when published; host must read sequence/payload/sequence and retry mismatch.
#[repr(C, align(4))]
pub struct Telemetry {
    pub magic: u32, pub version: u32, pub sequence: u32, pub ready: u32,
    pub irq_entries: u32, pub requests: u32, pub responses: u32, pub notifications: u32,
    pub malformed: u32, pub ipsr: u32, pub proc_events: u32, pub nvic_pending: u32,
    pub nvic_active: u32, pub status: u32, pub header: u32, pub error: u32,
    pub raw_entry_us: u32, pub raw_exit_us: u32, pub max_handler_us: u32,
    pub vtor: u32, pub vector: u32, pub shmem: u32, pub priority: u32,
    pub primask: u32, pub basepri: u32, pub linux_votes: u32, pub config_requests: u32,
}

#[used]
#[unsafe(no_mangle)]
pub static mut RP1_SCMI_TELEMETRY: Telemetry = unsafe { core::mem::zeroed() };

fn read(a: usize) -> u32 { unsafe { (a as *const u32).read_volatile() } }
fn write(a: usize, v: u32) { unsafe { (a as *mut u32).write_volatile(v) } }
fn barrier() { unsafe { core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags)); } }
fn mask() { write(0xe000_e184, BIT); barrier(); }

/// # Safety
/// Sole proc0 cold startup, before Linux and scheduler, PRIMASK=1. IRQ57 must
/// not belong to another service. No restart or live-channel reinitialization.
pub unsafe fn prepare() -> Result<(), &'static str> {
    let primask: u32;
    unsafe { core::arch::asm!("mrs {}, PRIMASK", out(reg) primask, options(nomem, nostack)); }
    let vtor = read(0xe000_ed08);
    if vtor != ptr::addr_of!(rp1_rt::VECTOR_TABLE) as u32 { return Err("VTOR owner"); }
    let vector = read(vtor as usize + IPSR as usize * 4);
    if primask != 1 || read(0xe000_e104) & BIT != 0 || read(0xe000_e304) & BIT != 0
        || read(0x4000_8008) != 0 || vector != RP1_SCMI_IRQHandler as *const () as u32 {
        return Err("active source/vector/interrupt owner");
    }
    if unsafe { (*ptr::addr_of!(STATE)).is_some() } { return Err("already initialized"); }
    let mut hw = ReadOnlyUartApb;
    for c in CLOCKS.iter().filter(|c| c.scmi_id.is_some()) {
        let physical = hw.read(c.rp1_id).map_err(|_| "clock tuple")?;
        if physical.rate_hz != u64::from(c.rate_hz) || !physical.enabled {
            return Err("physical clock mismatch");
        }
    }
    let server = Server::new(CLOCKS, false).map_err(|_| "profile")?;
    let shared = scmi_mailbox::shared_address();
    let mut io = unsafe { Rp1Mailbox::new(shared) }.ok_or("SRAM")?;
    scmi_mailbox::initialize(&mut io);
    unsafe { ptr::addr_of_mut!(STATE).write(Some(State { server, io, count: Counters::default() })); }
    unsafe {
        let t = &mut *ptr::addr_of_mut!(RP1_SCMI_TELEMETRY);
        ptr::addr_of_mut!(t.sequence).write_volatile(1); barrier();
        t.magic = u32::from_le_bytes(*b"SCI1"); t.version = 1;
        t.vtor = vtor; t.vector = vector; t.shmem = shared as u32;
        t.priority = u32::from(PRIORITY); t.primask = primask;
        (0xe000_e400usize.wrapping_add(IRQ) as *mut u8).write_volatile(PRIORITY);
        if (0xe000_e400usize.wrapping_add(IRQ) as *const u8).read_volatile() != PRIORITY {
            return Err("priority readback");
        }
        t.ready = 1;
        barrier(); ptr::addr_of_mut!(t.sequence).write_volatile(2); barrier();
    }
    write(0xe000_e284, BIT); // Only this NVIC pending bit, not shared PROC_EVENTS.
    barrier(); write(0xe000_e104, BIT); barrier();
    // Scheduler owns PRIMASK transition. Never rewrite VTOR, AIRCR or BASEPRI.
    Ok(())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn RP1_SCMI_IRQHandler() {
    let start = read(0x400a_c028);
    let ipsr: u32; let primask: u32; let basepri: u32;
    unsafe { core::arch::asm!("mrs {}, IPSR", "mrs {}, PRIMASK", "mrs {}, BASEPRI",
        out(reg) ipsr, out(reg) primask, out(reg) basepri, options(nomem, nostack)); }
    let t = unsafe { &mut *ptr::addr_of_mut!(RP1_SCMI_TELEMETRY) };
    let sequence = t.sequence.wrapping_add(1) | 1;
    unsafe { ptr::addr_of_mut!(t.sequence).write_volatile(sequence); } barrier();
    t.irq_entries = t.irq_entries.wrapping_add(1);
    t.ipsr = ipsr; t.primask = primask; t.basepri = basepri;
    t.proc_events = read(0x4000_8008);
    t.nvic_pending = read(0xe000_e204); t.nvic_active = read(0xe000_e304);
    t.raw_entry_us = start;
    if ipsr != IPSR || t.proc_events & !(1 << rp1_hal::clock_profile_generated::MAILBOX_CHANNEL) != 0 {
        t.error = 1; t.ready = 0; mask(); // No broad ACK or interrupt storm.
    } else if let Some(state) = unsafe { &mut *ptr::addr_of_mut!(STATE) } {
        t.header = state.io.read(6);
        let handled = scmi_mailbox::service_irq(&mut state.server, &mut ReadOnlyUartApb,
            &mut state.io, &mut state.count);
        t.status = state.io.read(1);
        t.requests = state.count.requests; t.responses = state.count.responses;
        t.notifications = state.count.notifications; t.malformed = state.count.malformed;
        t.linux_votes = state.server.linux_votes(); t.config_requests = state.server.config_requests;
        if !handled { t.error = 2; t.ready = 0; mask(); } // Spurious source is bounded.
    } else { t.error = 3; t.ready = 0; mask(); }
    t.raw_exit_us = read(0x400a_c028);
    t.max_handler_us = t.max_handler_us.max(t.raw_exit_us.wrapping_sub(start));
    barrier(); unsafe { ptr::addr_of_mut!(t.sequence).write_volatile(sequence.wrapping_add(1)); }
    barrier();
}

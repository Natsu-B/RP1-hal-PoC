//! Minimal event-driven SCMI server for RP1/Linux UART clock coexistence.
//!
//! This intentionally exposes only two fixed-rate clocks over SCMI:
//! - clock 0: RP1 shared UART functional clock, 50 MHz
//! - clock 1: RP1 UART APB clock (PLL_SYS_PRI_PH), 100 MHz
//!
//! Linux owns the SCMI vote. RP1 firmware owns a second vote. The physical
//! clock is enabled while either vote is set. No polling is used: Linux rings
//! RP1 mailbox doorbell 1, handled through the M3 SYSCFG interrupt.

use core::sync::atomic::{AtomicU32, Ordering};

pub const SCMI_CLOCK_UART: u32 = 0;
pub const SCMI_CLOCK_UART_APB: u32 = 1;
pub const SCMI_CLOCK_COUNT: u32 = 2;

pub const SCMI_SHMEM_BASE: usize = 0x2000_fb00;
pub const SCMI_SHMEM_SIZE: usize = 0x100;
pub const SCMI_DOORBELL_CHANNEL: u32 = 1;
pub const SCMI_DOORBELL_MASK: u32 = 1 << SCMI_DOORBELL_CHANNEL;

const SYSCFG_BASE: usize = 0x4000_8000;
const SYSCFG_PROC_EVENTS: usize = SYSCFG_BASE + 0x08;
const SYSCFG_HOST_EVENTS: usize = SYSCFG_BASE + 0x0c;
const HW_SET_BITS: usize = 0x2000;
const HW_CLR_BITS: usize = 0x3000;

const CLOCKMAN_BASE: usize = 0x4001_8000;
const CLK_UART_CTRL: usize = CLOCKMAN_BASE + 0x54;
const CLK_CTRL_ENABLE: u32 = 1 << 11;
const PLL_SYS_PRIM: usize = CLOCKMAN_BASE + 0x8000 + 0x10;
const PLL_PH_EN: u32 = 1 << 4;

const SCMI_PROTOCOL_BASE: u8 = 0x10;
const SCMI_PROTOCOL_CLOCK: u8 = 0x14;
const SCMI_PROTOCOL_VERSION: u8 = 0x00;
const SCMI_PROTOCOL_ATTRIBUTES: u8 = 0x01;
const SCMI_PROTOCOL_MESSAGE_ATTRIBUTES: u8 = 0x02;

const BASE_DISCOVER_VENDOR: u8 = 0x03;
const BASE_DISCOVER_SUB_VENDOR: u8 = 0x04;
const BASE_DISCOVER_IMPLEMENT_VERSION: u8 = 0x05;
const BASE_DISCOVER_LIST_PROTOCOLS: u8 = 0x06;
const BASE_DISCOVER_AGENT: u8 = 0x07;

const CLOCK_ATTRIBUTES: u8 = 0x03;
const CLOCK_DESCRIBE_RATES: u8 = 0x04;
const CLOCK_RATE_SET: u8 = 0x05;
const CLOCK_RATE_GET: u8 = 0x06;
const CLOCK_CONFIG_SET: u8 = 0x07;
const CLOCK_RATE_NOTIFY: u8 = 0x09;
const CLOCK_RATE_CHANGE_REQUESTED_NOTIFY: u8 = 0x0a;

const SCMI_SUCCESS: i32 = 0;
const SCMI_NOT_SUPPORTED: i32 = -1;
const SCMI_INVALID_PARAMETERS: i32 = -2;
const SCMI_PROTOCOL_ERROR: i32 = -10;

const SHMEM_CHANNEL_STATUS: usize = 0x04;
const SHMEM_FLAGS: usize = 0x10;
const SHMEM_LENGTH: usize = 0x14;
const SHMEM_MSG_HEADER: usize = 0x18;
const SHMEM_PAYLOAD: usize = 0x1c;
const SHMEM_CHANNEL_FREE: u32 = 1 << 0;
const SHMEM_CHANNEL_ERROR: u32 = 1 << 1;
const SHMEM_FLAG_INTR_ENABLED: u32 = 1 << 0;

const SCMI_MSG_TYPE_MASK: u32 = 0x3 << 8;
const SCMI_MSG_TYPE_COMMAND: u32 = 0;
const SCMI_MSG_PROTOCOL_SHIFT: u32 = 10;
const SCMI_MSG_PROTOCOL_MASK: u32 = 0xff << SCMI_MSG_PROTOCOL_SHIFT;

const UART_RATE: u64 = 50_000_000;
const UART_APB_RATE: u64 = 100_000_000;

const VOTE_UART: u32 = 1 << SCMI_CLOCK_UART;
const VOTE_UART_APB: u32 = 1 << SCMI_CLOCK_UART_APB;
const ALL_UART_VOTES: u32 = VOTE_UART | VOTE_UART_APB;

static LINUX_VOTES: AtomicU32 = AtomicU32::new(0);
static FIRMWARE_VOTES: AtomicU32 = AtomicU32::new(ALL_UART_VOTES);
static IRQ_COUNT: AtomicU32 = AtomicU32::new(0);
static PROC_EVENT_COUNT: AtomicU32 = AtomicU32::new(0);
static HOST_EVENT_COUNT: AtomicU32 = AtomicU32::new(0);
static CLOCK_CONFIG_SET_COUNT: AtomicU32 = AtomicU32::new(0);
static CLOCK_ENABLE_COUNT: AtomicU32 = AtomicU32::new(0);
static CLOCK_DISABLE_COUNT: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
pub struct Telemetry {
    pub irq_count: u32,
    pub proc_event_count: u32,
    pub host_event_count: u32,
    pub clock_config_set_count: u32,
    pub clock_enable_count: u32,
    pub clock_disable_count: u32,
    pub linux_votes: u32,
    pub firmware_votes: u32,
    pub clk_uart_ctrl: u32,
    pub clk_uart_div_int: u32,
    pub clk_uart_sel: u32,
    pub pll_sys_prim: u32,
}

/// Initialise the SCMI shared-memory channel and make the firmware vote visible
/// in the physical clock gates. Call this after the desired UART source/rates
/// have been programmed by firmware.
pub fn init_uart_clock_server() {
    LINUX_VOTES.store(0, Ordering::Release);
    FIRMWARE_VOTES.store(ALL_UART_VOTES, Ordering::Release);
    IRQ_COUNT.store(0, Ordering::Release);
    PROC_EVENT_COUNT.store(0, Ordering::Release);
    HOST_EVENT_COUNT.store(0, Ordering::Release);
    CLOCK_CONFIG_SET_COUNT.store(0, Ordering::Release);
    CLOCK_ENABLE_COUNT.store(0, Ordering::Release);
    CLOCK_DISABLE_COUNT.store(0, Ordering::Release);
    apply_clock_votes();

    write32(SCMI_SHMEM_BASE + SHMEM_FLAGS, SHMEM_FLAG_INTR_ENABLED);
    write32(SCMI_SHMEM_BASE + SHMEM_LENGTH, 0);
    write32(SCMI_SHMEM_BASE + SHMEM_MSG_HEADER, 0);
    write32(SCMI_SHMEM_BASE + SHMEM_CHANNEL_STATUS, SHMEM_CHANNEL_FREE);

    // Discard a stale channel-1 event before enabling the NVIC line.
    write32(SYSCFG_PROC_EVENTS + HW_CLR_BITS, SCMI_DOORBELL_MASK);
    barrier();
}

/// Set the RP1-firmware side clock votes. For the UART0=firmware demo both are
/// kept true for the lifetime of the firmware.
pub fn set_firmware_uart_votes(functional: bool, apb: bool) {
    let mut votes = 0;
    if functional {
        votes |= VOTE_UART;
    }
    if apb {
        votes |= VOTE_UART_APB;
    }
    FIRMWARE_VOTES.store(votes, Ordering::Release);
    apply_clock_votes();
}

pub fn linux_votes() -> u32 {
    LINUX_VOTES.load(Ordering::Acquire)
}

pub fn firmware_votes() -> u32 {
    FIRMWARE_VOTES.load(Ordering::Acquire)
}

pub fn telemetry() -> Telemetry {
    Telemetry {
        irq_count: IRQ_COUNT.load(Ordering::Acquire),
        proc_event_count: PROC_EVENT_COUNT.load(Ordering::Acquire),
        host_event_count: HOST_EVENT_COUNT.load(Ordering::Acquire),
        clock_config_set_count: CLOCK_CONFIG_SET_COUNT.load(Ordering::Acquire),
        clock_enable_count: CLOCK_ENABLE_COUNT.load(Ordering::Acquire),
        clock_disable_count: CLOCK_DISABLE_COUNT.load(Ordering::Acquire),
        linux_votes: linux_votes(),
        firmware_votes: firmware_votes(),
        clk_uart_ctrl: read32(CLK_UART_CTRL),
        clk_uart_div_int: read32(CLOCKMAN_BASE + 0x58),
        clk_uart_sel: read32(CLOCKMAN_BASE + 0x60),
        pll_sys_prim: read32(PLL_SYS_PRIM),
    }
}

/// Service mailbox doorbell 1. Intended to be called directly from the M3
/// SYSCFG/NVIC interrupt handler.
pub fn handle_doorbell_irq() {
    IRQ_COUNT.fetch_add(1, Ordering::Relaxed);
    let events = read32(SYSCFG_PROC_EVENTS);
    if events & SCMI_DOORBELL_MASK == 0 {
        return;
    }
    PROC_EVENT_COUNT.fetch_add(1, Ordering::Relaxed);

    write32(SYSCFG_PROC_EVENTS + HW_CLR_BITS, SCMI_DOORBELL_MASK);
    barrier();

    if read32(SCMI_SHMEM_BASE + SHMEM_CHANNEL_STATUS) & SHMEM_CHANNEL_FREE != 0 {
        return;
    }

    service_request();

    // SCMI requires the channel to be released before the completion IRQ.
    write32(SCMI_SHMEM_BASE + SHMEM_CHANNEL_STATUS, SHMEM_CHANNEL_FREE);
    barrier();

    if read32(SCMI_SHMEM_BASE + SHMEM_FLAGS) & SHMEM_FLAG_INTR_ENABLED != 0 {
        write32(SYSCFG_HOST_EVENTS + HW_SET_BITS, SCMI_DOORBELL_MASK);
        HOST_EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
        barrier();
    }
}

fn service_request() {
    let length = read32(SCMI_SHMEM_BASE + SHMEM_LENGTH) as usize;
    let header = read32(SCMI_SHMEM_BASE + SHMEM_MSG_HEADER);
    let msg_type = header & SCMI_MSG_TYPE_MASK;
    let protocol = ((header & SCMI_MSG_PROTOCOL_MASK) >> SCMI_MSG_PROTOCOL_SHIFT) as u8;
    let message = (header & 0xff) as u8;

    // TX length includes the four-byte SCMI message header, but not the SMT
    // transport fields preceding it.
    if length < 4
        || length > SCMI_SHMEM_SIZE - SHMEM_MSG_HEADER
        || msg_type != SCMI_MSG_TYPE_COMMAND
    {
        finish_error(SCMI_PROTOCOL_ERROR);
        return;
    }

    let request_len = length - 4;
    match protocol {
        SCMI_PROTOCOL_BASE => service_base(message, request_len),
        SCMI_PROTOCOL_CLOCK => service_clock(message, request_len),
        _ => finish_error(SCMI_NOT_SUPPORTED),
    }
}

fn service_base(message: u8, request_len: usize) {
    match message {
        SCMI_PROTOCOL_VERSION if request_len == 0 => {
            // Base protocol v2.0.
            finish_u32(SCMI_SUCCESS, 0x0002_0000);
        }
        SCMI_PROTOCOL_ATTRIBUTES if request_len == 0 => {
            // num_protocols=1 (Clock), num_agents=2 (platform + Linux OSPM).
            finish_u32(SCMI_SUCCESS, 0x0000_0201);
        }
        SCMI_PROTOCOL_MESSAGE_ATTRIBUTES if request_len == 4 => {
            let id = request_u32(0) as u8;
            let supported = matches!(
                id,
                SCMI_PROTOCOL_VERSION
                    | SCMI_PROTOCOL_ATTRIBUTES
                    | SCMI_PROTOCOL_MESSAGE_ATTRIBUTES
                    | BASE_DISCOVER_VENDOR
                    | BASE_DISCOVER_SUB_VENDOR
                    | BASE_DISCOVER_IMPLEMENT_VERSION
                    | BASE_DISCOVER_LIST_PROTOCOLS
                    | BASE_DISCOVER_AGENT
            );
            if supported {
                finish_u32(SCMI_SUCCESS, 0);
            } else {
                finish_error(SCMI_NOT_SUPPORTED);
            }
        }
        BASE_DISCOVER_VENDOR if request_len == 0 => {
            finish_name(SCMI_SUCCESS, b"RP1-PoC");
        }
        BASE_DISCOVER_SUB_VENDOR if request_len == 0 => {
            finish_name(SCMI_SUCCESS, b"Natsu-B");
        }
        BASE_DISCOVER_IMPLEMENT_VERSION if request_len == 0 => {
            finish_u32(SCMI_SUCCESS, 1);
        }
        BASE_DISCOVER_LIST_PROTOCOLS if request_len == 4 => {
            let skip = request_u32(0);
            if skip == 0 {
                response_begin(SCMI_SUCCESS, 8);
                response_u32(0, 1);
                // One u8 protocol id followed by zero padding.
                response_u32(4, SCMI_PROTOCOL_CLOCK as u32);
            } else {
                response_begin(SCMI_SUCCESS, 4);
                response_u32(0, 0);
            }
        }
        BASE_DISCOVER_AGENT if request_len == 4 => {
            let id = request_u32(0);
            match id {
                0 => finish_agent(SCMI_SUCCESS, 0, b"RP1-FW"),
                1 => finish_agent(SCMI_SUCCESS, 1, b"OSPM"),
                _ => finish_error(SCMI_INVALID_PARAMETERS),
            }
        }
        _ => finish_error(SCMI_NOT_SUPPORTED),
    }
}

fn service_clock(message: u8, request_len: usize) {
    match message {
        SCMI_PROTOCOL_VERSION if request_len == 0 => {
            // Clock protocol v2.0. This keeps CONFIG_SET on the compact v1/v2
            // layout while still allowing enable-latency in CLOCK_ATTRIBUTES.
            finish_u32(SCMI_SUCCESS, 0x0002_0000);
        }
        SCMI_PROTOCOL_ATTRIBUTES if request_len == 0 => {
            // u16 num_clocks, u8 max_async_req, u8 reserved.
            finish_u32(SCMI_SUCCESS, SCMI_CLOCK_COUNT);
        }
        SCMI_PROTOCOL_MESSAGE_ATTRIBUTES if request_len == 4 => {
            let id = request_u32(0) as u8;
            let supported = matches!(
                id,
                SCMI_PROTOCOL_VERSION
                    | SCMI_PROTOCOL_ATTRIBUTES
                    | SCMI_PROTOCOL_MESSAGE_ATTRIBUTES
                    | CLOCK_ATTRIBUTES
                    | CLOCK_DESCRIBE_RATES
                    | CLOCK_RATE_GET
                    | CLOCK_CONFIG_SET
            );
            if supported {
                finish_u32(SCMI_SUCCESS, 0);
            } else if id == CLOCK_RATE_SET
                || id == CLOCK_RATE_NOTIFY
                || id == CLOCK_RATE_CHANGE_REQUESTED_NOTIFY
            {
                finish_error(SCMI_NOT_SUPPORTED);
            } else {
                finish_error(SCMI_NOT_SUPPORTED);
            }
        }
        CLOCK_ATTRIBUTES if request_len == 4 => {
            let id = request_u32(0);
            let Some((name, vote)) = clock_name_vote(id) else {
                finish_error(SCMI_INVALID_PARAMETERS);
                return;
            };
            let enabled = combined_votes() & vote != 0;
            response_begin(SCMI_SUCCESS, 24);
            response_u32(0, u32::from(enabled));
            response_name(4, name);
            // Unknown/irrelevant for this fixed clock service. Linux treats 0
            // as unspecified and falls back to a non-atomic prepare/unprepare.
            response_u32(20, 0);
        }
        CLOCK_DESCRIBE_RATES if request_len == 8 => {
            let id = request_u32(0);
            let index = request_u32(4);
            let Some(rate) = clock_rate(id) else {
                finish_error(SCMI_INVALID_PARAMETERS);
                return;
            };
            if index == 0 {
                response_begin(SCMI_SUCCESS, 12);
                // one discrete rate, zero remaining, bit12 clear => discrete.
                response_u32(0, 1);
                response_u64(4, rate);
            } else {
                response_begin(SCMI_SUCCESS, 4);
                response_u32(0, 0);
            }
        }
        CLOCK_RATE_GET if request_len == 4 => {
            let id = request_u32(0);
            let Some(rate) = clock_rate(id) else {
                finish_error(SCMI_INVALID_PARAMETERS);
                return;
            };
            response_begin(SCMI_SUCCESS, 8);
            response_u64(0, rate);
        }
        CLOCK_CONFIG_SET if request_len >= 8 => {
            let id = request_u32(0);
            let state = request_u32(4) & 0x3;
            let Some((_, vote)) = clock_name_vote(id) else {
                finish_error(SCMI_INVALID_PARAMETERS);
                return;
            };
            match state {
                0 => {
                    CLOCK_CONFIG_SET_COUNT.fetch_add(1, Ordering::Relaxed);
                    CLOCK_DISABLE_COUNT.fetch_add(1, Ordering::Relaxed);
                    LINUX_VOTES.fetch_and(!vote, Ordering::AcqRel);
                    apply_clock_votes();
                    finish_error(SCMI_SUCCESS);
                }
                1 => {
                    CLOCK_CONFIG_SET_COUNT.fetch_add(1, Ordering::Relaxed);
                    CLOCK_ENABLE_COUNT.fetch_add(1, Ordering::Relaxed);
                    LINUX_VOTES.fetch_or(vote, Ordering::AcqRel);
                    apply_clock_votes();
                    finish_error(SCMI_SUCCESS);
                }
                _ => finish_error(SCMI_INVALID_PARAMETERS),
            }
        }
        CLOCK_RATE_SET => finish_error(SCMI_NOT_SUPPORTED),
        _ => finish_error(SCMI_NOT_SUPPORTED),
    }
}

fn combined_votes() -> u32 {
    LINUX_VOTES.load(Ordering::Acquire) | FIRMWARE_VOTES.load(Ordering::Acquire)
}

fn apply_clock_votes() {
    let votes = combined_votes();
    update_bit(CLK_UART_CTRL, CLK_CTRL_ENABLE, votes & VOTE_UART != 0);
    update_bit(PLL_SYS_PRIM, PLL_PH_EN, votes & VOTE_UART_APB != 0);
    barrier();
}

fn clock_name_vote(id: u32) -> Option<(&'static [u8], u32)> {
    match id {
        SCMI_CLOCK_UART => Some((b"rp1-uart", VOTE_UART)),
        SCMI_CLOCK_UART_APB => Some((b"rp1-uart-apb", VOTE_UART_APB)),
        _ => None,
    }
}

fn clock_rate(id: u32) -> Option<u64> {
    match id {
        SCMI_CLOCK_UART => Some(UART_RATE),
        SCMI_CLOCK_UART_APB => Some(UART_APB_RATE),
        _ => None,
    }
}

fn finish_error(status: i32) {
    response_begin(status, 0);
}

fn finish_u32(status: i32, value: u32) {
    response_begin(status, 4);
    response_u32(0, value);
}

fn finish_name(status: i32, name: &[u8]) {
    response_begin(status, 16);
    response_name(0, name);
}

fn finish_agent(status: i32, id: u32, name: &[u8]) {
    response_begin(status, 20);
    response_u32(0, id);
    response_name(4, name);
}

fn response_begin(status: i32, data_len: usize) {
    let max_data = SCMI_SHMEM_SIZE - (SHMEM_PAYLOAD + 4);
    if data_len > max_data {
        write32(SCMI_SHMEM_BASE + SHMEM_PAYLOAD, SCMI_PROTOCOL_ERROR as u32);
        write32(SCMI_SHMEM_BASE + SHMEM_LENGTH, 8);
        write32(SCMI_SHMEM_BASE + SHMEM_CHANNEL_STATUS, SHMEM_CHANNEL_ERROR);
        return;
    }
    write32(SCMI_SHMEM_BASE + SHMEM_PAYLOAD, status as u32);
    // Response length includes message header + status + response data.
    write32(SCMI_SHMEM_BASE + SHMEM_LENGTH, (8 + data_len) as u32);
}

fn request_u32(offset: usize) -> u32 {
    read32(SCMI_SHMEM_BASE + SHMEM_PAYLOAD + offset)
}

fn response_u32(offset: usize, value: u32) {
    write32(SCMI_SHMEM_BASE + SHMEM_PAYLOAD + 4 + offset, value);
}

fn response_u64(offset: usize, value: u64) {
    response_u32(offset, value as u32);
    response_u32(offset + 4, (value >> 32) as u32);
}

fn response_name(offset: usize, name: &[u8]) {
    let base = SCMI_SHMEM_BASE + SHMEM_PAYLOAD + 4 + offset;
    for i in 0..16 {
        let value = if i < name.len() { name[i] } else { 0 };
        write8(base + i, value);
    }
}

fn update_bit(addr: usize, bit: u32, set: bool) {
    let mut value = read32(addr);
    if set {
        value |= bit;
    } else {
        value &= !bit;
    }
    write32(addr, value);
}

#[inline(always)]
fn read32(addr: usize) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

#[inline(always)]
fn write32(addr: usize, value: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) }
}

#[inline(always)]
fn write8(addr: usize, value: u8) {
    unsafe { core::ptr::write_volatile(addr as *mut u8, value) }
}

#[inline(always)]
fn barrier() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

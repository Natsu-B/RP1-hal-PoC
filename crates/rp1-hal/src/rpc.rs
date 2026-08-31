use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use rp1_abi::rpc::{self, ClockId, Opcode, Request, Status};

use crate::mmio::Reg;
use crate::timer::RawTimer;

const PLL_SYS_PRIM: usize = 0x4002_0010;
const CLK_UART_CTRL: usize = 0x4001_8054;
const CLK_UART_DIV_INT: usize = 0x4001_8058;
const CLK_UART_SEL: usize = 0x4001_8060;

static LAST_SEQUENCE: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_SEQUENCE_VALID: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct ClockState {
    effective: u32,
    physical: u32,
    result0: u32,
    result1: u32,
}

pub fn init() {
    LAST_SEQUENCE.store(u32::MAX, Ordering::Release);
    LAST_SEQUENCE_VALID.store(false, Ordering::Release);
    reg(rpc::RESPONSE_ADDR as usize).write(0);
    dsb_sy();
    let mut response = [0u32; rpc::RESPONSE_WORDS];
    rpc::init_response_header(&mut response);
    clear_slot(rpc::REQUEST_ADDR as usize, rpc::REQUEST_WORDS);
    clear_slot(rpc::RESPONSE_ADDR as usize, rpc::RESPONSE_WORDS);
    clear_slot(rpc::TELEMETRY_ADDR as usize, rpc::TELEMETRY_WORDS);
    write_words_except_magic(rpc::RESPONSE_ADDR as usize, &response);
    dmb_sy();
    reg(rpc::RESPONSE_ADDR as usize).write(rpc::RESPONSE_MAGIC);
    init_telemetry();
}

pub fn poll(timer: &RawTimer) {
    poll_with(timer.now(), read_clock_state);
}

fn poll_with(now: u64, read_clock: fn(ClockId) -> Option<ClockState>) -> bool {
    telemetry_inc(4);
    let request_owner = reg(rpc::REQUEST_ADDR as usize + rpc::OWNER_WORD * 4).read();
    if request_owner != rpc::OWNER_READY {
        return false;
    }
    let response_owner = reg(rpc::RESPONSE_ADDR as usize + rpc::OWNER_WORD * 4).read();
    if !slots_ready(request_owner, response_owner) {
        telemetry_inc(16);
        return false;
    }

    dmb_sy();
    let request_words = read_slot(rpc::REQUEST_ADDR as usize);
    telemetry_inc(5);
    let last = last_sequence();
    let (response, next_sequence, validation_error) =
        handle_request(&request_words, last, now, read_clock);
    if let Some(status) = validation_error {
        telemetry_inc(error_counter_word(status));
    } else {
        telemetry_inc(6);
    }
    if let Some(sequence) = next_sequence {
        remember_sequence(sequence);
    }
    record_last(&request_words, &response);

    write_words_except_owner(rpc::RESPONSE_ADDR as usize, &response);
    dmb_sy();
    reg(rpc::RESPONSE_ADDR as usize + rpc::OWNER_WORD * 4).write(rpc::OWNER_READY);
    dsb_sy();
    reg(rpc::REQUEST_ADDR as usize + rpc::OWNER_WORD * 4).write(rpc::OWNER_EMPTY);
    telemetry_inc(7);
    true
}

fn slots_ready(request_owner: u32, response_owner: u32) -> bool {
    request_owner == rpc::OWNER_READY && response_owner == rpc::OWNER_EMPTY
}

fn handle_request(
    request_words: &[u32; rpc::REQUEST_WORDS],
    last_sequence: Option<u32>,
    now: u64,
    read_clock: fn(ClockId) -> Option<ClockState>,
) -> ([u32; rpc::RESPONSE_WORDS], Option<u32>, Option<Status>) {
    let (response, validation_error) = match rpc::validate_request(request_words, last_sequence) {
        Ok(request) => (success_response(request, now, read_clock), None),
        Err(status) => (error_response(request_words, status, now), Some(status)),
    };
    let next_sequence = if response[rpc::word::STATUS] == Status::Replay as u32 {
        last_sequence
    } else {
        Some(request_words[rpc::word::SEQUENCE])
    };
    (response, next_sequence, validation_error)
}

fn success_response(
    request: Request,
    now: u64,
    read_clock: fn(ClockId) -> Option<ClockState>,
) -> [u32; rpc::RESPONSE_WORDS] {
    let mut response = [0u32; rpc::RESPONSE_WORDS];
    rpc::init_response(&mut response, &request, Status::Ok, now);
    match request.opcode {
        Opcode::Ping => {
            response[rpc::word::EFFECTIVE_STATE] = 1;
            response[rpc::word::PHYSICAL_STATE] = 1;
            response[rpc::word::RESULT0] = rpc::VERSION;
            response[rpc::word::RESULT1] = rpc::PING_BITMAP;
        }
        Opcode::GetCapabilities => {
            response[rpc::word::RESULT0] = rpc::FEATURE_BITMAP;
            response[rpc::word::RESULT1] = 0x200;
        }
        Opcode::GetClockState => {
            let Ok(clock_id) = ClockId::try_from(request.object) else {
                response[rpc::word::STATUS] = Status::BadObject as u32;
                rpc::stamp_response(&mut response);
                return response;
            };
            let Some(state) = read_clock(clock_id) else {
                response[rpc::word::STATUS] = Status::BadObject as u32;
                rpc::stamp_response(&mut response);
                return response;
            };
            response[rpc::word::EFFECTIVE_STATE] = state.effective;
            response[rpc::word::PHYSICAL_STATE] = state.physical;
            response[rpc::word::RESULT0] = state.result0;
            response[rpc::word::RESULT1] = state.result1;
        }
    }
    rpc::stamp_response(&mut response);
    response
}

fn error_response(
    request_words: &[u32; rpc::REQUEST_WORDS],
    status: Status,
    now: u64,
) -> [u32; rpc::RESPONSE_WORDS] {
    let mut response = [0u32; rpc::RESPONSE_WORDS];
    rpc::init_response_raw(
        &mut response,
        request_words[rpc::word::SEQUENCE],
        request_words[rpc::word::OPCODE],
        status,
        now,
    );
    rpc::stamp_response(&mut response);
    response
}

fn read_clock_state(clock_id: ClockId) -> Option<ClockState> {
    match clock_id {
        ClockId::PllSysPriPh => {
            let prim = reg(PLL_SYS_PRIM).read();
            Some(ClockState {
                effective: (prim >> 4) & 1,
                physical: prim,
                result0: prim & 0x3,
                result1: 2,
            })
        }
        ClockId::Uart => {
            let ctrl = reg(CLK_UART_CTRL).read();
            Some(ClockState {
                effective: (ctrl >> 11) & 1,
                physical: ctrl,
                result0: reg(CLK_UART_DIV_INT).read(),
                result1: reg(CLK_UART_SEL).read(),
            })
        }
    }
}

fn last_sequence() -> Option<u32> {
    if LAST_SEQUENCE_VALID.load(Ordering::Acquire) {
        Some(LAST_SEQUENCE.load(Ordering::Acquire))
    } else {
        None
    }
}

fn remember_sequence(sequence: u32) {
    LAST_SEQUENCE.store(sequence, Ordering::Release);
    LAST_SEQUENCE_VALID.store(true, Ordering::Release);
}

fn record_last(request: &[u32; rpc::REQUEST_WORDS], response: &[u32; rpc::RESPONSE_WORDS]) {
    let base = rpc::TELEMETRY_ADDR as usize;
    reg(base + 18 * 4).write(request[rpc::word::SEQUENCE]);
    reg(base + 19 * 4).write(request[rpc::word::OPCODE]);
    reg(base + 20 * 4).write(request[rpc::word::OBJECT]);
    reg(base + 21 * 4).write(response[rpc::word::STATUS]);
    reg(base + 22 * 4).write(request[rpc::CHECKSUM_WORD]);
    reg(base + 23 * 4).write(response[rpc::CHECKSUM_WORD]);
}

fn init_telemetry() {
    clear_slot(rpc::TELEMETRY_ADDR as usize, rpc::TELEMETRY_WORDS);
    let base = rpc::TELEMETRY_ADDR as usize;
    reg(base + 4).write(rpc::VERSION);
    reg(base + 8).write(rpc::TELEMETRY_WORDS as u32);
    reg(base + 12).write(1);
    reg(base + 18 * 4).write(u32::MAX);
    reg(base + 19 * 4).write(u32::MAX);
    reg(base + 20 * 4).write(u32::MAX);
    reg(base + 21 * 4).write(u32::MAX);
    dmb_sy();
    reg(base).write(rpc::TELEMETRY_MAGIC);
}

fn error_counter_word(status: Status) -> usize {
    match status {
        Status::BadMagic => 8,
        Status::BadVersion => 9,
        Status::BadLength => 10,
        Status::BadChecksum => 11,
        Status::BadReserved => 12,
        Status::BadFlags => 13,
        Status::BadOpcode => 14,
        Status::BadObject => 15,
        Status::Busy => 16,
        Status::Replay => 17,
        Status::Ok => 7,
    }
}

fn read_slot(addr: usize) -> [u32; rpc::REQUEST_WORDS] {
    let mut words = [0u32; rpc::REQUEST_WORDS];
    for (index, word) in words.iter_mut().enumerate() {
        *word = reg(addr + index * 4).read();
    }
    words
}

fn write_words_except_magic(addr: usize, words: &[u32; rpc::RESPONSE_WORDS]) {
    for (index, word) in words.iter().enumerate().skip(1) {
        reg(addr + index * 4).write(*word);
    }
}

fn write_words_except_owner(addr: usize, words: &[u32; rpc::RESPONSE_WORDS]) {
    for (index, word) in words.iter().enumerate() {
        if index != rpc::OWNER_WORD {
            reg(addr + index * 4).write(*word);
        }
    }
}

fn clear_slot(addr: usize, words: usize) {
    for index in 0..words {
        reg(addr + index * 4).write(0);
    }
}

fn telemetry_inc(index: usize) {
    let addr = rpc::TELEMETRY_ADDR as usize + index * 4;
    let old = reg(addr).read();
    reg(addr).write(old.wrapping_add(1));
}

#[inline(always)]
fn reg(addr: usize) -> Reg<u32> {
    unsafe { Reg::new(addr) }
}

#[inline(always)]
fn dmb_sy() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dmb sy", options(nostack, preserves_flags));
    }

    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[inline(always)]
fn dsb_sy() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(opcode: Opcode, object: u32, sequence: u32) -> [u32; rpc::REQUEST_WORDS] {
        let mut words = [0u32; rpc::REQUEST_WORDS];
        words[rpc::word::MAGIC] = rpc::REQUEST_MAGIC;
        words[rpc::word::VERSION] = rpc::VERSION;
        words[rpc::word::HEADER_WORDS] = rpc::HEADER_WORDS;
        words[rpc::word::TOTAL_WORDS] = rpc::TOTAL_WORDS;
        words[rpc::word::SEQUENCE] = sequence;
        words[rpc::word::OPCODE] = opcode as u32;
        words[rpc::word::OBJECT] = object;
        rpc::stamp_checksum(&mut words);
        words[rpc::OWNER_WORD] = rpc::OWNER_READY;
        words
    }

    fn fake_clock(id: ClockId) -> Option<ClockState> {
        match id {
            ClockId::Uart => Some(ClockState {
                effective: 1,
                physical: 0x800,
                result0: 1,
                result1: 2,
            }),
            ClockId::PllSysPriPh => None,
        }
    }

    #[test]
    fn slot_gate_requires_owned_request_and_empty_response() {
        assert!(slots_ready(rpc::OWNER_READY, rpc::OWNER_EMPTY));
        assert!(!slots_ready(rpc::OWNER_EMPTY, rpc::OWNER_EMPTY));
        assert!(!slots_ready(rpc::OWNER_READY, rpc::OWNER_READY));
        assert!(!slots_ready(rpc::OWNER_READY, 2));
    }

    #[test]
    fn shared_handler_returns_clock_state_and_next_sequence() {
        let request = req(Opcode::GetClockState, ClockId::Uart as u32, 1);
        let (response, next, validation_error) = handle_request(&request, None, 10, fake_clock);
        assert_eq!(response[rpc::word::STATUS], Status::Ok as u32);
        assert_eq!(response[rpc::word::PHYSICAL_STATE], 0x800);
        assert_eq!(next, Some(1));
        assert_eq!(validation_error, None);
    }

    #[test]
    fn shared_handler_allows_u32_max_once_then_keeps_replay_state() {
        let request = req(Opcode::Ping, 0, u32::MAX);
        let (first, last, validation_error) = handle_request(&request, None, 1, fake_clock);
        assert_eq!(first[rpc::word::STATUS], Status::Ok as u32);
        assert_eq!(last, Some(u32::MAX));
        assert_eq!(validation_error, None);

        let (second, last, validation_error) = handle_request(&request, last, 2, fake_clock);
        assert_eq!(second[rpc::word::STATUS], Status::Replay as u32);
        assert_eq!(last, Some(u32::MAX));
        assert_eq!(validation_error, Some(Status::Replay));

        let (third, last, validation_error) = handle_request(&request, last, 3, fake_clock);
        assert_eq!(third[rpc::word::STATUS], Status::Replay as u32);
        assert_eq!(last, Some(u32::MAX));
        assert_eq!(validation_error, Some(Status::Replay));
    }
}

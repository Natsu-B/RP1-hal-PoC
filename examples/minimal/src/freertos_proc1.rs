//! P1R1 worker connected to a proc0 FreeRTOS task; proc1 runs no kernel.
//! One launch, one outstanding request, three epochs; no reset/reclamation of live state.

const MAGIC: u32 = 0x3152_3150;
const VERSION: u32 = 0x0040_0002;
const DONE: u32 = 0x3145_4e44;
const GUAR: u32 = 0x5241_5547;
const RTOU: u32 = 0x554f_5452;
const TOUT: u32 = 0x5455_4f54;
const FAIL: u32 = 0x4c49_4146;
const CANARY: u32 = 0xa17e_cafe;
const INITIAL: u32 = 0x1357_9bdf;
const WORK: u32 = 1;
const PAUSE: u32 = 2;
const RESUME: u32 = 3;
const OK: u32 = 1;
const PAUSED: u32 = 2;
const RESUMED: u32 = 3;
const STALE: u32 = 4;
const DUP: u32 = 5;
const INVALID: u32 = 6;
const WRONG_STATE: u32 = 7;
const BAD_GUARD: u32 = 8;
const EXHAUSTED: u32 = 9;

#[derive(Clone, Copy)]
struct Request { epoch: u32, seq: u32, op: u32, arg: u32, token: u32 }

#[derive(Clone, Copy, Debug, PartialEq)]
struct State { epoch: u32, seq: u32, token: u32, paused: bool }

impl State {
    const fn new() -> Self { Self { epoch: 1, seq: 0, token: 0, paused: false } }

    #[inline(always)]
    fn consume(&mut self, r: Request, guards: bool) -> u32 {
        if !guards { return BAD_GUARD; }
        if r.token <= self.token || r.token == u32::MAX { return EXHAUSTED; }
        self.token = r.token; // A rejected transaction consumes transport, not application state.
        if r.op == RESUME {
            if r.epoch <= self.epoch { return STALE; }
            if self.epoch == u32::MAX - 1 { return EXHAUSTED; }
            if !self.paused { return WRONG_STATE; }
            if r.epoch != self.epoch + 1 || r.seq != 1 || r.arg != 0 { return INVALID; }
            self.epoch = r.epoch;
            self.seq = 1;
            self.paused = false;
            return RESUMED;
        }
        if r.epoch != self.epoch { return STALE; }
        if r.seq == 0 || r.seq <= self.seq { return DUP; }
        if r.seq == u32::MAX || self.seq == u32::MAX - 1 { return EXHAUSTED; }
        if r.seq != self.seq + 1 { return INVALID; }
        if (r.op != WORK && r.op != PAUSE) || (r.op == WORK && r.arg > 255)
            || (r.op == PAUSE && r.arg != 0) { return INVALID; }
        if self.paused { return WRONG_STATE; }
        self.seq = r.seq;
        self.paused = r.op == PAUSE;
        if self.paused { PAUSED } else { OK }
    }
}

#[derive(Default)]
struct Producer { sent: u32, acked: u32, frozen: bool }
impl Producer {
    fn begin(&mut self, token: u32) -> bool {
        if self.frozen || self.sent != self.acked || token == u32::MAX
            || self.sent.checked_add(1) != Some(token) { return false; }
        self.sent = token;
        true
    }
    fn acquire(&mut self, token: u32) -> bool {
        if self.frozen || token != self.sent || self.sent == self.acked { return false; }
        self.acked = token;
        true
    }
    fn timeout(&mut self) { self.frozen = true; }
}

const fn req(epoch: u32, seq: u32, op: u32, arg: u32, token: u32) -> Request {
    Request { epoch, seq, op, arg, token }
}
const PLAN: [(Request, u32); 12] = [
    (req(1, 1, WORK, 7, 1), OK), (req(1, 2, PAUSE, 0, 2), PAUSED),
    (req(0, 3, WORK, 99, 3), STALE), (req(2, 1, RESUME, 0, 4), RESUMED),
    (req(1, 3, WORK, 99, 5), STALE), (req(2, 2, WORK, 11, 6), OK),
    (req(2, 2, WORK, 99, 7), DUP), (req(2, 3, 99, 99, 8), INVALID),
    (req(2, 3, PAUSE, 0, 9), PAUSED), (req(3, 1, RESUME, 0, 10), RESUMED),
    (req(3, 2, WORK, 13, 11), OK), (req(3, 3, PAUSE, 0, 12), PAUSED),
];

fn start_allowed(ctrl: u32, done: u32) -> bool { ctrl >> 31 == 1 && done >> 31 == 0 }

#[cfg(target_arch = "arm")]
mod hardware {
    use super::*;
    use core::ptr::{addr_of, addr_of_mut, read_volatile, write_volatile};
    const CTRL: *mut u32 = 0x4001_4000 as *mut u32;
    const RESET_DONE: *const u32 = 0x4001_4018 as *const u32;
    const SCRATCH_MAGIC: *mut u32 = 0x4015_400c as *mut u32;
    const SCRATCH_ENTRY: *mut u32 = 0x4015_4014 as *mut u32;
    const SCRATCH_SP: *mut u32 = 0x4015_401c as *mut u32;
    const READY: u32 = 0x5944_4552;
    const MAX_POLLS: u32 = 200; // Independent finite bound on delayed observations.
    const TIMEOUT_US: u64 = 100_000;

    #[unsafe(link_section = ".proc1.init")]
    static PROC1_APP_INIT: u32 = INITIAL;
    #[unsafe(link_section = ".proc1.data")]
    static mut PROC1_APP_DATA: u32 = 0;
    #[unsafe(link_section = ".proc1.bss")]
    static mut PROC1_APP_BSS: u32 = 0;
    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".proc1.lifecycle")]
    static mut PROC1_RUNTIME_READY: [u32; 14] = [0; 14];
    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".proc1.request")]
    static mut PROC1_RUNTIME_REQUEST: [u32; 5] = [0; 5];
    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".proc1.response")]
    static mut PROC1_RUNTIME_RESPONSE: [u32; 12] = [0; 12];
    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".proc1.fault")]
    static mut PROC1_RUNTIME_FAULT: [u32; 3] = [0; 3];

    unsafe extern "C" {
        static __proc1_stack_low: u8;
        static __proc1_stack_top: u8;
        static __proc1_guard_low: u8;
        static __proc1_guard_high: u8;
        static __proc1_vectors_start: u8;
        fn Proc1RuntimeEntry(core_id: u32) -> !;
    }

    core::arch::global_asm!(
        ".syntax unified", ".thumb",
        ".section .proc1.text.entry,\"ax\",%progbits", ".balign 4",
        ".global Proc1RuntimeEntry", ".type Proc1RuntimeEntry,%function", ".thumb_func",
        "Proc1RuntimeEntry:",
        "cpsid i", "mov r12, r0", "mrs r1, MSP", "mrs r2, PSP", "mrs r3, CONTROL",
        "cmp r0, #1", "bne 1f", "ldr r0, =0xe00ff01c", "ldr r0, [r0]",
        "cmp r0, #1", "bne 1f", "mrs r0, IPSR", "cmp r0, #0", "bne 1f",
        "cmp r3, #0", "bne 1f",
        "ldr r0, =PROC1_RUNTIME_READY", "str r12, [r0, #0]",
        "movs r12, #1", "str r12, [r0, #4]",
        "ldr r12, =0xe000ed00", "ldr r12, [r12]", "str r12, [r0, #8]",
        "str r1, [r0, #12]", "str r2, [r0, #16]", "str r3, [r0, #20]",
        "movs r3, #0", "str r3, [r0, #24]",
        // Install stackless fault handlers first. NMI/HardFault cannot be masked;
        // the pre-VTOR interval still belongs to ROM and is not fault-injection proved.
        "ldr r1, =__proc1_vectors_start", "ldr r2, =0xe000ed08", "str r1, [r2]",
        "dsb sy", "isb sy", "ldr r1, =__proc1_stack_top", "msr MSP, r1",
        "msr CONTROL, r3", "isb sy", "mrs r1, MSP", "str r1, [r0, #28]",
        "b.w Proc1RuntimeBody", "1:", "movs r0, #1", "b.w Proc1RuntimeFaultPark",
        ".size Proc1RuntimeEntry, .-Proc1RuntimeEntry", ".ltorg",
        ".global Proc1RuntimeFault", ".type Proc1RuntimeFault,%function", ".thumb_func",
        "Proc1RuntimeFault:", "movs r0, #2", "b.w Proc1RuntimeFaultPark",
        ".size Proc1RuntimeFault, .-Proc1RuntimeFault",
        ".global Proc1RuntimeFaultPark", ".type Proc1RuntimeFaultPark,%function", ".thumb_func",
        "Proc1RuntimeFaultPark:", "cpsid i", "mrs r1, IPSR",
        "ldr r2, =PROC1_RUNTIME_FAULT", "str r0, [r2]", "str r1, [r2, #4]",
        "dmb sy", "movs r0, #1", "str r0, [r2, #8]", "dsb sy", "sev",
        "2:", "wfe", "b 2b", ".size Proc1RuntimeFaultPark, .-Proc1RuntimeFaultPark", ".ltorg",
        ".section .proc1.vectors,\"a\",%progbits", ".balign 512",
        ".word __proc1_stack_top", ".rept 79", ".word Proc1RuntimeFault", ".endr",
    );

    // Only raw pointers cross cores; never create overlapping references to shared
    // storage. Aligned u32 volatile accesses + DMB publication are the whole SPSC
    // contract. Each slot has one writer after the verified-held initialization.
    #[inline(always)]
    fn get(p: *const u32, i: usize) -> u32 { unsafe { read_volatile(p.add(i)) } }
    #[inline(always)]
    fn put(p: *mut u32, i: usize, v: u32) { unsafe { write_volatile(p.add(i), v) } }
    #[inline(always)]
    fn acquire() { unsafe { core::arch::asm!("dmb sy", options(nostack, preserves_flags)); } }
    #[inline(always)]
    fn notify() { unsafe { core::arch::asm!("dsb sy", "sev", options(nostack, preserves_flags)); } }
    #[inline(always)]
    fn guards() -> bool {
        let low = addr_of!(__proc1_guard_low).cast::<u32>();
        let high = addr_of!(__proc1_guard_high).cast::<u32>();
        get(low, 0) == CANARY && get(low, 1) == !CANARY
            && get(high, 0) == CANARY && get(high, 1) == !CANARY
    }

    #[unsafe(no_mangle)]
    #[unsafe(link_section = ".proc1.text.body")]
    unsafe extern "C" fn Proc1RuntimeBody() -> ! {
        let ready = addr_of_mut!(PROC1_RUNTIME_READY).cast::<u32>();
        let request = addr_of!(PROC1_RUNTIME_REQUEST).cast::<u32>();
        let response = addr_of_mut!(PROC1_RUNTIME_RESPONSE).cast::<u32>();
        let data = addr_of_mut!(PROC1_APP_DATA);
        let bss = addr_of_mut!(PROC1_APP_BSS);
        let msp: u32;
        unsafe { core::arch::asm!("mrs {}, MSP", out(reg) msp, options(nostack, preserves_flags)); }
        put(ready, 8, msp);
        put(ready, 9, get(0xe000_ed08 as *const u32, 0));
        put(ready, 10, 1);
        put(ready, 11, get(data, 0));
        put(ready, 12, get(bss, 0));
        acquire(); put(ready, 13, READY); notify();
        let mut state = State::new();
        let mut heartbeat = 1u32;
        let mut seen = 0;
        loop {
            let token = get(request, 4);
            if token == seen {
                // No deadline in this control wait. SEV is a hint; always recheck.
                unsafe { core::arch::asm!("dsb sy", "wfe", options(nostack, preserves_flags)); }
                continue;
            }
            acquire();
            let r = Request { epoch: get(request, 0), seq: get(request, 1),
                op: get(request, 2), arg: get(request, 3), token };
            seen = token;
            let status = state.consume(r, guards());
            if status == OK {
                put(data, 0, get(data, 0).wrapping_add(r.arg));
                put(bss, 0, get(bss, 0).wrapping_add(1));
            } else if status == RESUMED {
                // Only app-owned words reset; never stack, guards, lifecycle or slots.
                put(data, 0, get(addr_of!(PROC1_APP_INIT), 0));
                put(bss, 0, 0);
            }
            heartbeat = heartbeat.saturating_add(1);
            put(response, 0, r.epoch); put(response, 1, r.seq);
            put(response, 2, status); put(response, 3, get(data, 0) ^ get(bss, 0));
            put(response, 4, guards() as u32); put(response, 5, heartbeat);
            put(response, 6, state.epoch); put(response, 7, state.seq);
            put(response, 8, if state.paused { 2 } else { 1 });
            put(response, 9, get(data, 0)); put(response, 10, get(bss, 0));
            acquire(); put(response, 11, token); notify();
            if status == BAD_GUARD || status == EXHAUSTED {
                unsafe { core::arch::asm!("movs r0, #3", "b.w Proc1RuntimeFaultPark", options(noreturn, nostack)); }
            }
        }
    }

    fn timer() -> Option<u64> {
        // The existing high/low/high timer contract, now also bounded if incoherent.
        for _ in 0..3 {
            let high = get(0x400a_c024 as *const u32, 0);
            let low = get(0x400a_c028 as *const u32, 0);
            if high == get(0x400a_c024 as *const u32, 0) {
                return Some((u64::from(high) << 32) | u64::from(low));
            }
        }
        None
    }

    fn wait_for(p: *const u32, index: usize, mask: u32, value: u32, heartbeat: &mut u32) -> bool {
        let Some(start) = timer() else { return false; };
        for _ in 0..MAX_POLLS {
            *heartbeat += 1;
            let Some(now) = timer() else { return false; };
            if now.wrapping_sub(start) >= TIMEOUT_US { return false; }
            if get(p, index) & mask == value { acquire(); return true; }
            // Proc0 blocks; proc1's SEV is not a FreeRTOS task notification.
            unsafe { rp1_freertos::delay(1).unwrap(); }
        }
        false
    }

    fn run(out: &mut [u32; 64], heartbeat: &mut u32) -> u32 {
        let ready = addr_of_mut!(PROC1_RUNTIME_READY).cast::<u32>();
        let request = addr_of_mut!(PROC1_RUNTIME_REQUEST).cast::<u32>();
        let response = addr_of_mut!(PROC1_RUNTIME_RESPONSE).cast::<u32>();
        let fault = addr_of_mut!(PROC1_RUNTIME_FAULT).cast::<u32>();
        // BEFORE-WRITE guard: no proc1-owned storage, scratch or reset write before this.
        if out[2] != 0 || !start_allowed(get(CTRL, 0), get(RESET_DONE, 0)) {
            out[55] = 1; return GUAR;
        }
        for (p, words) in [(ready, 14), (request, 5), (response, 12), (fault, 3)] {
            for i in 0..words { put(p, i, 0); }
        }
        put(addr_of_mut!(PROC1_APP_DATA), 0, get(addr_of!(PROC1_APP_INIT), 0));
        put(addr_of_mut!(PROC1_APP_BSS), 0, 0);
        for p in [addr_of!(__proc1_guard_low), addr_of!(__proc1_guard_high)] {
            put(p as *mut u32, 0, CANARY); put(p as *mut u32, 1, !CANARY);
        }
        // Only while proc1 is verified held. Never refill on cooperative RESUME.
        for i in 0..512 { put(addr_of!(__proc1_stack_low) as *mut u32, i, 0xa5a5_a5a5); }
        out[13] = addr_of!(__proc1_stack_low) as u32;
        out[14] = addr_of!(__proc1_stack_top) as u32;
        out[15] = get(addr_of!(__proc1_guard_low).cast(), 0);
        out[17] = get(addr_of!(__proc1_guard_high).cast(), 0);
        out[59] = (Proc1RuntimeEntry as *const () as u32 | 1) ^ 0x4ff8_3f2d;
        put(SCRATCH_MAGIC, 0, 0xb007_c0de);
        put(SCRATCH_SP, 0, out[14]);
        unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
        put(SCRATCH_ENTRY, 0, out[59]);
        unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
        let ctrl = get(CTRL, 0);
        if !start_allowed(ctrl, get(RESET_DONE, 0)) { out[55] = 1; return GUAR; }
        // Sole direct bit31 deassert; no CLEAR alias, reassert, retry, or hardware restart.
        put(CTRL, 0, ctrl & !(1 << 31));
        unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
        if !wait_for(RESET_DONE, 0, 1 << 31, 1 << 31, heartbeat) { out[55] = 2; return RTOU; }
        notify();
        out[57] = get(SCRATCH_MAGIC, 0); out[58] = get(SCRATCH_SP, 0);
        if !wait_for(ready, 13, u32::MAX, READY, heartbeat) { out[55] = 3; return TOUT; }
        // ROM clears entry before BLX; DONE alone does not prove dispatch ran.
        out[60] = get(SCRATCH_ENTRY, 0);
        // READY acquired; these immutable snapshots never change on RESUME.
        for i in 0..10 { out[3 + i] = get(ready, i); }
        out[21] = get(ready, 10); out[22] = out[21];
        out[35] = get(ready, 11); out[36] = get(ready, 12);
        out[37] = out[35]; out[38] = out[36];
        out[25] = 1; out[26] = 0; out[27] = 1;
        if out[3] != 1 || out[4] != 1 || out[8] != 0 || out[9] != 0
            || out[10] != out[14] || !(out[13]..out[14]).contains(&out[11])
            || out[12] != addr_of!(__proc1_vectors_start) as u32
            || out[35] != INITIAL || out[36] != 0 || out[21] != 1 {
            out[55] = 5; return FAIL;
        }
        let mut producer = Producer::default();
        let mut model = State::new();
        let mut data = INITIAL;
        let mut bss = 0u32;
        for (index, (r, expected_status)) in PLAN.into_iter().enumerate() {
            // Let the unmodified R1 tasks run between requests. No busy polling,
            // proc1 IRQ, or second FreeRTOS kernel is introduced.
            unsafe { rp1_freertos::delay(100).unwrap(); }
            *heartbeat += 1;
            if !producer.begin(r.token) { out[55] = 5; return FAIL; }
            put(request, 0, r.epoch); put(request, 1, r.seq);
            put(request, 2, r.op); put(request, 3, r.arg);
            acquire(); put(request, 4, r.token); notify();
            if !wait_for(response, 11, u32::MAX, r.token, heartbeat) {
                producer.timeout(); out[55] = 4; return TOUT;
            }
            // Matching response acquired: producer must not send again until validated.
            let mut ack = [0u32; 12];
            for (i, word) in ack.iter_mut().enumerate() { *word = get(response, i); }
            if !producer.acquire(ack[11]) { out[55] = 5; return FAIL; }
            let status = model.consume(r, true);
            if status == OK { data = data.wrapping_add(r.arg); bss += 1; }
            if status == RESUMED { data = INITIAL; bss = 0; }
            let valid = ack[0] == r.epoch && ack[1] == r.seq && ack[2] == expected_status
                && status == expected_status && ack[3] == data ^ bss && ack[4] == 1
                && ack[5] == out[22] + 1 && ack[6] == model.epoch && ack[7] == model.seq
                && ack[8] == if model.paused { 2 } else { 1 }
                && ack[9] == data && ack[10] == bss;
            // Rejections must preserve actual acquired app/accepted epoch/seq/state.
            let unchanged = !matches!(ack[2], STALE | DUP | INVALID)
                || (ack[6] == out[25] && ack[7] == out[26] && ack[8] == out[27]
                    && ack[9] == out[37] && ack[10] == out[38]);
            out[22] = ack[5]; out[25] = ack[6]; out[26] = ack[7]; out[27] = ack[8];
            out[28] += 1; out[29] = ack[11]; out[30] = ack[4];
            out[37] = ack[9]; out[38] = ack[10];
            out[43 + index] = ack[0] << 24 | ack[1] << 16 | ack[2] << 8 | ack[11];
            if !valid || !unchanged { out[55] = 5; return FAIL; }
            out[42] |= 1 << index;
            match ack[2] {
                OK => out[31 + r.epoch as usize] = ack[3],
                STALE => out[39] += 1, DUP => out[40] += 1, INVALID => out[41] += 1,
                _ => (),
            }
        }
        // Final PAUSED acquired; no further request or worker application writes.
        out[16] = get(addr_of!(__proc1_guard_low).cast(), 0);
        out[18] = get(addr_of!(__proc1_guard_high).cast(), 0);
        if !guards() || out[27] != 2 || out[42] != 0xfff { out[55] = 5; return FAIL; }
        DONE
    }

    pub unsafe extern "C" fn worker(_: *mut core::ffi::c_void) {
        // No boot-stack pointer crosses vTaskStartScheduler. This task alone owns
        // the producer and words96..159; proc1 owns only its dedicated slots.
        unsafe { rp1_freertos::delay(2000).unwrap(); }
        let (ipsr, control, psp, msp): (u32, u32, u32, u32);
        unsafe {
            core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
                out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
        }
        for (i, v) in [ipsr, control, psp, msp].into_iter().enumerate() { super::super::put(160+i, v); }
        assert_eq!(ipsr, 0); assert_eq!(control & 3, 2); assert_eq!(psp & 7, 0);
        super::super::put(164, super::super::get(8));
        super::super::put(166, super::super::get(9));
        super::super::put(170, super::super::raw_low());
        let mut out = [0u32; 64];
        out[0] = MAGIC; out[1] = VERSION; out[31] = INITIAL;
        out[2] = get(0xe00f_f01c as *const u32, 0);
        out[61] = get(0xe000_ed08 as *const u32, 0);
        unsafe { core::arch::asm!("mrs {}, MSP", out(reg) out[62], options(nostack, preserves_flags)); }
        let mut heartbeat = 1;
        out[19] = heartbeat;
        let mut terminal = run(&mut out, &mut heartbeat);
        // Check kernel/task progress after cooperative PAUSE, without touching
        // shared .data/.bss, VTOR, priorities, PRIMASK or proc1 reset again.
        unsafe { rp1_freertos::delay(2000).unwrap(); }
        out[20] = heartbeat; out[56] = 2000; // RTOS delay ticks, ABI v2.
        super::super::put(165, super::super::get(8));
        super::super::put(167, super::super::get(9));
        super::super::put(171, super::super::raw_low());
        super::super::put(169, unsafe { super::super::task(7).stack_high_water().unwrap() });
        assert!(super::super::get(165).wrapping_sub(super::super::get(164)) >= 2000);
        assert!(super::super::get(167).wrapping_sub(super::super::get(166)) > 0);
        assert_eq!(unsafe { core::ptr::addr_of!(super::super::DATA_SENTINEL).read_volatile() }, INITIAL);
        assert_eq!(unsafe { core::ptr::addr_of!(super::super::BSS_SENTINEL).read_volatile() }, 0x2468_ace0);
        let fault = addr_of!(PROC1_RUNTIME_FAULT).cast::<u32>();
        // Do not inspect even the fault slot after a rejected launch (may belong to live code).
        if out[55] != 1 && get(fault, 2) == 1 {
            acquire(); out[23] = get(fault, 0); out[24] = get(fault, 1);
            if terminal == DONE { terminal = FAIL; out[55] = 5; }
        }
        if terminal == DONE {
            let mut unused = 0;
            while unused < 512 && get(addr_of!(__proc1_stack_low).cast(), unused) == 0xa5a5_a5a5 { unused += 1; }
            super::super::put(172, (512 - unused) as u32 * 4);
            assert!(unused >= 32);
        }
        let mailbox = unsafe { super::super::TELEMETRY.add(96) };
        put(mailbox, 63, 0);
        for (i, word) in out.into_iter().enumerate().take(63) { put(mailbox, i, word); }
        acquire(); put(mailbox, 63, terminal);
        unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
        // Immutable receipt: no late response/fault may republish success.
        // The owner keeps checking the paused worker's guard/fault state; never
        // restart/reclaim after a timeout. No dependency on a live Linux host.
        loop {
            unsafe { rp1_freertos::delay(1000).unwrap(); }
            super::super::increment(168);
            super::super::put(169, unsafe { super::super::task(7).stack_high_water().unwrap() });
            if terminal == DONE {
                assert!(guards());
                assert_eq!(get(fault, 2), 0);
            }
        }
    }
}

#[cfg(target_arch = "arm")]
pub use hardware::worker;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_protocol() {
        assert_eq!(MAGIC.to_le_bytes(), *b"P1R1");
        assert_eq!(VERSION, (64 << 16) | 2);
        assert_eq!(core::mem::size_of::<[u32; 64]>(), 256);
        assert!(start_allowed(1 << 31, 0));
        assert!(!start_allowed(0, 0)); assert!(!start_allowed(1 << 31, 1 << 31));
        let mut state = State::new();
        let mut producer = Producer::default();
        let mut data = INITIAL;
        let mut bss = 0;
        let mut results = Vec::new();
        for (r, expected) in PLAN {
            assert!(producer.begin(r.token));
            assert!(!producer.begin(r.token + 1)); // busy, no overwrite
            let before = state;
            let status = state.consume(r, true);
            assert_eq!(status, expected);
            match status {
                OK => { data += r.arg; bss += 1; results.push(data ^ bss); }
                RESUMED => { data = INITIAL; bss = 0; }
                STALE | DUP | INVALID => {
                    assert_eq!((state.epoch, state.seq, state.paused),
                               (before.epoch, before.seq, before.paused));
                }
                _ => (),
            }
            assert!(producer.acquire(r.token));
            assert!(!producer.acquire(r.token));
        }
        assert_eq!(results, [0x1357_9be7, 0x1357_9beb, 0x1357_9bed]);
        assert_eq!(state, State { epoch: 3, seq: 3, token: 12, paused: true });
        let before = state;
        assert_eq!(state.consume(req(4, 1, RESUME, 0, 13), false), BAD_GUARD);
        assert_eq!(state, before);
        assert_eq!(state.consume(req(3, 4, WORK, 0, 13), true), WRONG_STATE);
        let mut active = State::new();
        assert_eq!(active.consume(req(2, 1, RESUME, 0, 1), true), WRONG_STATE);
        assert_eq!(active.consume(req(1, 1, WORK, 256, 2), true), INVALID);
        assert_eq!(active.consume(req(1, 1, WORK, 1, 2), true), EXHAUSTED);
        assert_eq!(active.consume(req(1, 1, WORK, 1, u32::MAX), true), EXHAUSTED);
        let mut final_epoch = State { epoch: u32::MAX - 1, seq: 2, token: 1, paused: true };
        assert_eq!(final_epoch.consume(req(u32::MAX, 1, RESUME, 0, 2), true), EXHAUSTED);
        let mut final_seq = State { epoch: 1, seq: u32::MAX - 1, token: 1, paused: false };
        assert_eq!(final_seq.consume(req(1, u32::MAX, WORK, 1, 2), true), EXHAUSTED);
        assert!(producer.begin(13)); producer.timeout();
        assert!(!producer.acquire(13)); assert!(!producer.begin(14));
        let mut exhausted = Producer { sent: u32::MAX - 1, acked: u32::MAX - 1, frozen: false };
        assert!(!exhausted.begin(u32::MAX)); assert!(!exhausted.begin(0));
    }
}

//! R1 integrated workload. These are test tasks, not a replacement scheduler.
//! Seven permanent proc0 tasks; no task/ISR retains a boot-stack pointer.
use core::{ffi::c_void, ptr};
use rp1_freertos::{self as os, BinarySemaphore, Mutex, Task, U32Queue};
use rp1_hal::gpio::{ConfiguredPin, Output};

const TELEMETRY: *mut u32 = 0x2000_f800 as *mut u32;
static mut DATA_SENTINEL: u32 = 0x1357_9bdf;
static mut BSS_SENTINEL: u32 = 0;
static mut TASKS: [Option<Task>; 7] = [None; 7];
static mut QUEUE: Option<U32Queue> = None;
static mut CHECK_QUEUE: Option<U32Queue> = None;
static mut SEM: Option<BinarySemaphore> = None;
static mut MUTEX: Option<Mutex> = None;
static mut MARKER: Option<ConfiguredPin<22, Output>> = None;

fn put(index: usize, value: u32) { unsafe { TELEMETRY.add(index).write_volatile(value); } }
fn get(index: usize) -> u32 { unsafe { TELEMETRY.add(index).read_volatile() } }
fn increment(index: usize) { put(index, get(index).wrapping_add(1)); }
fn raw_low() -> u32 { unsafe { (0x400a_c028 as *const u32).read_volatile() } }
unsafe fn task(slot: usize) -> Task {
    unsafe { ptr::addr_of!(TASKS).cast::<Option<Task>>().add(slot).read().unwrap() }
}

/// SysTick processor-clock calibration, interrupts disabled, five 10ms samples.
/// No CLOCKS/PLL writes, no DWT or TENMS assumption. Raw timer is the established
/// microsecond reference. Record spread; reject stuck/implausible/unstable counts.
fn calibrate_cpu_hz() -> u32 {
    let mut min = u32::MAX;
    let mut max = 0;
    let mut total = 0u64;
    unsafe {
        let ctrl = 0xe000_e010 as *mut u32;
        let reload = 0xe000_e014 as *mut u32;
        let value = 0xe000_e018 as *mut u32;
        assert_eq!(ctrl.read_volatile() & 3, 0);
        reload.write_volatile(0x00ff_ffff);
        value.write_volatile(0);
        ctrl.write_volatile(5); // ENABLE + processor clock; TICKINT remains clear.
        core::arch::asm!("dsb sy", "isb", options(nostack));
        for _ in 0..5 {
            let t0 = raw_low();
            let c0 = value.read_volatile();
            let mut t1 = t0;
            // Iteration guard is independent of the timer reference.
            for _ in 0..10_000_000 {
                t1 = raw_low();
                if t1.wrapping_sub(t0) >= 10_000 { break; }
            }
            let c1 = value.read_volatile();
            let dt = t1.wrapping_sub(t0);
            assert!((10_000..=11_000).contains(&dt));
            let cycles = c0.wrapping_sub(c1) & 0x00ff_ffff;
            let hz = (u64::from(cycles) * 1_000_000 / u64::from(dt)) as u32;
            assert!((1_000_000..=500_000_000).contains(&hz));
            min = min.min(hz); max = max.max(hz); total += u64::from(hz);
        }
        ctrl.write_volatile(0);
        value.write_volatile(0);
        // Clear any stale SysTick pending; do not touch peripheral IRQ ownership.
        (0xe000_ed04 as *mut u32).write_volatile(1 << 25);
    }
    put(6, min); put(7, max);
    assert!(max - min <= max / 100); // <=1% calibration spread, not accuracy claim.
    ((total / 5 + 500) / 1000 * 1000) as u32
}

pub fn run(marker: ConfiguredPin<22, Output>) -> ! {
    for n in 0..256 { put(n, 0); } // Clear stale exception record as well.
    put(0, u32::from_le_bytes(*b"RT01")); put(1, 1); put(2, 1);
    put(12, u32::MAX);
    unsafe {
        put(19, ptr::addr_of!(DATA_SENTINEL).read_volatile());
        put(20, ptr::addr_of!(BSS_SENTINEL).read_volatile());
        assert_eq!(get(19), 0x1357_9bdf); assert_eq!(get(20), 0);
        ptr::addr_of_mut!(BSS_SENTINEL).write_volatile(0x2468_ace0);
        ptr::addr_of_mut!(MARKER).write(Some(marker));
        // PRIGROUP=0: all implemented priority bits used for preemption. No reset.
        let aircr = 0xe000_ed0c as *mut u32;
        aircr.write_volatile(0x05fa_0000 | (aircr.read_volatile() & 0x8000));
        put(21, aircr.read_volatile());
        assert_eq!(get(21) & 0x700, 0);
        for (index, address) in [(23, 0x4001_8014), (24, 0x4001_8018), (25, 0x4001_8020)] {
            put(index, (address as *const u32).read_volatile());
        }
    }
    let hz = calibrate_cpu_hz(); put(5, hz); put(2, 2);
    unsafe {
        ptr::addr_of_mut!(QUEUE).write(Some(U32Queue::create(0, 4).unwrap()));
        ptr::addr_of_mut!(CHECK_QUEUE).write(Some(U32Queue::create(1, 1).unwrap()));
        assert!(U32Queue::create(2, 0).is_err());
        assert!(U32Queue::create(2, 17).is_err());
        ptr::addr_of_mut!(SEM).write(Some(BinarySemaphore::create(0).unwrap()));
        ptr::addr_of_mut!(MUTEX).write(Some(Mutex::create(0).unwrap()));
        let entries: [(os::TaskEntry, &core::ffi::CStr, u32, u32); 7] = [
            (monitor, c"monitor", 4, 512), (spin, c"spin-a", 1, 128),
            (spin, c"spin-b", 1, 128), (consumer, c"consumer", 3, 256),
            (producer, c"producer", 2, 256), (mutex_low, c"mutex-low", 1, 256),
            (mutex_high, c"mutex-high", 3, 256),
        ];
        for (slot, (entry, name, priority, words)) in entries.into_iter().enumerate() {
            let arg = match slot { 1 => 0x2000_f900, 2 => 0x2000_f940, _ => 0 };
            let handle = Task::create(slot as u32, name, entry, arg as *mut c_void, priority, words).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(slot).write(Some(handle));
        }
        put(2, 3);
        os::start(hz).unwrap();
    }
    panic!("scheduler returned")
}

#[unsafe(no_mangle)]
extern "C" fn rp1_freertos_tick_hook() {
    let now = raw_low();
    let last = get(11);
    if get(8) != 0 {
        let dt = now.wrapping_sub(last);
        put(12, get(12).min(dt)); put(13, get(13).max(dt));
    } else { put(26, now); }
    put(11, now); increment(8);
}

#[unsafe(no_mangle)]
extern "C" fn rp1_freertos_switch_hook(id: u32) {
    put(10, id); increment(9);
}

#[unsafe(no_mangle)]
extern "C" fn rp1_freertos_fault_hook(reason: u32, detail: u32) -> ! {
    unsafe { core::arch::asm!("cpsid i", options(nostack)); }
    put(3, reason); put(4, detail);
    let (ipsr, control, psp, msp): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
            out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
    }
    for (i, v) in [ipsr, control, psp, msp].into_iter().enumerate() { put(56+i, v); }
    unsafe { core::arch::asm!("dsb sy", options(nostack)); }
    put(2, 0xffff_ffff);
    loop { unsafe { core::arch::asm!("wfi", options(nomem, nostack)); } }
}

unsafe extern "C" fn monitor(_: *mut c_void) {
    let (ipsr, control, psp, msp): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
            out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
    }
    for (i, v) in [ipsr, control, psp, msp].into_iter().enumerate() { put(28+i, v); }
    assert_eq!(ipsr, 0); assert_eq!(control & 3, 2); assert_eq!(psp & 7, 0);
    assert!((0x2000_e000..=0x2000_f000).contains(&msp));
    let mut marker = unsafe { ptr::addr_of!(MARKER).read().unwrap() };
    let q = unsafe { ptr::addr_of!(CHECK_QUEUE).read().unwrap() };
    unsafe {
        assert_eq!(q.receive(2).unwrap(), None); // real block/timeout
        assert!(q.send(0x1234_abcd, 0).unwrap());
        assert!(!q.send(1, 0).unwrap()); // full queue
        assert_eq!(q.receive(0).unwrap(), Some(0x1234_abcd));
        assert_eq!(q.receive(0).unwrap(), None);
    }
    put(39, 1); put(2, 4);
    let mut previous = [0; 4];
    loop {
        unsafe { os::delay(1000).unwrap(); }
        let current = [get(64), get(80), get(49), get(50)];
        for i in 0..4 { assert_ne!(current[i], previous[i]); }
        previous = current;
        assert_eq!(get(70) | get(86), 0); // assembly context error latches
        unsafe {
            for slot in 0..7 { put(32+slot, task(slot).stack_high_water().unwrap()); }
            put(22, (0xe000_e014 as *const u32).read_volatile());
        }
        let mut untouched = 0;
        while untouched < 1024 && unsafe { (0x2000_e000 as *const u32).add(untouched).read_volatile() } == 0xa5a5_a5a5 {
            untouched += 1;
        }
        put(18, (1024 - untouched) as u32 * 4);
        assert!(untouched >= 32); // ISR/boot MSP guard remains intact
        increment(14); put(27, raw_low()); put(2, 5);
        marker.toggle();
    }
}

unsafe extern "C" fn producer(_: *mut c_void) {
    let q = unsafe { ptr::addr_of!(QUEUE).read().unwrap() };
    let sem = unsafe { ptr::addr_of!(SEM).read().unwrap() };
    let mut sequence = 0u32;
    loop { unsafe {
        sequence = sequence.wrapping_add(1);
        assert!(q.send(sequence, 10).unwrap());
        assert!(sem.give().unwrap());
        task(3).notification_give().unwrap();
        put(48, sequence);
        os::delay(5).unwrap();
    } }
}

unsafe extern "C" fn consumer(_: *mut c_void) {
    let q = unsafe { ptr::addr_of!(QUEUE).read().unwrap() };
    let sem = unsafe { ptr::addr_of!(SEM).read().unwrap() };
    let mut expected = 0u32;
    loop { unsafe {
        assert_eq!(os::notification_take(true, 100).unwrap(), 1);
        expected = expected.wrapping_add(1);
        assert_eq!(q.receive(10).unwrap(), Some(expected));
        assert!(sem.take(10).unwrap());
        put(49, expected); increment(15);
    } }
}

unsafe extern "C" fn mutex_low(_: *mut c_void) {
    let m = unsafe { ptr::addr_of!(MUTEX).read().unwrap() };
    loop { unsafe {
        assert!(m.take(100).unwrap());
        task(6).notification_give().unwrap(); // high task runs, blocks on this mutex
        assert_eq!(task(5).priority().unwrap(), 3); // observed inherited priority
        increment(17);
        assert!(m.give().unwrap());
        assert_eq!(task(5).priority().unwrap(), 1);
        os::delay(20).unwrap();
    } }
}

unsafe extern "C" fn mutex_high(_: *mut c_void) {
    let m = unsafe { ptr::addr_of!(MUTEX).read().unwrap() };
    loop { unsafe {
        assert_eq!(os::notification_take(true, 100).unwrap(), 1);
        assert!(m.take(100).unwrap());
        increment(50);
        assert!(m.give().unwrap());
    } }
}

// No function calls, yield, blocking, or compiler-elidable register test here.
// Both equal-priority tasks continuously verify R4-R11 across actual preemption.
#[unsafe(naked)]
unsafe extern "C" fn spin(_: *mut c_void) {
    core::arch::naked_asm!(
        "mrs r1, IPSR", "str r1, [r0, #4]",
        "mrs r1, CONTROL", "str r1, [r0, #8]",
        "mrs r1, PSP", "str r1, [r0, #12]",
        "mrs r1, MSP", "str r1, [r0, #16]",
        "mov r4, #0x44", "mov r5, #0x55", "mov r6, #0x66", "mov r7, #0x77",
        "mov r8, #0x88", "mov r9, #0x99", "mov r10, #0xaa", "mov r11, #0xbb",
        "2:",
        "cmp r4, #0x44", "bne 3f", "cmp r5, #0x55", "bne 3f",
        "cmp r6, #0x66", "bne 3f", "cmp r7, #0x77", "bne 3f",
        "cmp r8, #0x88", "bne 3f", "cmp r9, #0x99", "bne 3f",
        "cmp r10, #0xaa", "bne 3f", "cmp r11, #0xbb", "bne 3f",
        "ldr r1, [r0]", "adds r1, #1", "str r1, [r0]", "b 2b",
        "3:", "movs r1, #1", "str r1, [r0, #24]", "b 3b",
    );
}

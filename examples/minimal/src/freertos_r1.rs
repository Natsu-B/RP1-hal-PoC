//! R1 integrated workload. These are test tasks, not a replacement scheduler.
//! Seven permanent proc0 tasks; no task/ISR retains a boot-stack pointer.
use core::{ffi::c_void, ptr};
use rp1_freertos::{self as os, BinarySemaphore, Task, U32Queue};
#[cfg(not(feature = "freertos-r2-mixed"))]
use rp1_freertos::Mutex;
use rp1_hal::gpio::{ConfiguredPin, Output};

#[cfg(all(feature = "freertos-r3-watchdog-arm-receipt", any(feature = "freertos-r3-proc1-worker", feature = "freertos-r1-critical-timing", feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "freertos-r1-assert", feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart", feature = "freertos-r2-mixed", feature = "uart0-rx-irq")))]
compile_error!("WDT2 owns task8/words96..183; normal R1 only, no proc1/fault/IO modes");
#[cfg(feature = "freertos-r3-watchdog-arm-receipt")]
#[path = "freertos_watchdog.rs"]
pub mod watchdog;

#[cfg(feature = "freertos-r3-watchdog-quiescence")]
#[path = "watchdog_quiescence.rs"]
mod watchdog_quiescence;
#[cfg(feature = "freertos-r3-watchdog-postack")]
#[path = "watchdog_postack.rs"]
mod watchdog_postack;

#[cfg(any(feature = "freertos-r3-reset-entry-selftest", feature = "freertos-r3-watchdog-expiry-entry"))]
#[path = "watchdog_reset_identity.rs"]
mod reset_identity;
#[cfg(any(feature = "freertos-r3-reset-entry-selftest", feature = "freertos-r3-watchdog-expiry-entry"))]
#[path = "reset_entry_target.rs"]
mod reset_entry;
#[cfg(feature = "freertos-r3-watchdog-expiry-entry")]
#[path = "watchdog_boot_entry.rs"]
mod boot_entry;
#[cfg(all(feature = "freertos-r3-watchdog-expiry-entry", feature = "freertos-r3-watchdog-late-disable"))]
compile_error!("Expiry entry and late-disable control are separate experiments");
#[cfg(all(feature = "freertos-r3-reset-entry-selftest", feature = "freertos-r3-watchdog-postack"))]
compile_error!("Reset-entry selftest performs no long watchdog arm; select it separately");

#[cfg(all(feature = "freertos-r3-proc1-worker", any(feature = "freertos-r1-critical-timing", feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "freertos-r1-assert", feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart", feature = "freertos-r2-mixed")))]
compile_error!("Initial proc1 worker owns task8/words96..172; normal R1 only");
#[cfg(feature = "freertos-r3-proc1-worker")]
#[path = "freertos_proc1.rs"]
pub mod proc1;

#[cfg(all(feature = "freertos-r1-critical-timing", any(feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "freertos-r1-assert", feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart", feature = "freertos-r2-mixed")))]
compile_error!("Critical timing initially owns words96..112 in the normal R1 cohort");

#[cfg(all(feature = "freertos-r2-mixed", any(feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "freertos-r1-assert", feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart", feature = "uart0-rx-irq")))]
compile_error!("Mixed R2 owns three tasks/IRQs; standalone workloads are separate");
#[cfg(feature = "freertos-r2-mixed")]
#[path = "freertos_mixed.rs"]
pub mod mixed;

#[cfg(all(feature = "freertos-r1-assert", any(feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "freertos-r1-timer-irq", feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart")))]
compile_error!("C configASSERT is a separate halt/recovery cohort");

#[cfg(all(feature = "freertos-r1-fault", feature = "freertos-r1-panic"))]
compile_error!("Select exactly one deliberate fault mode");
#[cfg(all(feature = "freertos-r1-timer-irq", any(feature = "freertos-r1-fault", feature = "freertos-r1-panic")))]
compile_error!("IRQ wakeup and deliberate faults are separate cohorts");
#[cfg(all(feature = "freertos-r2-spi", any(feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic")))]
compile_error!("Select one task8 workload; keep intentional faults separate");
#[cfg(feature = "freertos-r2-spi")]
#[path = "freertos_spi.rs"]
pub mod spi;
#[cfg(all(feature = "freertos-r2-i2c-nack", any(feature = "freertos-r2-spi", feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic")))]
compile_error!("Select one task8 workload; keep intentional faults separate");
#[cfg(feature = "freertos-r2-i2c-nack")]
#[path = "freertos_i2c.rs"]
pub mod i2c;
#[cfg(all(feature = "freertos-r2-uart", any(feature = "freertos-r2-i2c-nack", feature = "freertos-r2-spi", feature = "freertos-r1-timer-irq", feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "uart0-rx-irq")))]
compile_error!("UART RTOS workload owns task8/vector41; legacy IRQ proof is separate");
#[cfg(all(feature = "freertos-r2-uart-lifecycle", feature = "freertos-r2-uart-overflow"))]
compile_error!("UART timeout/cancel and overflow workloads have separate receipt ABIs");
#[cfg(feature = "freertos-r2-uart")]
#[path = "freertos_uart.rs"]
pub mod uart;

const TELEMETRY: *mut u32 = 0x2000_f800 as *mut u32;
static mut DATA_SENTINEL: u32 = 0x1357_9bdf;
static mut BSS_SENTINEL: u32 = 0;
const TASK_COUNT: usize = if cfg!(any(feature = "freertos-r3-watchdog-arm-receipt", feature = "freertos-r3-proc1-worker", feature = "freertos-r1-timer-irq", feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart", feature = "freertos-r2-mixed")) { 8 } else { 7 };
static mut TASKS: [Option<Task>; TASK_COUNT] = [None; TASK_COUNT];
static mut QUEUE: Option<U32Queue> = None;
static mut CHECK_QUEUE: Option<U32Queue> = None;
static mut SEM: Option<BinarySemaphore> = None;
#[cfg(not(feature = "freertos-r2-mixed"))]
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
        #[cfg(feature = "freertos-r2-mixed-i2c-pair")]
        mixed::pair::prepare();
        ptr::addr_of_mut!(SEM).write(Some(BinarySemaphore::create(0).unwrap()));
        #[cfg(not(feature = "freertos-r2-mixed"))]
        ptr::addr_of_mut!(MUTEX).write(Some(Mutex::create(0).unwrap()));
        #[cfg(not(feature = "freertos-r2-mixed"))]
        let entries: [(os::TaskEntry, &core::ffi::CStr, u32, u32); 7] = [
            // Under 5kHz load, stack scans must share the lowest application
            // priority, not starve either producer or the mutex owner.
            (monitor, c"monitor", if cfg!(feature = "freertos-r1-periodic-200us") { 1 } else { 4 }, 512), (spin, c"spin-a", 1, 128),
            (spin, c"spin-b", 1, 128), (consumer, c"consumer", 3, 256),
            (producer, c"producer", 2, 256), (mutex_low, c"mutex-low", 1, 256),
            (mutex_high, c"mutex-high", 3, 256),
        ];
        #[cfg(feature = "freertos-r2-mixed")]
        let entries: [(os::TaskEntry, &core::ffi::CStr, u32, u32); 8] = [
            // 2560 stack words exactly: no pool growth or reserved-memory change.
            // Separate R1 cohort retains mutex-inheritance stress; not claimed here.
            (monitor, c"monitor", 4, 256), (spin, c"spin-a", 1, 128),
            (spin, c"spin-b", 1, 128), (consumer, c"consumer", 3, 256),
            (producer, c"producer", 2, 256), (mixed::spi_worker, c"spi-rx", 5, 512),
            (mixed::i2c_worker, if cfg!(feature = "freertos-r2-mixed-i2c-pair") {c"i2c-pair"} else {c"i2c-nack"}, 5, 512), (mixed::uart_worker, c"uart-rx", 5, 512),
        ];
        #[cfg(feature = "freertos-r2-mixed")]
        {
            assert!(entries.iter().map(|e| e.3).sum::<u32>() == 2560);
            put(40, u32::from_le_bytes(*b"MIX1")); put(41, 5); put(42, 3);
            put(43, 2560); put(44, 256);
        }
        for (slot, (entry, name, priority, words)) in entries.into_iter().enumerate() {
            let arg = match slot { 1 => 0x2000_f900, 2 => 0x2000_f940, _ => 0 };
            let handle = Task::create(slot as u32, name, entry, arg as *mut c_void, priority, words).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(slot).write(Some(handle));
        }
        #[cfg(feature = "freertos-r1-timer-irq")]
        {
            timer_irq::prepare();
            let handle = Task::create(7, c"timer-irq", timer_irq::worker, ptr::null_mut(), 5, 256).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(7).write(Some(handle));
        }
        #[cfg(feature = "freertos-r2-spi")]
        {
            let handle = Task::create(7, c"spi-rx", spi::worker, ptr::null_mut(), 5, 512).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(7).write(Some(handle));
        }
        #[cfg(feature = "freertos-r2-i2c-nack")]
        {
            let handle = Task::create(7, c"i2c-rx", i2c::worker, ptr::null_mut(), 5, 512).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(7).write(Some(handle));
        }
        put(2, 3);
        #[cfg(feature = "freertos-r2-uart")]
        {
            let handle = Task::create(7, c"uart-rx", uart::worker, ptr::null_mut(), 5, 512).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(7).write(Some(handle));
        }
        #[cfg(feature = "freertos-r3-proc1-worker")]
        {
            let handle = Task::create(7, c"proc1-owner", proc1::worker, ptr::null_mut(), 5, 512).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(7).write(Some(handle));
        }
        #[cfg(feature = "freertos-r3-watchdog-arm-receipt")]
        {
            let handle = Task::create(7, c"wd-receipt", watchdog::worker, ptr::null_mut(), 5, 512).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(7).write(Some(handle));
        }
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
    #[cfg(feature = "freertos-r1-timer-irq")]
    if get(120) == 1 { put(121, id); put(120, 0); }
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
    #[cfg(feature = "freertos-r1-assert")]
    unsafe {
        put(184,(0xe000_ed28 as *const u32).read_volatile()); // CFSR after assertion
        put(185,(0xe000_ed2c as *const u32).read_volatile()); // HFSR after assertion
        let (primask,basepri):(u32,u32);
        core::arch::asm!("mrs {0}, PRIMASK", "mrs {1}, BASEPRI",
            out(reg) primask,out(reg) basepri,options(nomem,nostack));
        put(186,primask);put(187,basepri);put(190,0); // proc0-only build, not a core-ID register read
        put(191,get(10)); // Last actual scheduler switch ID; no lock or C API in halt path.
    }
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
    #[cfg(feature = "freertos-r1-critical-timing")]
    unsafe { critical_timing_calibrate(); }
    #[cfg(feature = "freertos-r1-periodic-200us")]
    unsafe { put(170, task(0).priority().unwrap()); }
    #[cfg(not(feature = "freertos-r2-i2c-peer"))]
    let mut marker = unsafe { ptr::addr_of!(MARKER).read().unwrap() };
    #[cfg(feature = "freertos-r2-i2c-peer")]
    let mut marker:Option<ConfiguredPin<22,Output>>=None;
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
    #[cfg(feature = "freertos-r2-mixed-repeat")]
    let mut wake = unsafe { os::tick().unwrap() };
    #[cfg(feature = "freertos-r2-mixed-repeat")]
    put(63, u32::from_le_bytes(*b"MD01")); // words60..63: monitor-only, not IO/fault.
    loop {
        #[cfg(feature = "freertos-r2-mixed-repeat")]
        let body_start = unsafe {
            if !os::delay_until(&mut wake, 1000).unwrap() {
                put(60, get(60).checked_add(1).unwrap());
                wake = os::tick().unwrap(); // No burst catch-up or unbounded retry.
                continue;
            }
            let late = os::tick().unwrap().wrapping_sub(wake);
            assert!(late < 0x8000_0000);
            put(62, get(62).max(late));
            raw_low()
        };
        #[cfg(not(any(feature = "freertos-r2-mixed-repeat", feature = "freertos-r2-spi-lifecycle", feature = "freertos-r2-i2c-cancel-window", feature = "freertos-r2-uart-lifecycle")))]
        unsafe { os::delay(1000).unwrap(); }
        #[cfg(feature = "freertos-r2-spi-lifecycle")]
        unsafe { spi::lifecycle::monitor_wait(1000); }
        #[cfg(feature = "freertos-r2-i2c-cancel-window")]
        unsafe { i2c::monitor_wait(1000); }
        #[cfg(feature = "freertos-r2-uart-lifecycle")]
        unsafe { uart::lifecycle::monitor_wait(1000); }
        #[cfg(feature = "freertos-r1-periodic-200us")]
        put(168, raw_low()); // Monitor bookkeeping interval, includes preemption.
        let current = [get(64), get(80), get(49), get(50)];
        let checked = if cfg!(feature = "freertos-r2-mixed") { 3 } else { 4 };
        for i in 0..checked { assert_ne!(current[i], previous[i]); }
        previous = current;
        assert_eq!(get(70) | get(86), 0); // assembly context error latches
        #[cfg(feature = "freertos-r1-timer-irq")]
        {
            assert!(get(97) > 0 && get(98) == 0 && get(114) == 0);
            #[cfg(not(feature = "freertos-r1-periodic-200us"))]
            unsafe { put(123, task(7).stack_high_water().unwrap()); }
        }
        #[cfg(any(feature = "freertos-r2-spi", feature = "freertos-r2-i2c-nack", feature = "freertos-r2-uart"))]
        unsafe { put(123, task(7).stack_high_water().unwrap()); }
        unsafe {
            for slot in 0..7 { put(32+slot, task(slot).stack_high_water().unwrap()); }
            #[cfg(feature = "freertos-r2-mixed")]
            put(45, task(7).stack_high_water().unwrap());
            put(22, (0xe000_e014 as *const u32).read_volatile());
        }
        let mut untouched = 0;
        while untouched < 1024 && unsafe { (0x2000_e000 as *const u32).add(untouched).read_volatile() } == 0xa5a5_a5a5 {
            untouched += 1;
        }
        put(18, (1024 - untouched) as u32 * 4);
        assert!(untouched >= 32); // ISR/boot MSP guard remains intact
        #[cfg(feature = "freertos-r1-critical-timing")]
        unsafe { critical_timing_publish(); }
        increment(14); put(27, raw_low()); put(2, 5);
        #[cfg(feature = "freertos-r3-reset-entry-selftest")]
        if unsafe { reset_entry::reenter_pending(&mut marker) } { continue; }
        #[cfg(feature = "freertos-r3-watchdog-postack")]
        if unsafe { watchdog_postack::emit_pending(&mut marker) } { continue; }
        #[cfg(all(feature = "freertos-r3-watchdog-quiescence", not(any(feature = "freertos-r3-watchdog-postack", feature = "freertos-r3-reset-entry-selftest"))))]
        if unsafe { watchdog_quiescence::emit_pending(&mut marker) } { continue; }
        #[cfg(not(feature = "freertos-r2-i2c-peer"))]
        marker.toggle();
        #[cfg(feature = "freertos-r2-mixed-repeat")]
        put(61, get(61).max(raw_low().wrapping_sub(body_start)));
        #[cfg(feature = "freertos-r2-i2c-peer")]
        if get(129)==4 {
            if marker.is_none() { marker=unsafe { ptr::addr_of_mut!(MARKER).replace(None) }; }
            marker.as_mut().unwrap().toggle();
        }
        #[cfg(feature = "freertos-r1-periodic-200us")]
        {
            let end=raw_low(); put(169,end);
            put(171,get(171).max(end.wrapping_sub(get(168)))); increment(172);
        }
        #[cfg(any(feature = "freertos-r1-fault", feature = "freertos-r1-panic", feature = "freertos-r1-assert"))]
        if get(14) == 5 {
            put(41, get(8)); put(42, raw_low());
            #[cfg(feature = "freertos-r1-fault")]
            unsafe {
                put(40, 1);
                // Architected UsageFault enable only, not RP1 reset/POWER MMIO.
                let shcsr = 0xe000_ed24 as *mut u32;
                shcsr.write_volatile(shcsr.read_volatile() | (1 << 18));
                core::arch::asm!("dsb sy", "isb", options(nostack));
                rp1_rtos_fault_probe();
            }
            #[cfg(feature = "freertos-r1-panic")]
            { put(40, 2); panic!("deliberate R1 task panic"); }
            #[cfg(feature = "freertos-r1-assert")]
            { put(40, 3); unsafe { os::trigger_config_assert(); } }
        }
    }
}

#[cfg(feature = "freertos-r1-critical-timing")]
unsafe fn critical_timing_calibrate() {
    unsafe extern "C" {
        fn vPortEnterCritical();
        fn vPortExitCritical();
    }
    unsafe {
        os::critical_timing::start().unwrap();
        put(96, u32::from_le_bytes(*b"CT01")); put(98, 200); put(111, raw_low());
        vPortEnterCritical();
        vPortEnterCritical(); // One outer sample, peak nesting two.
        let start = raw_low();
        let mut elapsed = 0;
        for _ in 0..1_000_000 {
            elapsed = raw_low().wrapping_sub(start);
            if elapsed >= 200 { break; }
        }
        vPortExitCritical();
        vPortExitCritical();
        put(99, elapsed);
        assert!((200..=500).contains(&elapsed));
        let sample = os::critical_timing::snapshot().unwrap();
        put(100, sample.count);
        assert_eq!(sample.count, 1);
        assert_eq!(sample.max_nesting, 2);
        assert!((200..=1000).contains(&sample.max_us));
        put(97, 1);
    }
}

#[cfg(feature = "freertos-r1-critical-timing")]
unsafe fn critical_timing_publish() {
    let sample = unsafe { os::critical_timing::snapshot().unwrap() };
    assert!(sample.count > 1 && sample.saturated == 0);
    let sequence = get(101).wrapping_add(2);
    put(101, sequence | 1);
    unsafe { core::arch::asm!("dsb sy", options(nostack)); }
    for (i, value) in [sample.count, sample.min_us, sample.max_us, sample.last_us,
                      sample.max_nesting, sample.saturated].into_iter().enumerate() {
        put(102+i, value);
    }
    put(112, raw_low());
    unsafe { core::arch::asm!("dsb sy", options(nostack)); }
    put(110, sequence);
    put(101, sequence);
}

/// Selected TIMER0 ALARM0 IRQ26 is already HW-proven. This opt-in cohort tests
/// RTOS FromISR wakeup/priority/latency, not IRQ discovery. Own ALARM0 exclusively;
/// raw timer reads and official SysTick keep their existing independent owners.
#[cfg(all(feature = "freertos-r1-timer-irq", not(feature = "freertos-r1-periodic-200us")))]
mod timer_irq {
    use super::*;
    const BIT: u32 = 1 << 26;
    const PERIOD_US: u32 = 20_000; // Initial IRQ/RTOS admission, not 200us acceptance.
    const ALARM: *mut u32 = 0x400a_c010 as *mut u32;
    const ARMED: *const u32 = 0x400a_c020 as *const u32;
    const INTR: *mut u32 = 0x400a_c034 as *mut u32;
    const INTE: *mut u32 = 0x400a_c038 as *mut u32;
    const INTF: *const u32 = 0x400a_c03c as *const u32;
    const INTS: *const u32 = 0x400a_c040 as *const u32;
    const ENABLE: *mut u32 = 0xe000_e100 as *mut u32;
    const DISABLE: *mut u32 = 0xe000_e180 as *mut u32;
    const PENDING: *mut u32 = 0xe000_e280 as *mut u32;
    const PRIORITY: *mut u8 = 0xe000_e41a as *mut u8;

    fn barrier() { unsafe { core::arch::asm!("dsb sy", "isb", options(nostack)); } }
    unsafe fn mask() { unsafe { DISABLE.write_volatile(BIT); } barrier(); }

    pub unsafe fn prepare() { unsafe {
        // No global VTOR/PRIMASK/BASEPRI or unrelated IRQ priority modification.
        assert_eq!(ENABLE.read_volatile() & BIT, 0);
        assert_eq!(ARMED.read_volatile(), 0);
        assert_eq!(INTE.read_volatile() | INTF.read_volatile() | INTS.read_volatile(), 0);
        assert_eq!(INTR.read_volatile() & !0xf, 0);
        INTR.write_volatile(1); // Known ALARM0 W1C, not unknown ARMED semantics.
        PENDING.write_volatile(BIT);
        PRIORITY.write_volatile(0xc0); // Logical6, lower urgency than max-syscall5.
        barrier();
        assert_eq!(PRIORITY.read_volatile(), 0xc0);
        put(100, u32::from(PRIORITY.read_volatile()));
    } }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler() { unsafe {
        let entered = raw_low();
        let (ipsr, primask, basepri): (u32, u32, u32);
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, PRIMASK", "mrs {2}, BASEPRI",
            out(reg) ipsr, out(reg) primask, out(reg) basepri, options(nomem, nostack));
        let source = INTS.read_volatile();
        INTE.write_volatile(0); // Stop source before notification; bounded one event.
        INTR.write_volatile(1);
        barrier();
        put(99, ipsr); put(102, primask); put(103, basepri); put(104, entered);
        increment(96);
        if source != 1 || ipsr != 42 || get(119) != 1 {
            increment(98); mask(); return;
        }
        put(113, get(112)); put(119, 0);
        put(120, 1); // The next switch hook must select the higher-priority task8.
        if task(7).notification_give_from_isr().is_err() { increment(98); mask(); }
        let end = raw_low(); put(105, end); put(109, get(109).max(end.wrapping_sub(entered)));
    } }

    pub unsafe extern "C" fn worker(_: *mut c_void) {
        let (ipsr, control, psp, msp): (u32, u32, u32, u32);
        unsafe {
            core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
                out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
        }
        for (i, value) in [ipsr, control, psp, msp].into_iter().enumerate() { put(124+i, value); }
        assert_eq!(ipsr, 0); assert_eq!(control & 3, 2); assert_eq!(psp & 7, 0);
        let mut deadline = raw_low().wrapping_add(PERIOD_US);
        loop { unsafe {
            mask();
            assert_eq!(ARMED.read_volatile() & 1, 0);
            assert_eq!((INTR.read_volatile() | INTE.read_volatile() | INTS.read_volatile()) & 1, 0);
            assert_eq!(os::notification_take(true, 0).unwrap(), 0);
            if os::deadline_remaining(raw_low(), deadline).is_none() {
                increment(114); deadline = raw_low().wrapping_add(PERIOD_US);
            }
            increment(112); put(119, 1); put(121, 0); put(110, deadline);
            PENDING.write_volatile(BIT);
            ALARM.write_volatile(deadline);
            INTE.write_volatile(1);
            barrier();
            assert_eq!(ARMED.read_volatile() & 1, 1);
            assert_eq!(INTE.read_volatile(), 1);
            assert_eq!(INTS.read_volatile(), 0);
            ENABLE.write_volatile(BIT);
            barrier();
            let count = os::notification_take(true, 50).unwrap(); // Block, not RX/timer polling.
            let woke = raw_low(); put(106, woke);
            mask(); INTE.write_volatile(0); barrier();
            if count != 1 {
                // Do not write unproven ARMED disarm bits or resume a failed task.
                put(119, 0); increment(98); panic!("TIMER IRQ task timeout");
            }
            assert_eq!(get(113), get(112));
            assert_eq!(get(121), 8); // Immediate portYIELD_FROM_ISR task selection.
            put(122, get(121)); // Persistent completed witness;121 clears on next arm.
            assert_eq!(INTR.read_volatile() & 1, 0);
            assert_eq!(INTS.read_volatile() & 1, 0);
            assert_eq!(get(98), 0);
            let latency = woke.wrapping_sub(get(105));
            put(107, latency); put(108, get(108).max(latency));
            let lateness = woke.wrapping_sub(deadline);
            assert!(lateness < 50_000);
            put(111, get(111).max(lateness));
            increment(97);
            deadline = deadline.wrapping_add(PERIOD_US);
        } }
    }
}

#[cfg(feature = "freertos-r1-periodic-200us")]
#[path = "periodic_200us.rs"]
mod periodic_200us;

/// Proc0 internal timer timestamp acquisition, NOT peripheral/IMU 5kHz sampling.
/// The legacy 20ms cohort above is unchanged. IRQ owns the absolute schedule
/// after the first task arm. A one-event mailbox stays immutable until copied
/// under IRQ26 exclusion. No heap, catch-up loop, logging, or waits in the ISR.
#[cfg(feature = "freertos-r1-periodic-200us")]
mod timer_irq {
    use super::*;
    use super::periodic_200us::{ARM_GUARD_US, PERIOD_US, SLOT_LIMIT, bounded_skip, next_deadline};
    const BIT: u32 = 1 << 26;
    const ALARM: *mut u32 = 0x400a_c010 as *mut u32;
    const ARMED: *const u32 = 0x400a_c020 as *const u32;
    const INTR: *mut u32 = 0x400a_c034 as *mut u32;
    const INTE: *mut u32 = 0x400a_c038 as *mut u32;
    const INTF: *const u32 = 0x400a_c03c as *const u32;
    const INTS: *const u32 = 0x400a_c040 as *const u32;
    const ENABLE: *mut u32 = 0xe000_e100 as *mut u32;
    const DISABLE: *mut u32 = 0xe000_e180 as *mut u32;
    const PENDING: *mut u32 = 0xe000_e280 as *mut u32;
    const PRIORITY: *mut u8 = 0xe000_e41a as *mut u8;

    // 96 entries,97 completions,98 IRQ errors,99 IPSR,100 priority,101 P200,
    // 102 PRIMASK,103 BASEPRI; stable mailbox:104 entry,105 end,110 deadline,
    // 113 entry-sequence,119 ready,141 woken,120/121 switch-hook handshake.
    // Task:106 sample,107/108 last/max end->sample,111 max deadline->sample,
    // 122 completed switch witness,123 stack words,124..127 task context.
    // IRQ:109 max body,112 successful rearms (excludes initial task arm),115 source,116 next deadline,117 ARMED,
    // 118 arm-start timestamp.114 remains zero (legacy late-rearm assertion).
    // 128 period,129 guard,130 publications,131 skipped slots,132 occupied drops,
    // 133 notify errors,134 >=period completion misses,135 nonblocked (woken=false),
    // 136 higher-priority-woken,137 max deadline->entry,138 last deadline->sample,
    // 139 last body,140 last deadline->entry,142 completion timestamp,
    // 143/144 last/max deadline->completion,145 first deadline,
    // 146 state (1 running,2 source gated/draining,3 stable final,4 failed),
    // 147 slot limit,148 completed seq,149 task errors,150 current grid slot,
    // 151 final nominal grid deadline,152 source-stop stamp,153 final task stamp,
    // 154 task failure reason,155 first-arm stamp,156 INTE,157 INTS,
    // 158 post-arm remaining (raw wrapping delta),159 IRQ failure reason.
    // Every counter has one writer (IRQ or task); mailbox/hook flags are the
    // explicit handoff exception. All timestamps/counters are wrapping u32.
    fn barrier() { unsafe { core::arch::asm!("dsb sy", "isb", options(nostack)); } }
    unsafe fn mask() { unsafe { DISABLE.write_volatile(BIT); } barrier(); }

    pub unsafe fn prepare() { unsafe {
        assert_eq!(ENABLE.read_volatile() & BIT, 0);
        assert_eq!(ARMED.read_volatile(), 0);
        assert_eq!(INTE.read_volatile() | INTF.read_volatile() | INTS.read_volatile(), 0);
        assert_eq!(INTR.read_volatile() & !0xf, 0);
        INTR.write_volatile(1);
        PENDING.write_volatile(BIT); // Only before the first alarm, never on rearm.
        PRIORITY.write_volatile(0xc0); // Logical6: the only FromISR caller.
        barrier();
        assert_eq!(PRIORITY.read_volatile(), 0xc0);
        put(100, 0xc0); put(101, u32::from_le_bytes(*b"P200"));
        put(128, PERIOD_US); put(129, ARM_GUARD_US); put(147, SLOT_LIMIT);
    } }

    /// The planned guard can be consumed by hardware/bus delays. Require a
    /// future, still-armed readback; fail visibly, never assume ARMED disarm.
    unsafe fn arm(deadline: u32) -> bool { unsafe {
        let before = raw_low();
        if os::deadline_remaining(before, deadline).is_none() { return false; }
        ALARM.write_volatile(deadline);
        INTE.write_volatile(1);
        barrier();
        let armed = ARMED.read_volatile();
        let enabled = INTE.read_volatile();
        let source = INTS.read_volatile();
        let remaining = deadline.wrapping_sub(raw_low());
        put(118, before); put(117, armed); put(156, enabled);
        put(157, source); put(158, remaining);
        armed & 1 == 1 && enabled == 1 && source == 0
            && remaining != 0 && remaining < 0x8000_0000
    } }

    unsafe fn stop_irq(reason: u32) { unsafe {
        INTE.write_volatile(0); mask();
        increment(98); put(159, reason); put(146, 4);
        // Return with the source gated. Task timeout/monitor halts the cohort;
        // an already-armed comparator is NOT claimed to have been disarmed.
    } }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler() { unsafe {
        let entered = raw_low();
        let (ipsr, primask, basepri): (u32, u32, u32);
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, PRIMASK", "mrs {2}, BASEPRI",
            out(reg) ipsr, out(reg) primask, out(reg) basepri, options(nomem, nostack));
        let source = INTS.read_volatile();
        INTE.write_volatile(0); INTR.write_volatile(1); barrier();
        increment(96); put(99, ipsr); put(102, primask); put(103, basepri); put(115, source);
        if source != 1 || ipsr != 42 || PRIORITY.read_volatile() != 0xc0 {
            stop_irq(1); return;
        }
        let deadline = get(116);
        let lateness = entered.wrapping_sub(deadline);
        if lateness >= 0x8000_0000 { stop_irq(2); return; }
        put(140, lateness); put(137, get(137).max(lateness));
        let Some((next, skipped)) = next_deadline(deadline, raw_low()) else {
            stop_irq(2); return;
        };
        let slot = get(150);
        let Some((skipped, finished)) = bounded_skip(slot, skipped) else {
            stop_irq(6); return;
        };
        put(131, get(131).wrapping_add(skipped));
        if finished {
            // INTE is already gated and current INTR acknowledged. Do not arm
            // outside the cohort, clear pending, or write unknown ARMED bits.
            mask();
            put(117, ARMED.read_volatile()); put(156, INTE.read_volatile());
            put(157, INTS.read_volatile()); put(152, raw_low()); put(146, 2);
        } else {
            put(116, next); put(150, slot + skipped + 1);
            if !arm(next) { stop_irq(3); return; }
            increment(112);
        }
        if get(119) == 0 {
            put(104, entered); put(110, deadline); put(113, get(96)); put(121, 0);
            let Some(receiver) = ptr::addr_of!(TASKS).cast::<Option<Task>>().add(7).read() else {
                increment(133); stop_irq(5); return;
            };
            let woken = match receiver.notification_give_from_isr_woken() {
                Ok(value) => value,
                Err(_) => { increment(133); stop_irq(4); return; }
            };
            put(141, u32::from(woken));
            if woken { increment(136); put(120, 1); }
            else { increment(135); }
            increment(130); put(119, 1);
            // Task cannot run before IRQ return; finalize its stable end stamp.
            put(105, raw_low());
        } else {
            increment(132); // No mailbox timestamp/sequence/witness overwrite.
        }
        let body = raw_low().wrapping_sub(entered);
        put(139, body); put(109, get(109).max(body));
        // End stamps exclude these final telemetry stores and exception return.
    } }

    unsafe fn task_fail(reason: u32) -> ! { unsafe {
        mask(); INTE.write_volatile(0); barrier();
        increment(149); put(154, reason); put(146, 4);
        panic!("periodic 200us task failure");
    } }

    pub unsafe extern "C" fn worker(_: *mut c_void) { unsafe {
        let (ipsr, control, psp, msp): (u32, u32, u32, u32);
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
            out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
        for (i, value) in [ipsr, control, psp, msp].into_iter().enumerate() { put(124+i, value); }
        assert_eq!(ipsr, 0); assert_eq!(control & 3, 2); assert_eq!(psp & 7, 0);
        assert!((0x2000_e000..=0x2000_f000).contains(&msp));
        // New mode has only this task as123 writer; a preempted lower-priority
        // monitor must never resume a stack telemetry store after finalstate3.
        put(123, task(7).stack_high_water().unwrap());
        mask();
        assert_eq!(os::notification_take(true, 0).unwrap(), 0);
        let started = raw_low();
        let first = started.wrapping_add(PERIOD_US);
        put(155, started); put(151, first.wrapping_add((SLOT_LIMIT - 1) * PERIOD_US));
        put(146, 1);
        put(145, first); put(116, first);
        if !arm(first) { task_fail(1); }
        ENABLE.write_volatile(BIT); barrier();
        let mut last_sequence = 0u32;
        loop {
            let count = match os::notification_take(true, 50) {
                Ok(value) => value, Err(_) => task_fail(2),
            };
            let sample = raw_low(); // The actual bounded workload: timer acquisition.
            mask();
            if count != 1 || get(119) != 1 || get(98) != 0 { task_fail(3); }
            let (sequence, deadline, entered, end, woken, witness) =
                (get(113), get(110), get(104), get(105), get(141), get(121));
            put(119, 0); // Release only after all mailbox fields have been copied.
            barrier();
            if get(146) == 1 { ENABLE.write_volatile(BIT); barrier(); }
            // New IRQs may now publish, so use ONLY the copied event fields.
            let advance = sequence.wrapping_sub(last_sequence);
            if advance == 0 || advance >= 0x8000_0000 || (woken != 0 && witness != 8)
                || sample.wrapping_sub(entered) >= 50_000 {
                task_fail(4);
            }
            last_sequence = sequence;
            let completion = raw_low();
            let latency = sample.wrapping_sub(end);
            let lateness = sample.wrapping_sub(deadline);
            let completed_after = completion.wrapping_sub(deadline);
            put(106, sample); put(107, latency); put(108, get(108).max(latency));
            put(138, lateness); put(111, get(111).max(lateness));
            put(142, completion); put(143, completed_after); put(144, get(144).max(completed_after));
            if completed_after >= PERIOD_US { increment(134); }
            put(122, witness); put(148, sequence); increment(97);
            // Completion excludes telemetry bookkeeping; next notification may
            // already be pending, which is counted separately by woken=false.
            if get(146) == 2 {
                // IRQ has permanently gated this finite source. Drain any final
                // notification/mailbox before publishing a stable final ledger.
                mask();
                if get(119) == 0 && get(130) == get(97) {
                    put(123, task(7).stack_high_water().unwrap());
                    put(153, raw_low()); barrier(); put(146, 3); barrier();
                    // The monitor never writes123 in this mode. Only base
                    // telemetry continues; all96..159 stay unchanged hereafter.
                    loop { os::delay(1000).unwrap(); }
                }
            }
        }
    } }
}

/// Known task-frame contents for the halt-only diagnostic/reboot test. No Rust
/// local stack frame, undefined Rust operation, or PC-advance recovery is used.
#[cfg(feature = "freertos-r1-fault")]
#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn rp1_rtos_fault_probe() -> ! {
    core::arch::naked_asm!(
        "movw r0, #0x101", "movw r1, #0x202", "movw r2, #0x303",
        "movw r3, #0x404", "movw r12, #0x1212",
        ".global rp1_rtos_udf_instruction",
        "rp1_rtos_udf_instruction:", "udf #0x51",
        "2:", "b 2b",
    );
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
        #[cfg(feature = "freertos-r1-periodic-200us")]
        { put(160,raw_low()); put(163,get(8)); increment(167); }
        let notification=os::notification_take(true, 100);
        #[cfg(feature = "freertos-r1-periodic-200us")]
        {
            put(161,raw_low()); put(164,get(8));
            put(162,notification.unwrap_or(u32::MAX));
            put(165,expected); put(166,get(48));
        }
        assert_eq!(notification.unwrap(), 1);
        expected = expected.wrapping_add(1);
        assert_eq!(q.receive(10).unwrap(), Some(expected));
        assert!(sem.take(10).unwrap());
        put(49, expected); increment(15);
    } }
}

#[cfg(not(feature = "freertos-r2-mixed"))]
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

#[cfg(not(feature = "freertos-r2-mixed"))]
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

//! Production-service foundation: two proc0 tasks, no synthetic R1 workload.
//! Proc1 is held; a separate admitted control/IRQ handoff is still required.
use super::*;

const MAGIC: u32 = u32::from_le_bytes(*b"AG01");
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".amp_control_contract")]
static RP1_AMP_CONTROL_CONTRACT: [u32; 6] = [MAGIC, 1, 2, 1024, 200, if cfg!(feature = "freertos-amp-proc1") { 1 } else { 0 }];

pub fn run(mut marker: ConfiguredPin<22, Output>) -> ! {
    for i in 0..256 { put(i, 0); }
    put(0, MAGIC); put(1, 1); put(2, 1); put(12, u32::MAX);
    put(42, 2); put(43, 1024); put(44, if cfg!(feature = "freertos-amp-proc1") { 1 } else { 0 }); // No peripheral control claim.
    marker.set_low(); // Safe marker before a future explicit proc1 handoff.
    #[cfg(feature = "freertos-amp-proc1")]
    crate::amp_proc1::handoff_marker(marker);
    unsafe {
        put(19, ptr::addr_of!(DATA_SENTINEL).read_volatile());
        put(20, ptr::addr_of!(BSS_SENTINEL).read_volatile());
        assert_eq!(get(19), 0x1357_9bdf); assert_eq!(get(20), 0);
        let aircr = 0xe000_ed0c as *mut u32;
        aircr.write_volatile(0x05fa_0000 | (aircr.read_volatile() & 0x8000));
        put(21, aircr.read_volatile()); assert_eq!(get(21) & 0x700, 0);
    }
    let hz = calibrate_cpu_hz(); put(5, hz);
    unsafe {
        // Cold initialization is reused unchanged; no alternate PLL writer.
        scmi::prepare().expect("AMP SCMI cold admission failed"); #[cfg(feature = "freertos-clk-sys-200")] let hz = { let _ = hz; clk_sys_200_init() };
        timesync::prepare();
        ddr::prepare();
        rp1_hal::openamp::prepare(timesync::boot_epoch());
        let entries: [(os::TaskEntry, &core::ffi::CStr, u32); 2] = [
            (service, c"amp-service", 2), (openamp_worker, c"amp-bulk", 1),
        ];
        for (slot, (entry, name, priority)) in entries.into_iter().enumerate() {
            let handle = Task::create(slot as u32, name, entry, ptr::null_mut(), priority, 512).unwrap();
            ptr::addr_of_mut!(TASKS).cast::<Option<Task>>().add(slot).write(Some(handle));
        }
        put(2, 3);
        os::start(hz).unwrap();
    }
    panic!("AMP scheduler returned")
}

unsafe extern "C" fn service(_: *mut c_void) {
    let (ipsr, control, psp, msp): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!("mrs {0}, IPSR", "mrs {1}, CONTROL", "mrs {2}, PSP", "mrs {3}, MSP",
            out(reg) ipsr, out(reg) control, out(reg) psp, out(reg) msp, options(nomem, nostack));
    }
    for (i,v) in [ipsr,control,psp,msp].into_iter().enumerate() { put(28+i,v); }
    assert_eq!(ipsr,0); assert_eq!(control & 3,2); assert_eq!(psp & 7,0);
    assert!((0x2000_e000..=0x2000_f000).contains(&msp));
    let mut uart = unsafe { ptr::addr_of_mut!(ENDPOINT_UART).replace(None).unwrap() };
    let mut endpoint = crate::linux_clk_uart_ownership::DbiMonitor::new();
    let start = raw_low();
    let mut gate = crate::linux_clk_uart_ownership::endpoint_gate::Gate::new(start);
    let mut once = crate::endpoint_config::commissioning::Once::new();
    let mut heartbeat_tick = unsafe { os::tick().unwrap() };
    let (mut sequence, mut off) = (0u32, 0u32);
    let mut last = start;
    loop {
        unsafe { os::delay(1).unwrap(); }
        let now = raw_low();
        put(62, get(62).max(now.wrapping_sub(last))); last = now;
        // The one-shot DBI window is finite. Transport RPC/time sync continue
        // every tick after it ends rather than falling back to a one-second wait.
        if now.wrapping_sub(start) < 60_000_000 {
            crate::linux_clk_uart_ownership::endpoint_gate::sample(
                &mut endpoint, &mut gate, u64::from(now.wrapping_sub(start)),
                |address| unsafe { (address as *const u32).read_volatile() }, raw_low,
                |line| increment(if endpoint_line(&mut uart, line) {60} else {61}),
                |low,budget| unsafe { once.candidate(low,budget) });
            endpoint_config_finish(&mut once, &mut uart);
        }
        unsafe { timesync::service(); ddr::service(); }
        let tick = unsafe { os::tick().unwrap() };
        if tick.wrapping_sub(heartbeat_tick) >= 1000 {
            heartbeat_tick = tick;
            unsafe {
                for slot in 0..2 {
                    let free = task(slot).stack_high_water().unwrap();
                    put(32+slot, free); assert!(free>=32);
                }
                put(22, (0xe000_e014 as *const u32).read_volatile());
            }
            let mut untouched = 0;
            while untouched < 1024 && unsafe { (0x2000_e000 as *const u32).add(untouched).read_volatile() } == 0xa5a5_a5a5 {
                untouched += 1;
            }
            put(18, (1024-untouched) as u32*4); assert!(untouched>=32);
            increment(14); put(27,raw_low()); put(2, 5);
            if once.safe_to_report() {
                sequence = sequence.wrapping_add(1);
                let ctrl = unsafe { (0x4001_8054 as *const u32).read_volatile() };
                off += u32::from(ctrl & 0x800 == 0);
                let line = crate::linux_clk_uart_ownership::heartbeat_line(sequence,ctrl,off);
                if !endpoint_line(&mut uart,&line) { increment(61); }
            }
        }
    }
}

// Preserve feature-off panic locations above; keep new cold-only code out of
// the sealed .text/BSS boundary, using the same free gap as the profiler.
#[cfg(feature = "freertos-clk-sys-200")]
#[inline(never)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".clk_sys_init_text")]
fn clk_sys_200_init() -> u32 {
    assert!(rp1_hal::clk_sys::switch(rp1_hal::clk_sys::Source::PllSys200Mhz).is_ok(),
        "AMP clk_sys switch failed");
    let hz = calibrate_cpu_hz();
    put(5, hz);
    hz
}

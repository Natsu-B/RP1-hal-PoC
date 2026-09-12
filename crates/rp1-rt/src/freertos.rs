//! Proc0-only startup. Loader places PT_LOAD bytes at their execution addresses;
//! Reset clears BSS. MSP is separate from the kernel's static PSP task stacks.
unsafe extern "C" {
    pub fn vPortSVCHandler();
    pub fn xPortPendSVHandler();
    pub fn xPortSysTickHandler();
    pub fn rp1_freertos_fault_hook(reason: u32, detail: u32) -> !;
}

#[cfg(feature = "freertos-reset-entry")]
unsafe extern "C" {
    // Capture touches the reserved record, REASON, and the checked known CTRL
    // disable path in expiry experiments.
    // It must not depend on BSS, initialized data, locks or a running kernel.
    fn rp1_freertos_capture_reset_entry() -> u32;
    fn rp1_freertos_reset_entry_halt() -> !;
}
#[cfg(feature = "freertos-warm-data")]
unsafe extern "C" { fn rp1_freertos_warm_start() -> !; }
#[cfg(feature = "freertos-warm-diagnostics")]
unsafe extern "C" { fn rp1_freertos_warm_data_failed() -> !; }

#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() {
    core::arch::naked_asm!(
        "cpsid i",
        "movs r0, #0",
        "msr CONTROL, r0",
        "msr BASEPRI, r0",
        "msr FAULTMASK, r0",
        // Establish the ordinary-memory copy ABI before the first Rust call:
        // unaligned word/halfword loads allowed, exception frames 8-byte aligned.
        // Preserve every other CCR bit. Explicit fault probes may change it later.
        "ldr r0, =0xe000ed14",
        "ldr r1, [r0]",
        "bic r1, r1, #8",
        "orr r1, r1, #0x200",
        "str r1, [r0]",
        "dsb sy",
        "isb",
        "ldr r0, =0x2000e000",
        "ldr r1, =_stack_start",
        "ldr r2, =0xa5a5a5a5",
        "2:",
        "str r2, [r0], #4",
        "cmp r0, r1",
        "blo 2b",
        "msr MSP, r1",
        "isb",
        "b rp1_freertos_reset",
    );
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rp1_freertos_reset() -> ! {
    #[cfg(feature = "freertos-reset-entry")]
    let reentered = unsafe { rp1_freertos_capture_reset_entry() };
    #[cfg(feature = "freertos-warm-data")]
    if !unsafe { super::warm_data::prepare(reentered == 1) } {
        #[cfg(feature = "freertos-warm-diagnostics")]
        unsafe { rp1_freertos_warm_data_failed() }
        #[cfg(not(feature = "freertos-warm-diagnostics"))]
        loop { unsafe { core::arch::asm!("wfe", options(nomem, nostack)); } }
    }
    unsafe {
        super::zero_bss();
        super::configure_vector_table();
        // Reset already established CCR before any Rust/copy operation.
    }
    // Warm kernel mode bypasses PCIe/application initialization. The separate
    // entry-only mode still reports and halts; neither is runtime PCIe reinit.
    #[cfg(feature = "freertos-warm-data")]
    if reentered == 1 { unsafe { rp1_freertos_warm_start() } }
    #[cfg(feature = "freertos-reset-entry")]
    if reentered == 1 { unsafe { rp1_freertos_reset_entry_halt() } }
    #[cfg(feature = "pcie-ep-init")]
    super::pcie_ep_init::init();
    // The example completes the already established PCIe boundary before start.
    unsafe { super::rp1_entry() }
}

// Never call Rust, allocate, lock, or trust a possibly broken task/ISR stack.
// Diagnostic reservation: fb00..fbff; publication magic is written LAST.
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RP1RtosFault() {
    core::arch::naked_asm!(
        "cpsid i",
        "ldr r0, =0x2000fb00",
        "movs r1, #0",
        "str r1, [r0]",
        "mrs r1, IPSR",
        "str r1, [r0, #4]",
        "mrs r1, MSP",
        "str r1, [r0, #8]",
        "mrs r2, PSP",
        "str r2, [r0, #12]",
        "mrs r3, CONTROL",
        "str r3, [r0, #16]",
        "str lr, [r0, #20]",
        "tst lr, #4",
        "it ne",
        "movne r1, r2",
        "ldr r2, =0xe000ed28",
        "ldr r3, [r2]",
        "str r3, [r0, #24]",
        "ldr r3, [r2, #4]",
        "str r3, [r0, #28]",
        "ldr r3, [r2, #12]",
        "str r3, [r0, #32]",
        "ldr r3, [r2, #16]",
        "str r3, [r0, #36]",
        "ldr r2, =0x20000000",
        "cmp r1, r2",
        "blo 3f",
        "ldr r2, =0x2000efe0",
        "cmp r1, r2",
        "bhi 3f",
        "tst r1, #3",
        "bne 3f",
        "movs r2, #40",
        "2:",
        "ldr r3, [r1], #4",
        "str r3, [r0, r2]",
        "adds r2, #4",
        "cmp r2, #72",
        "blo 2b",
        "3:",
        "ldr r1, =0x31544652", // RFT1
        "dsb sy",
        "str r1, [r0]",
        "dsb sy",
        "4:",
        "wfi",
        "b 4b",
    );
}

//! NVIC IRQ57 enable path for the SCMI mailbox doorbell.

#[cfg(target_arch = "arm")]
const SCMI_IRQ_NUMBER: u32 = 57;
#[cfg(target_arch = "arm")]
const SCMI_IRQ_BIT: u32 = 1 << (SCMI_IRQ_NUMBER - 32);
#[cfg(target_arch = "arm")]
const NVIC_ISER1: *mut u32 = 0xe000_e104 as *mut u32;
#[cfg(target_arch = "arm")]
const NVIC_ICER1: *mut u32 = 0xe000_e184 as *mut u32;
#[cfg(target_arch = "arm")]
const NVIC_ICPR1: *mut u32 = 0xe000_e284 as *mut u32;

#[cfg(target_arch = "arm")]
pub unsafe fn enable() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, SCMI_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR1, SCMI_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
        core::ptr::write_volatile(NVIC_ISER1, SCMI_IRQ_BIT);
        core::arch::asm!(
            "dsb sy",
            "isb",
            "cpsie i",
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(not(target_arch = "arm"))]
pub unsafe fn enable() {}

#[cfg(target_arch = "arm")]
pub unsafe fn disable() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, SCMI_IRQ_BIT);
        core::ptr::write_volatile(NVIC_ICPR1, SCMI_IRQ_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(not(target_arch = "arm"))]
pub unsafe fn disable() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn RP1_SCMI_IRQHandler() {
    crate::scmi::handle_doorbell_irq();
}

//! M3 interrupt routing for the RP1 SCMI mailbox service.
//!
//! The stock RP1 firmware binds the SYSCFG/PROC_EVENTS handler at vector index
//! 73, i.e. external NVIC IRQ 57. For this PoC we relocate VTOR to a private
//! 512-byte-aligned table and replace only that vector.

const VECTOR_COUNT: usize = 80;
const CORE_VECTOR_COUNT: usize = 16;
const SCMI_IRQ_NUMBER: usize = 57;
const SCMI_VECTOR_INDEX: usize = CORE_VECTOR_COUNT + SCMI_IRQ_NUMBER;
const SCMI_IRQ_ISER1_BIT: u32 = 1 << (SCMI_IRQ_NUMBER - 32);

const SCB_VTOR: *mut u32 = 0xe000_ed08 as *mut u32;
const NVIC_ISER1: *mut u32 = 0xe000_e104 as *mut u32;
const NVIC_ICER1: *mut u32 = 0xe000_e184 as *mut u32;
const NVIC_ICPR1: *mut u32 = 0xe000_e284 as *mut u32;

#[repr(C, align(512))]
struct AlignedVectorTable([usize; VECTOR_COUNT]);

static mut SCMI_VECTOR_TABLE: AlignedVectorTable = AlignedVectorTable([0; VECTOR_COUNT]);

/// Relocate VTOR, install the SCMI/SYSCFG handler at IRQ57 and unmask it.
///
/// This PoC assumes no other custom external-interrupt vectors need to be
/// preserved. Core exception vectors are copied from the active table and all
/// other external vectors point at rp1-rt's DefaultHandler.
#[cfg(target_arch = "arm")]
pub unsafe fn install_and_enable() {
    let old_base = unsafe { core::ptr::read_volatile(SCB_VTOR) } as *const usize;
    let new_base = unsafe { core::ptr::addr_of_mut!(SCMI_VECTOR_TABLE.0) as *mut usize };

    for index in 0..CORE_VECTOR_COUNT {
        let value = unsafe { core::ptr::read_volatile(old_base.add(index)) };
        unsafe { core::ptr::write_volatile(new_base.add(index), value) };
    }
    for index in CORE_VECTOR_COUNT..VECTOR_COUNT {
        unsafe {
            core::ptr::write_volatile(
                new_base.add(index),
                rp1_rt::DefaultHandler as *const () as usize,
            );
        }
    }
    unsafe {
        core::ptr::write_volatile(
            new_base.add(SCMI_VECTOR_INDEX),
            RP1_SCMI_IRQHandler as *const () as usize,
        );

        core::ptr::write_volatile(NVIC_ICER1, SCMI_IRQ_ISER1_BIT);
        core::ptr::write_volatile(NVIC_ICPR1, SCMI_IRQ_ISER1_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));

        core::ptr::write_volatile(SCB_VTOR, new_base as u32);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));

        core::ptr::write_volatile(NVIC_ISER1, SCMI_IRQ_ISER1_BIT);
        core::arch::asm!(
            "dsb sy",
            "isb",
            "cpsie i",
            options(nostack, preserves_flags)
        );
    }
}

#[cfg(not(target_arch = "arm"))]
pub unsafe fn install_and_enable() {}

#[cfg(target_arch = "arm")]
pub unsafe fn disable() {
    unsafe {
        core::ptr::write_volatile(NVIC_ICER1, SCMI_IRQ_ISER1_BIT);
        core::ptr::write_volatile(NVIC_ICPR1, SCMI_IRQ_ISER1_BIT);
        core::arch::asm!("dsb sy", "isb", options(nostack, preserves_flags));
    }
}

#[cfg(not(target_arch = "arm"))]
pub unsafe fn disable() {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn RP1_SCMI_IRQHandler() {
    crate::scmi::handle_doorbell_irq();
}

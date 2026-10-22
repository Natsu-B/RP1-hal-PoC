//! Registration-only BAR2 bank. No queue access or MMIO selector emulation.
//! Linux 6.12.75 include/uapi/linux/{virtio_mmio.h,virtio_ids.h} and
//! VirtIO 1.2 section 4.2.2: version 2, RPMsg device 7.
//! Feature zero deliberately stops virtio_rpmsg before queue allocation.
use core::ptr;

pub const MAGIC_VALUE: usize = 0x000;
pub const VERSION: usize = 0x004;
pub const DEVICE_ID: usize = 0x008;
pub const VENDOR_ID: usize = 0x00c;
pub const DEVICE_FEATURES: usize = 0x010;
pub const QUEUE_NUM_MAX: usize = 0x034;

#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".virtio_mmio")]
pub static mut RP1_VIRTIO_MMIO: [u32; 64] = [0; 64];
#[used]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".openamp_telemetry")]
pub static mut RP1_OPENAMP_TELEMETRY: [u32; 16] = [0; 16];

const fn probe_words() -> [u32; 64] {
    let mut words = [0; 64];
    words[MAGIC_VALUE / 4] = 0x7472_6976;
    words[VERSION / 4] = 2;
    words[DEVICE_ID / 4] = 7;
    words[VENDOR_ID / 4] = 0; // experimental/unassigned
    words
}

/// # Safety
/// Only during cold firmware startup, before Linux accesses the bank.
#[cfg(target_arch = "arm")]
pub unsafe fn prepare() {
    let bank = ptr::addr_of_mut!(RP1_VIRTIO_MMIO).cast::<u32>();
    let telemetry = ptr::addr_of_mut!(RP1_OPENAMP_TELEMETRY).cast::<u32>();
    for (i, word) in probe_words().into_iter().enumerate() {
        unsafe { bank.add(i).write_volatile(if i == 0 { 0 } else { word }); }
    }
    for i in 0..16 { unsafe { telemetry.add(i).write_volatile(0); } }
    unsafe {
        telemetry.write_volatile(u32::from_le_bytes(*b"VPR1"));
        telemetry.add(1).write_volatile(1); // ABI: registration-only
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        bank.write_volatile(probe_words()[0]);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn registration_stops_before_dma() {
        let words = super::probe_words();
        assert_eq!(&words[..4], &[0x74726976, 2, 7, 0]);
        assert_eq!(words[super::DEVICE_FEATURES / 4], 0);
        assert_eq!(words[super::QUEUE_NUM_MAX / 4], 0);
        assert!(words[4..].iter().all(|v| *v == 0));
    }
}

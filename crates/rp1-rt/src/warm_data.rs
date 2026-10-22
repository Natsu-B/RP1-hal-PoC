//! Preserve the loader-provided .data image across proc0-only warm entry.
//! The shadow is software-read-only after cold publication, not MPU protected.
//! SRAM retention across a hardware reset remains an experimental prerequisite.

const MAGIC: u32 = 0x3144_4d57; // WMD1
const HEADER_BYTES: usize = 12;
const FRESH_TOKEN: [u32; 2] = [0x3852_444c, !0x3852_444c]; // LDR8
const CONSUMED_TOKEN: [u32; 2] = [0x384e_5552, !0x384e_5552]; // RUN8

// Fresh only when the loader really writes this image's .data PT_LOAD bytes.
// This is a fail-closed state guard, not cryptographic proof of cold provenance.
#[cfg(target_arch = "arm")]
#[unsafe(link_section = ".data.warm_loader_token")]
static mut LOADER_TOKEN: [u32; 2] = FRESH_TOKEN;

fn token_allows(warm: bool, token: [u32; 2]) -> bool {
    token == if warm { CONSUMED_TOKEN } else { FRESH_TOKEN }
}

#[cfg(target_arch = "arm")]
unsafe extern "C" {
    static mut __data_start: u8;
    static mut __data_end: u8;
    static mut __warm_data_shadow_start: u32;
    static mut __warm_data_shadow_end: u8;
}

#[cfg(target_arch = "arm")]
unsafe fn ranges() -> Option<(*mut u8, *mut u32, usize)> {
    let data = core::ptr::addr_of_mut!(__data_start);
    let shadow = core::ptr::addr_of_mut!(__warm_data_shadow_start);
    let len = (core::ptr::addr_of_mut!(__data_end) as usize).checked_sub(data as usize)?;
    let capacity =
        (core::ptr::addr_of_mut!(__warm_data_shadow_end) as usize).checked_sub(shadow as usize)?;
    if shadow as usize & 3 != 0
        || len > u32::MAX as usize
        || capacity != len.checked_add(HEADER_BYTES)?
    {
        return None;
    }
    Some((data, shadow, len))
}

/// Cold: capture .data before any runtime mutation. Warm: validate then restore.
/// Returns false on invalid metadata, checksum or exact-copy verification.
///
/// # Safety
/// Call only on proc0 with interrupts masked, before BSS clear/tasks and after
/// capturing the warm-entry nonce. `warm == false` requires a genuine fresh
/// loader copy of .data; never recapture a dirty runtime image. No other core,
/// DMA or host writer may modify either linker-bounded range during this call.
/// Fresh data with a stale valid ARM cookie also fails closed, not as warm.
#[cfg(target_arch = "arm")]
pub unsafe fn prepare(warm: bool) -> bool {
    if !token_allows(warm, unsafe {
        core::ptr::addr_of!(LOADER_TOKEN).read_volatile()
    }) {
        return false; // In particular, a lost cookie does not authorize recapture.
    }
    let Some((data, shadow, len)) = (unsafe { ranges() }) else {
        return false;
    };
    if !unsafe { prepare_region(warm, data, shadow, len) } {
        return false;
    }
    // Cold shadow keeps FRESH; successful warm restore also restores FRESH.
    // Consume before any caller may enter runtime or clear BSS.
    unsafe {
        core::ptr::addr_of_mut!(LOADER_TOKEN).write_volatile(CONSUMED_TOKEN);
    }
    publish_barrier();
    unsafe { core::ptr::addr_of!(LOADER_TOKEN).read_volatile() == CONSUMED_TOKEN }
}

/// Read-only validated `(byte_length, FNV-1a checksum)` of the cold image.
///
/// # Safety
/// The shadow must not be concurrently modified. Does not read runtime .data.
#[cfg(target_arch = "arm")]
pub unsafe fn info() -> Option<(u32, u32)> {
    let (_, shadow, len) = unsafe { ranges()? };
    unsafe { valid_image(shadow, len).map(|digest| (len as u32, digest)) }
}

unsafe fn checksum(bytes: *const u8, len: usize) -> u32 {
    // ponytail: accidental-corruption checksum only; add authentication if inputs become untrusted.
    let mut digest = 0x811c_9dc5u32;
    for index in 0..len {
        digest =
            (digest ^ unsafe { bytes.add(index).read_volatile() } as u32).wrapping_mul(0x0100_0193);
    }
    digest
}

unsafe fn equal(left: *const u8, right: *const u8, len: usize) -> bool {
    for index in 0..len {
        if unsafe { left.add(index).read_volatile() != right.add(index).read_volatile() } {
            return false;
        }
    }
    true
}

unsafe fn valid_image(shadow: *const u32, len: usize) -> Option<u32> {
    if unsafe { shadow.read_volatile() != MAGIC || shadow.add(1).read_volatile() != len as u32 } {
        return None;
    }
    let digest = unsafe { shadow.add(2).read_volatile() };
    let image = unsafe { shadow.cast::<u8>().add(HEADER_BYTES) };
    (unsafe { checksum(image, len) } == digest).then_some(digest)
}

// Caller supplies disjoint regions of len and HEADER_BYTES + len bytes.
unsafe fn prepare_region(warm: bool, data: *mut u8, shadow: *mut u32, len: usize) -> bool {
    let image = unsafe { shadow.cast::<u8>().add(HEADER_BYTES) };
    if warm && unsafe { valid_image(shadow, len).is_none() } {
        return false; // No data or shadow writes before full validation.
    }
    if !warm {
        unsafe {
            shadow.write_volatile(0);
        }
        publish_barrier();
    }
    let (source, destination) = if warm { (image, data) } else { (data, image) };
    for index in 0..len {
        unsafe {
            destination
                .add(index)
                .write_volatile(source.add(index).read_volatile());
        }
    }
    if !unsafe { equal(data, image, len) } {
        return false;
    }
    if !warm {
        unsafe {
            shadow.add(1).write_volatile(len as u32);
            shadow.add(2).write_volatile(checksum(image, len));
        }
        publish_barrier();
        unsafe {
            shadow.write_volatile(MAGIC);
        } // Publication is always last.
    }
    publish_barrier();
    true
}

fn publish_barrier() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_token_rejects_lost_cookie_and_partial_states() {
        assert!(token_allows(false, FRESH_TOKEN));
        assert!(token_allows(true, CONSUMED_TOKEN));
        assert!(!token_allows(true, FRESH_TOKEN)); // Stale valid ARM after loader copy.
        assert!(!token_allows(false, CONSUMED_TOKEN)); // Lost/damaged nonce requests cold.
        for token in [
            [0; 2],
            [u32::MAX; 2],
            [FRESH_TOKEN[0], CONSUMED_TOKEN[1]],
            [CONSUMED_TOKEN[0], FRESH_TOKEN[1]],
        ] {
            assert!(!token_allows(false, token));
            assert!(!token_allows(true, token));
        }
        for base in [FRESH_TOKEN, CONSUMED_TOKEN] {
            for word in 0..2 {
                for bit in 0..32 {
                    let mut damaged = base;
                    damaged[word] ^= 1 << bit;
                    assert!(!token_allows(false, damaged));
                    assert!(!token_allows(true, damaged));
                }
            }
        }
    }

    #[test]
    fn cold_restore_and_reject_corruption_without_writes() {
        let initial = [0xdf, 0x9b, 0x57, 0x13, 0xaa, 0xaa, 0xaa, 0xaa];
        let mut data = initial;
        let mut shadow = [0u32; 5];
        unsafe {
            assert_eq!(checksum(b"hello".as_ptr(), 5), 0x4f9f_2cab);
            assert!(prepare_region(
                false,
                data.as_mut_ptr(),
                shadow.as_mut_ptr(),
                data.len()
            ));
            let published = shadow;
            for _ in 0..2 {
                data.fill(0x55);
                assert!(prepare_region(
                    true,
                    data.as_mut_ptr(),
                    shadow.as_mut_ptr(),
                    data.len()
                ));
                assert_eq!(data, initial);
                assert_eq!(shadow, published); // Warm entry never mutates its source.
            }
            for word in 0..shadow.len() {
                for bit in 0..32 {
                    shadow = published;
                    shadow[word] ^= 1 << bit;
                    let bad = shadow;
                    data.fill(0x55);
                    assert!(!prepare_region(
                        true,
                        data.as_mut_ptr(),
                        shadow.as_mut_ptr(),
                        data.len()
                    ));
                    assert_eq!(data, [0x55; 8]);
                    assert_eq!(shadow, bad);
                }
            }
            let mut empty = [0u32; 3];
            assert!(prepare_region(
                false,
                data.as_mut_ptr(),
                empty.as_mut_ptr(),
                0
            ));
            assert!(prepare_region(
                true,
                data.as_mut_ptr(),
                empty.as_mut_ptr(),
                0
            ));
        }
    }
}

pub const GET_FIRMWARE_VERSION: u16 = 0x0001;
pub const GET_FEATURE: u16 = 0x0002;

pub const FOURCC_PIO: u32 = u32::from_be_bytes(*b"PIO ");

pub const VERSION_NON_PIO: [u32; 5] = [0x5250_3100, 0, 0, 0, 0];

#[repr(C)]
pub struct FeatureRange {
    pub op_base: u32,
    pub op_count: u32,
}

pub fn firmware_version() -> [u32; 5] {
    VERSION_NON_PIO
}

pub fn get_feature(fourcc: u32) -> Option<FeatureRange> {
    match fourcc {
        FOURCC_PIO => None,
        _ => None,
    }
}

pub fn poll() {
    #[cfg(all(
        any(feature = "debug-mailbox-init", feature = "debug-stub"),
        target_arch = "arm"
    ))]
    {
        rp1_rt::debug_stub::poll();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_wire_layout() {
        assert_eq!(FOURCC_PIO, 0x5049_4f20);
        assert_eq!(core::mem::size_of::<FeatureRange>(), 8);
        assert_eq!(core::mem::align_of::<FeatureRange>(), 4);
    }
}

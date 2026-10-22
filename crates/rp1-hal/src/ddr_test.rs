//! Bounded commissioning patterns, not a real-time data plane. No MMIO here.
pub const CAPABILITY: u32 = u32::from_le_bytes(*b"BUL1");
pub const MAX_BYTES: usize = 256;
/// Diagnostic RPCs close permanently when the bulk window is claimed.
pub fn bulk_operation_allowed(op: u32, claimed: bool, diagnostics: bool) -> bool {
    !claimed && if diagnostics { matches!(op, 2|5|6|7) } else { matches!(op, 1|2|5) }
}
pub fn pattern(mode: u32, seed: u32, byte_offset: u32) -> Option<u32> {
    let mut x = seed ^ (byte_offset / 4).wrapping_mul(0x9e37_79b9);
    match mode {
        0 => Some(0), 1 => Some(u32::MAX), 2 => Some(0xaaaa_5555),
        3 => Some(seed.wrapping_add(byte_offset / 4)),
        4 => { x ^= x << 13; x ^= x >> 17; x ^= x << 5; Some(x) },
        _ => None,
    }
}
/// region1 may coexist ONLY for data operations and only at its sealed tuple.
/// Configure/restore remain refused while the transport owns region1.
pub fn peer_region_allowed(op: u32, selector: u32, values: [u32;8], openamp: bool) -> bool {
    values[1] & 0x8000_0000 == 0 ||
        (openamp && selector == 0x43 && matches!(op,3|4|6|7) &&
         values == crate::pcie_outbound::OPENAMP_REGION1)
}
#[cfg(test)]
mod tests {
    #[test] fn bulk_claim_closes_all_diagnostic_operations() {
        for op in 0..12 {
            assert!(!super::bulk_operation_allowed(op,true,true));
            assert!(!super::bulk_operation_allowed(op,true,false));
            assert_eq!(super::bulk_operation_allowed(op,false,false),matches!(op,1|2|5));
            assert_eq!(super::bulk_operation_allowed(op,false,true),matches!(op,2|5|6|7));
        }
    }
    #[test] fn coexistence_is_exact_and_never_grants_reconfiguration() {
        use super::*;
        let expected=crate::pcie_outbound::OPENAMP_REGION1;
        for op in 0..10 { assert_eq!(peer_region_allowed(op,0x43,expected,true),matches!(op,3|4|6|7)); }
        for i in 0..8 { let mut wrong=expected;wrong[i]^=1;assert!(!peer_region_allowed(6,0x43,wrong,true)); }
        assert!(!peer_region_allowed(6,0x83,expected,true));
        assert!(!peer_region_allowed(6,0x43,expected,false));
        for i in 0..65536 { assert_eq!(pattern(3,7,i),Some(7+i/4)); }
        assert_eq!(pattern(5,1,0),None);
    }
}

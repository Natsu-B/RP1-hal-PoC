//! Pure validation for the known ROM's proc0 scratch re-publication.
//! ROM SHA256: 0ebdfc2bdcadecf43b69b563fdac2f3571bf0f68579651d9f9983bd31d2ffd27.
//! Tuple order: magic (0x4015400c), encoded entry (0x40154010), SP (0x40154018).
//! That ROM clears the encoded entry before dispatch. This model neither touches
//! registers nor proves hardware reset, SRAM retention, or scheduler restart.

pub const BOOT_MAGIC: u32 = 0xb007_c0de;
pub const VECTOR_SP: u32 = 0x2000_f000; // Fixed image stack, eight-byte aligned.

pub const fn valid_before(before: [u32; 3], vector_sp: u32, vector_entry: u32) -> bool {
    let address = vector_entry & !1;
    before[0] == BOOT_MAGIC
        && before[1] == 0
        && before[2] == vector_sp
        && vector_sp == VECTOR_SP
        && vector_entry & 1 == 1
        && address >= 0x2000_0000
        && address < 0x2000_c000
}

pub const fn planned_tuple(
    before: [u32; 3],
    vector_sp: u32,
    vector_entry: u32,
) -> Option<[u32; 3]> {
    if !valid_before(before, vector_sp, vector_entry) {
        return None;
    }
    Some([before[0], vector_entry ^ BOOT_MAGIC, before[2]])
}

/// Re-publication ownership/readback requires all three words to match exactly.
pub fn owned_tuple(after: [u32; 3], desired: [u32; 3]) -> bool {
    after == desired
}

pub fn restored_tuple(after: [u32; 3], before: [u32; 3]) -> bool {
    after == before
}

/// Low 24 bits are the counter; only disabled or ENABLE-only top bytes are known.
pub const fn known_ctrl(ctrl: u32) -> bool {
    matches!(ctrl >> 24, 0 | 0x40)
}

pub const fn timer_reason(reason: u32) -> bool {
    matches!(reason, 1 | 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEFORE: [u32; 3] = [BOOT_MAGIC, 0, VECTOR_SP];
    const ENTRY: u32 = 0x2000_0141;

    #[test]
    fn before_and_entry_boundaries() {
        for entry in [0x2000_0001, ENTRY, 0x2000_bfff] {
            assert!(valid_before(BEFORE, VECTOR_SP, entry));
            assert_eq!(
                planned_tuple(BEFORE, VECTOR_SP, entry),
                Some([BOOT_MAGIC, entry ^ BOOT_MAGIC, VECTOR_SP])
            );
        }
        for (before, sp, entry) in [
            ([BOOT_MAGIC ^ 1, 0, VECTOR_SP], VECTOR_SP, ENTRY),
            (
                [BOOT_MAGIC, ENTRY ^ BOOT_MAGIC, VECTOR_SP],
                VECTOR_SP,
                ENTRY,
            ),
            ([BOOT_MAGIC, 0, VECTOR_SP - 8], VECTOR_SP, ENTRY),
            (BEFORE, VECTOR_SP - 8, ENTRY),
            ([BOOT_MAGIC, 0, VECTOR_SP - 8], VECTOR_SP - 8, ENTRY),
            ([BOOT_MAGIC, 0, VECTOR_SP - 1], VECTOR_SP - 1, ENTRY),
            (BEFORE, VECTOR_SP, 0),
            (BEFORE, VECTOR_SP, ENTRY & !1),
            (BEFORE, VECTOR_SP, 0x1fff_ffff),
            (BEFORE, VECTOR_SP, 0x2000_c001),
            (BEFORE, VECTOR_SP, u32::MAX),
        ] {
            assert!(!valid_before(before, sp, entry));
            assert_eq!(planned_tuple(before, sp, entry), None);
        }
    }

    #[test]
    fn one_field_delta_and_exact_readback_restoration() {
        let desired = planned_tuple(BEFORE, VECTOR_SP, ENTRY).unwrap();
        assert_eq!(desired[1] ^ desired[0], ENTRY);
        assert_eq!((0..3).filter(|&i| desired[i] != BEFORE[i]).count(), 1);
        assert!(owned_tuple(desired, desired));
        assert!(!owned_tuple(BEFORE, desired));
        assert!(restored_tuple(BEFORE, BEFORE));
        assert!(!restored_tuple(desired, BEFORE));
        for i in 0..3 {
            for bit in 0..32 {
                let mut corrupt = desired;
                corrupt[i] ^= 1 << bit;
                assert!(!owned_tuple(corrupt, desired));
                let mut corrupt = BEFORE;
                corrupt[i] ^= 1 << bit;
                assert!(!restored_tuple(corrupt, BEFORE));
            }
        }
    }

    #[test]
    fn ctrl_and_reason_reject_unknown_bits() {
        for top in 0..=255 {
            for counter in [0, 1, 0x00ff_ffff] {
                assert_eq!(known_ctrl((top << 24) | counter), top == 0 || top == 0x40);
            }
            assert_eq!(timer_reason(top), top == 1 || top == 3);
        }
        for bit in 8..32 {
            assert!(!timer_reason(1 | (1 << bit)));
            assert!(!timer_reason(3 | (1 << bit)));
        }
    }
}

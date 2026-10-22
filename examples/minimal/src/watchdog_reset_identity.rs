//! Pure, no_std-compatible reset-cookie and GPIO diagnostic encoding.
//! Integration reserves WDT-only telemetry words 136..139 (0x2000fa20..0x2000fa2f).
//! No memory access occurs here; SRAM retention and publication are not assumed.
//! A matching nonce correlates selected diagnostics, not authentication, proof
//! of watchdog cause, or proof that the runtime/scheduler restarted.

pub const COOKIE_FIRST_WORD: usize = 136;
pub const COOKIE_WORDS: usize = 4;
// Fixed little-endian ASCII labels "WARM" and "WENT" distinguish record states.
pub const ARM_MAGIC: u32 = u32::from_le_bytes(*b"WARM");
pub const ENTRY_MAGIC: u32 = u32::from_le_bytes(*b"WENT");
pub const CHECKSUM_SALT: u32 = 0x6d93_b4e1;
pub const ENTRY_PACKET_TYPE: u32 = 0xb; // Leading ONE avoids the observed boot ZERO false start.
pub const MAX_GPIO_EVENTS: usize = 72 + 34 + 66; // prefix + ARM + ENTRY
pub const GPIO_CAPTURE_CAPACITY: usize = 192;

const fn valid_nonce(nonce: u32) -> bool {
    nonce != 0 && nonce <= 0xffff
}

const fn checksum(magic: u32, nonce: u32, field: u32) -> u32 {
    // ponytail: non-cryptographic corruption check; authenticate separately if
    // hostile modification ever enters scope. Rotation keeps each word covered.
    CHECKSUM_SALT ^ magic ^ nonce.rotate_left(7) ^ field.rotate_left(17)
}

/// ARM = [magic, nonzero 16-bit nonce, 32-bit !nonce, checksum].
pub const fn arm_words(nonce: u32) -> Option<[u32; 4]> {
    if !valid_nonce(nonce) {
        return None;
    }
    Some([ARM_MAGIC, nonce, !nonce, checksum(ARM_MAGIC, nonce, !nonce)])
}

pub const fn valid_arm(words: [u32; 4]) -> bool {
    words[0] == ARM_MAGIC
        && valid_nonce(words[1])
        && words[2] == !words[1]
        && words[3] == checksum(words[0], words[1], words[2])
}

/// ENTRY = [magic, nonzero 16-bit nonce, exact raw 32-bit reason, checksum].
/// The reason is recorded without assigning watchdog-cause semantics to it.
pub const fn entry_words(nonce: u32, raw_reason: u32) -> Option<[u32; 4]> {
    if !valid_nonce(nonce) {
        return None;
    }
    Some([
        ENTRY_MAGIC,
        nonce,
        raw_reason,
        checksum(ENTRY_MAGIC, nonce, raw_reason),
    ])
}

pub const fn valid_entry(words: [u32; 4]) -> bool {
    words[0] == ENTRY_MAGIC
        && valid_nonce(words[1])
        && words[3] == checksum(words[0], words[1], words[2])
}

/// Only a valid ARM record can transition to an ENTRY record.
pub const fn entry_from_arm(words: [u32; 4], raw_reason: u32) -> Option<[u32; 4]> {
    if !valid_arm(words) {
        return None;
    }
    entry_words(words[1], raw_reason)
}

const fn check_nibble(word: u32) -> u32 {
    let folded = word ^ (word >> 16);
    let folded = folded ^ (folded >> 8);
    (folded ^ (folded >> 4)) & 0xf
}

/// Packet: type [31:28], nonce [27:12], reason [11:4], XOR check [3:0].
/// All eight nibbles XOR to zero. A reason above 255 is rejected, never truncated.
pub const fn encode_entry(nonce: u32, raw_reason: u32) -> Option<u32> {
    if !valid_nonce(nonce) || raw_reason > 0xff {
        return None;
    }
    let word = (ENTRY_PACKET_TYPE << 28) | (nonce << 12) | (raw_reason << 4);
    Some(word | check_nibble(word))
}

pub const fn decode_entry(word: u32) -> Option<(u16, u8)> {
    let nonce = (word >> 12) & 0xffff;
    if word >> 28 != ENTRY_PACKET_TYPE || !valid_nonce(nonce) || check_nibble(word) != 0 {
        return None;
    }
    Some((nonce as u16, ((word >> 4) & 0xff) as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_boundaries_and_raw_reason_are_preserved() {
        for nonce in [1, 2, 0xff, 0x100, 0xfffe, 0xffff] {
            let arm = arm_words(nonce).unwrap();
            assert!(valid_arm(arm));
            assert_eq!(&arm[..3], &[ARM_MAGIC, nonce, !nonce]);
            assert!(!valid_entry(arm));
            for reason in [0, 1, 0xff, 0x100, 0x8000_0000, u32::MAX] {
                let entry = entry_from_arm(arm, reason).unwrap();
                assert_eq!(Some(entry), entry_words(nonce, reason));
                assert_eq!(&entry[..3], &[ENTRY_MAGIC, nonce, reason]);
                assert!(valid_entry(entry));
                assert!(!valid_arm(entry));
                assert_eq!(entry_from_arm(entry, reason), None);
            }
        }
    }

    #[test]
    fn invalid_nonce_is_rejected_even_with_consistent_checksum() {
        for nonce in [0, 0x1_0000, 0x8000_0000, u32::MAX] {
            assert_eq!(arm_words(nonce), None);
            assert_eq!(entry_words(nonce, 0), None);
            assert_eq!(encode_entry(nonce, 0), None);
            let arm = [ARM_MAGIC, nonce, !nonce, checksum(ARM_MAGIC, nonce, !nonce)];
            let entry = [ENTRY_MAGIC, nonce, 0, checksum(ENTRY_MAGIC, nonce, 0)];
            assert!(!valid_arm(arm));
            assert!(!valid_entry(entry));
            assert_eq!(entry_from_arm(arm, 0), None);
        }
        let nonce = 1;
        let not_complement = [ARM_MAGIC, nonce, nonce, checksum(ARM_MAGIC, nonce, nonce)];
        assert!(!valid_arm(not_complement));
    }

    #[test]
    fn every_single_bit_corruption_is_rejected() {
        for nonce in [1, 0x5a3c, 0xffff] {
            let arm = arm_words(nonce).unwrap();
            for reason in [0, 0xa5a5_5a5a, u32::MAX] {
                let entry = entry_words(nonce, reason).unwrap();
                for word in 0..4 {
                    for bit in 0..32 {
                        let mut damaged_arm = arm;
                        damaged_arm[word] ^= 1 << bit;
                        assert!(!valid_arm(damaged_arm));
                        assert_eq!(entry_from_arm(damaged_arm, reason), None);
                        let mut damaged_entry = entry;
                        damaged_entry[word] ^= 1 << bit;
                        assert!(!valid_entry(damaged_entry));
                    }
                    for mask in [0x0000_ffff, 0xffff_0000, 0xa5a5_5a5a, u32::MAX] {
                        let mut damaged_arm = arm;
                        damaged_arm[word] ^= mask;
                        assert!(!valid_arm(damaged_arm));
                        let mut damaged_entry = entry;
                        damaged_entry[word] ^= mask;
                        assert!(!valid_entry(damaged_entry));
                    }
                }
            }
        }
    }

    #[test]
    fn packet_roundtrips_boundaries_and_rejects_truncation() {
        for nonce in [1, 2, 0xff, 0x100, 0xfffe, 0xffff] {
            for reason in [0, 1, 0x7f, 0x80, 0xfe, 0xff] {
                let packet = encode_entry(nonce, reason).unwrap();
                assert_eq!(decode_entry(packet), Some((nonce as u16, reason as u8)));
                for bit in 0..32 {
                    assert_eq!(decode_entry(packet ^ (1 << bit)), None);
                }
            }
            for reason in [0x100, 0x1_0000, u32::MAX] {
                assert_eq!(encode_entry(nonce, reason), None);
            }
        }
        assert_eq!(encode_entry(1, 0), Some(0xb000_100a));
        assert_eq!(encode_entry(0xffff, 0xff), Some(0xbfff_fffb));
    }

    #[test]
    fn packet_rejects_wrong_type_and_zero_nonce_with_valid_check() {
        for kind in 0..16 {
            if kind != ENTRY_PACKET_TYPE {
                let word = (kind << 28) | (1 << 12);
                assert_eq!(decode_entry(word | check_nibble(word)), None);
            }
        }
        let word = (ENTRY_PACKET_TYPE << 28) | (0xff << 4);
        assert_eq!(decode_entry(word | check_nibble(word)), None);
    }

    #[test]
    fn telemetry_range_and_conservative_capture_budget() {
        assert_eq!(COOKIE_FIRST_WORD, 136);
        assert_eq!(COOKIE_FIRST_WORD + COOKIE_WORDS, 140);
        assert_eq!(MAX_GPIO_EVENTS, 172);
        assert!(MAX_GPIO_EVENTS <= GPIO_CAPTURE_CAPACITY);
    }
}

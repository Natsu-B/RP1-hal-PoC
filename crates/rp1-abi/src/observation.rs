//! OBS1 acquisition trace. A timer wakeup does not establish a sensor edge.
use crate::amp::{Error, Generation, VERSION};

pub const BYTES: usize = 96;
pub const MAGIC: u32 = u32::from_le_bytes(*b"OBS1");
pub const EDGE_CAPTURED: u32 = 1;
pub const TRANSFER_COMPLETED: u32 = 2;
pub const ACTUATOR_UPDATED: u32 = 4;
pub const TIME_SYNCHRONIZED: u32 = 8;
pub const END_TO_END: u32 = EDGE_CAPTURED | TRANSFER_COMPLETED | ACTUATOR_UPDATED;
pub const START: u32 = 11;
pub const QUERY: u32 = 12;
pub const STREAM_REPLY: u32 = 4;
pub const COUNTER_REPLY: u32 = 5;
pub const COUNTER_MAGIC: u32 = u32::from_le_bytes(*b"OBR1");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    pub generation: Generation,
    pub sequence: u64,
    pub edge_us: u64,
    pub irq_us: u64,
    pub transfer_us: u64,
    pub calculation_us: u64,
    pub actuator_us: u64,
    pub sample_timestamp_rp1: u64,
    pub time_sync_generation: u32,
    pub time_sync_uncertainty_us: u32,
    pub flags: u32,
}

impl Observation {
    #[cfg_attr(feature="amp-observation",inline(never))]
    #[cfg_attr(feature="amp-observation",unsafe(link_section=".amp_observation_codec"))]
    pub fn validate(&self) -> Result<(), Error> {
        Generation::new(self.generation.boot, self.generation.session)?;
        if self.sequence == 0 {
            return Err(Error::Sequence);
        }
        if self.flags & !(END_TO_END | TIME_SYNCHRONIZED) != 0 {
            return Err(Error::Flags);
        }
        if self.irq_us > self.transfer_us
            || self.transfer_us > self.calculation_us
            || self.calculation_us > self.actuator_us
            || self.sample_timestamp_rp1 > self.transfer_us
        {
            return Err(Error::Time);
        }
        if self.flags & EDGE_CAPTURED != 0 {
            if self.edge_us > self.irq_us || self.edge_us > self.sample_timestamp_rp1 {
                return Err(Error::Time);
            }
        } else if self.edge_us != 0 {
            return Err(Error::Flags);
        }
        if self.flags & TIME_SYNCHRONIZED != 0 {
            if self.time_sync_generation == 0 || self.time_sync_uncertainty_us == u32::MAX {
                return Err(Error::Time);
            }
        } else if self.time_sync_generation != 0 || self.time_sync_uncertainty_us != u32::MAX {
            return Err(Error::Time);
        }
        Ok(())
    }

    /// Flags attest measured boundaries; the codec cannot verify physical wiring.
    pub fn has_end_to_end(self) -> bool {
        self.flags & END_TO_END == END_TO_END
    }

    /// Words for an aligned DDR writer; this is the OBS1 wire layout, not Rust struct memory.
    pub fn encode_words(self) -> Result<[u32; BYTES / 4], Error> {
        let mut words = [0; BYTES / 4];
        self.encode_words_into(&mut words)?;
        Ok(words)
    }

    /// Encode into the C0 collector's local storage without a second record/word array.
    #[inline(never)]
    pub fn encode_words_into(&self, words: &mut [u32; BYTES / 4]) -> Result<(), Error> {
        self.validate()?;
        words.fill(0);
        words[0] = MAGIC;
        words[1] = VERSION;
        fn pair(words: &mut [u32; BYTES / 4], i: usize, value: u64) {
            words[i] = value as u32;
            words[i + 1] = (value >> 32) as u32;
        }
        pair(words, 2, self.generation.boot);
        pair(words, 4, self.generation.session);
        pair(words, 6, self.sequence);
        pair(words, 8, self.edge_us);
        pair(words, 10, self.irq_us);
        pair(words, 12, self.transfer_us);
        pair(words, 14, self.calculation_us);
        pair(words, 16, self.actuator_us);
        pair(words, 18, self.sample_timestamp_rp1);
        words[20] = self.time_sync_generation;
        words[21] = self.time_sync_uncertainty_us;
        words[22] = self.flags;
        Ok(())
    }

    pub fn encode(self) -> Result<[u8; BYTES], Error> {
        let words = self.encode_words()?;
        let mut bytes = [0; BYTES];
        for (dst, value) in bytes.chunks_exact_mut(4).zip(words) {
            dst.copy_from_slice(&value.to_le_bytes());
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        let words = core::array::from_fn::<_, 24, _>(|i| {
            u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
        });
        if words[0] != MAGIC || words[1] != VERSION {
            return Err(Error::Version);
        }
        if words[23] != 0 {
            return Err(Error::Flags);
        }
        let pair = |i| u64::from(words[i]) | u64::from(words[i + 1]) << 32;
        let result = Self {
            generation: Generation::new(pair(2), pair(4))?,
            sequence: pair(6),
            edge_us: pair(8),
            irq_us: pair(10),
            transfer_us: pair(12),
            calculation_us: pair(14),
            actuator_us: pair(16),
            sample_timestamp_rp1: pair(18),
            time_sync_generation: words[20],
            time_sync_uncertainty_us: words[21],
            flags: words[22],
        };
        result.validate()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    fn record() -> Observation {
        Observation {
            generation: Generation {
                boot: 0x12345678_abcdef01,
                session: 2,
            },
            sequence: 3,
            edge_us: 100,
            irq_us: 103,
            transfer_us: 140,
            calculation_us: 150,
            actuator_us: 180,
            sample_timestamp_rp1: 102,
            time_sync_generation: 0,
            time_sync_uncertainty_us: u32::MAX,
            flags: END_TO_END,
        }
    }
    #[test]
    fn unmeasured_timer_probe_has_no_edge_or_end_to_end_attestation() {
        let mut r = record();
        r.flags = 0;
        r.edge_us = 0;
        assert!(!r.has_end_to_end());
        assert_eq!(Observation::decode(&r.encode().unwrap()).unwrap(), r);
        r.edge_us = r.irq_us;
        assert_eq!(r.encode(), Err(Error::Flags));
    }
    #[test]
    fn round_trip_and_explicit_little_endian() {
        let r = record();
        let b = r.encode().unwrap();
        assert_eq!(&b[..4], b"OBS1");
        assert_eq!(&b[8..16], &r.generation.boot.to_le_bytes());
        assert_eq!(Observation::decode(&b).unwrap(), r);
        let words = r.encode_words().unwrap();
        assert_eq!(words[23], 0);
        for (bytes, word) in b.chunks_exact(4).zip(words) {
            assert_eq!(bytes, word.to_le_bytes());
        }
        if let Some(path) = option_env!("RP1_OBSERVATION_FIXTURE") {
            std::fs::write(path, b).unwrap();
        }
    }
    #[test]
    fn encode_into_rejects_before_changing_output() {
        let mut words = [0xcccccccc; BYTES / 4];
        let mut r = record();
        r.flags = 16;
        assert_eq!(r.encode_words_into(&mut words), Err(Error::Flags));
        assert_eq!(words, [0xcccccccc; BYTES / 4]);
        r.flags = END_TO_END;
        r.encode_words_into(&mut words).unwrap();
        assert_eq!(words, r.encode_words().unwrap());
        assert_eq!(words[23], 0);
    }
    #[test]
    fn malformed_and_missing_boundaries_are_not_end_to_end() {
        let r = record();
        let b = r.encode().unwrap();
        for n in [0, 64, 95] {
            assert_eq!(Observation::decode(&b[..n]), Err(Error::Length));
        }
        for (word, value) in [
            (0usize, 0u32),
            (1, 2),
            (2, 0),
            (3, 0),
            (4, 0),
            (6, 0),
            (8, 104),
            (10, 141),
            (12, 151),
            (14, 181),
            (18, 141),
            (20, 1),
            (21, 0),
            (22, 16),
            (23, 1),
        ] {
            let mut bad = b;
            bad[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
            // A single zero high/low boot word can still describe a valid generation.
            if word == 2 || word == 3 {
                assert!(Observation::decode(&bad).is_ok());
            } else {
                assert!(Observation::decode(&bad).is_err(), "word {word}");
            }
        }
        let mut periodic = r;
        periodic.edge_us = 0;
        periodic.flags = 0;
        assert!(
            !Observation::decode(&periodic.encode().unwrap())
                .unwrap()
                .has_end_to_end()
        );
        for flag in [EDGE_CAPTURED, TRANSFER_COMPLETED, ACTUATOR_UPDATED] {
            let mut partial = r;
            partial.flags &= !flag;
            if flag == EDGE_CAPTURED {
                partial.edge_us = 0;
            }
            assert!(
                !Observation::decode(&partial.encode().unwrap())
                    .unwrap()
                    .has_end_to_end()
            );
        }
    }
    #[test]
    fn timestamp_sync_requires_explicit_generation_and_uncertainty() {
        let mut r = record();
        r.flags |= TIME_SYNCHRONIZED;
        assert_eq!(r.validate(), Err(Error::Time));
        r.time_sync_generation = 1;
        r.time_sync_uncertainty_us = 5;
        assert_eq!(Observation::decode(&r.encode().unwrap()).unwrap(), r);
        r.flags &= !TIME_SYNCHRONIZED;
        assert_eq!(r.validate(), Err(Error::Time));
    }
}

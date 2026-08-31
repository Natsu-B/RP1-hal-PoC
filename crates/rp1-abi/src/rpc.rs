use core::convert::TryFrom;

use crate::debug::snapshot_checksum_update;

pub const VERSION: u32 = 1;
pub const REQUEST_MAGIC: u32 = u32::from_le_bytes(*b"R1RQ");
pub const RESPONSE_MAGIC: u32 = u32::from_le_bytes(*b"R1RS");
pub const TELEMETRY_MAGIC: u32 = u32::from_le_bytes(*b"R1TM");

pub const REQUEST_ADDR: u32 = 0x2000_f900;
pub const RESPONSE_ADDR: u32 = 0x2000_f980;
pub const TELEMETRY_ADDR: u32 = 0x2000_fa00;
pub const REQUEST_WORDS: usize = 32;
pub const RESPONSE_WORDS: usize = 32;
pub const TELEMETRY_WORDS: usize = 64;
pub const HEADER_WORDS: u32 = 16;
pub const TOTAL_WORDS: u32 = 32;
pub const OWNER_WORD: usize = 15;
pub const CHECKSUM_WORD: usize = 14;
pub const OWNER_EMPTY: u32 = 0;
pub const OWNER_READY: u32 = 1;
pub const CHECKSUM_SEED: u32 = 0x5250_4331;
pub const FEATURE_BITMAP: u32 = 0x3b;
pub const PING_BITMAP: u32 = 0x7;

pub mod word {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 1;
    pub const HEADER_WORDS: usize = 2;
    pub const TOTAL_WORDS: usize = 3;
    pub const SEQUENCE: usize = 4;
    pub const OPCODE: usize = 5;
    pub const OBJECT: usize = 6;
    pub const STATUS: usize = 6;
    pub const FLAGS: usize = 7;
    pub const ARG0: usize = 8;
    pub const ARG1: usize = 9;
    pub const ARG2: usize = 10;
    pub const ARG3: usize = 11;
    pub const RESERVED0: usize = 12;
    pub const RESERVED1: usize = 13;
    pub const EFFECTIVE_STATE: usize = 8;
    pub const PHYSICAL_STATE: usize = 9;
    pub const TIMESTAMP_LO: usize = 10;
    pub const TIMESTAMP_HI: usize = 11;
    pub const RESULT0: usize = 12;
    pub const RESULT1: usize = 13;
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opcode {
    Ping = 0,
    GetCapabilities = 1,
    GetClockState = 2,
}

impl TryFrom<u32> for Opcode {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ping),
            1 => Ok(Self::GetCapabilities),
            2 => Ok(Self::GetClockState),
            _ => Err(()),
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockId {
    PllSysPriPh = 6,
    Uart = 15,
}

impl TryFrom<u32> for ClockId {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            6 => Ok(Self::PllSysPriPh),
            15 => Ok(Self::Uart),
            _ => Err(()),
        }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Ok = 0,
    BadMagic = 1,
    BadVersion = 2,
    BadLength = 3,
    BadChecksum = 4,
    BadReserved = 5,
    BadFlags = 6,
    BadOpcode = 7,
    BadObject = 8,
    Busy = 9,
    Replay = 10,
}

pub fn checksum(words: &[u32; REQUEST_WORDS]) -> u32 {
    let mut checksum = CHECKSUM_SEED;
    let mut index = 0;
    while index < REQUEST_WORDS {
        if index != CHECKSUM_WORD && index != OWNER_WORD {
            checksum = snapshot_checksum_update(checksum, words[index]);
        }
        index += 1;
    }
    checksum
}

pub fn stamp_checksum(words: &mut [u32; REQUEST_WORDS]) {
    words[CHECKSUM_WORD] = checksum(words);
}

pub fn valid_checksum(words: &[u32; REQUEST_WORDS]) -> bool {
    words[CHECKSUM_WORD] == checksum(words)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub sequence: u32,
    pub opcode: Opcode,
    pub object: u32,
}

pub fn validate_request(
    words: &[u32; REQUEST_WORDS],
    last_sequence: Option<u32>,
) -> Result<Request, Status> {
    if words[word::MAGIC] != REQUEST_MAGIC {
        return Err(Status::BadMagic);
    }
    if words[word::VERSION] != VERSION {
        return Err(Status::BadVersion);
    }
    if words[word::HEADER_WORDS] != HEADER_WORDS || words[word::TOTAL_WORDS] != TOTAL_WORDS {
        return Err(Status::BadLength);
    }
    if words[word::ARG0] != 0
        || words[word::ARG1] != 0
        || words[word::ARG2] != 0
        || words[word::ARG3] != 0
        || words[word::RESERVED0] != 0
        || words[word::RESERVED1] != 0
        || words[16..].iter().any(|&word| word != 0)
    {
        return Err(Status::BadReserved);
    }
    if words[word::FLAGS] != 0 {
        return Err(Status::BadFlags);
    }
    if !valid_checksum(words) {
        return Err(Status::BadChecksum);
    }
    if last_sequence == Some(words[word::SEQUENCE]) {
        return Err(Status::Replay);
    }

    let opcode = Opcode::try_from(words[word::OPCODE]).map_err(|_| Status::BadOpcode)?;
    match opcode {
        Opcode::Ping | Opcode::GetCapabilities => {
            if words[word::OBJECT] != 0 {
                return Err(Status::BadObject);
            }
        }
        Opcode::GetClockState => {
            ClockId::try_from(words[word::OBJECT]).map_err(|_| Status::BadObject)?;
        }
    }

    Ok(Request {
        sequence: words[word::SEQUENCE],
        opcode,
        object: words[word::OBJECT],
    })
}

pub fn init_response_header(words: &mut [u32; RESPONSE_WORDS]) {
    words.fill(0);
    words[word::MAGIC] = RESPONSE_MAGIC;
    words[word::VERSION] = VERSION;
    words[word::HEADER_WORDS] = HEADER_WORDS;
    words[word::TOTAL_WORDS] = TOTAL_WORDS;
    stamp_response(words);
}

pub fn init_response(
    words: &mut [u32; RESPONSE_WORDS],
    request: &Request,
    status: Status,
    now: u64,
) {
    init_response_raw(words, request.sequence, request.opcode as u32, status, now);
}

pub fn init_response_raw(
    words: &mut [u32; RESPONSE_WORDS],
    sequence: u32,
    opcode: u32,
    status: Status,
    now: u64,
) {
    words.fill(0);
    words[word::MAGIC] = RESPONSE_MAGIC;
    words[word::VERSION] = VERSION;
    words[word::HEADER_WORDS] = HEADER_WORDS;
    words[word::TOTAL_WORDS] = TOTAL_WORDS;
    words[word::SEQUENCE] = sequence;
    words[word::OPCODE] = opcode;
    words[word::STATUS] = status as u32;
    words[word::TIMESTAMP_LO] = now as u32;
    words[word::TIMESTAMP_HI] = (now >> 32) as u32;
}

pub fn stamp_response(words: &mut [u32; RESPONSE_WORDS]) {
    words[CHECKSUM_WORD] = checksum(words);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(opcode: Opcode, object: u32, sequence: u32) -> [u32; REQUEST_WORDS] {
        let mut words = [0u32; REQUEST_WORDS];
        words[word::MAGIC] = REQUEST_MAGIC;
        words[word::VERSION] = VERSION;
        words[word::HEADER_WORDS] = HEADER_WORDS;
        words[word::TOTAL_WORDS] = TOTAL_WORDS;
        words[word::SEQUENCE] = sequence;
        words[word::OPCODE] = opcode as u32;
        words[word::OBJECT] = object;
        stamp_checksum(&mut words);
        words[OWNER_WORD] = OWNER_READY;
        words
    }

    #[test]
    fn checksum_excludes_checksum_and_owner_words() {
        let mut words = request(Opcode::Ping, 0, 1);
        let original = checksum(&words);
        words[CHECKSUM_WORD] ^= 1;
        words[OWNER_WORD] ^= 1;
        assert_eq!(checksum(&words), original);
        words[word::SEQUENCE] ^= 1;
        assert_ne!(checksum(&words), original);
    }

    #[test]
    fn validation_rejects_bad_fields_before_checksum() {
        let mut words = request(Opcode::Ping, 0, 1);
        words[word::FLAGS] = 1;
        assert_eq!(validate_request(&words, None), Err(Status::BadFlags));

        let mut words = request(Opcode::GetClockState, ClockId::Uart as u32, 1);
        words[word::ARG0] = 1;
        assert_eq!(validate_request(&words, None), Err(Status::BadReserved));
    }

    #[test]
    fn validation_detects_checksum_and_replay() {
        let mut words = request(Opcode::Ping, 0, 7);
        words[word::SEQUENCE] = 8;
        assert_eq!(validate_request(&words, None), Err(Status::BadChecksum));

        let words = request(Opcode::Ping, 0, 7);
        assert_eq!(validate_request(&words, Some(7)), Err(Status::Replay));
    }

    #[test]
    fn first_u32_max_sequence_is_valid_then_replays() {
        let words = request(Opcode::Ping, 0, u32::MAX);
        assert_eq!(validate_request(&words, None).unwrap().sequence, u32::MAX);
        assert_eq!(
            validate_request(&words, Some(u32::MAX)),
            Err(Status::Replay)
        );
    }

    #[test]
    fn ping_and_capabilities_nonzero_object_is_bad_object() {
        let words = request(Opcode::Ping, 1, 1);
        assert_eq!(validate_request(&words, None), Err(Status::BadObject));

        let words = request(Opcode::GetCapabilities, 1, 1);
        assert_eq!(validate_request(&words, None), Err(Status::BadObject));
    }

    #[test]
    fn clock_object_allowlist_is_typed() {
        let words = request(Opcode::GetClockState, ClockId::Uart as u32, 1);
        assert_eq!(validate_request(&words, None).unwrap().object, 15);

        let words = request(Opcode::GetClockState, 99, 1);
        assert_eq!(validate_request(&words, None), Err(Status::BadObject));
    }

    #[test]
    fn pinned_wire_constants_match_phase9_contract() {
        assert_eq!(REQUEST_MAGIC, 0x5152_3152);
        assert_eq!(RESPONSE_MAGIC, 0x5352_3152);
        assert_eq!(TELEMETRY_MAGIC, 0x4d54_3152);
        assert_eq!(Opcode::Ping as u32, 0);
        assert_eq!(Opcode::GetCapabilities as u32, 1);
        assert_eq!(Opcode::GetClockState as u32, 2);
        assert_eq!(FEATURE_BITMAP, 0x3b);
        assert_eq!(CHECKSUM_SEED, 0x5250_4331);
    }
}

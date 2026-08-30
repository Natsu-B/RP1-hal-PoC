pub const MAILBOX_ADDR: u32 = 0x2000_fc00;
pub const MAILBOX_SIZE: usize = 1024;
pub const MAILBOX_V1_SIZE: usize = 0x300;
pub const MAILBOX_DATA_LEN: usize = 256;
pub const MAILBOX_REG_COUNT: usize = 18;

pub mod shared_sram {
    pub const APP_START: u32 = 0x2000_0000;
    pub const APP_LEN_V2: usize = 0xf700;
    pub const STACK_TOP_V2: u32 = 0x2000_f700;

    pub const DMA_ADDR: u32 = 0x2000_f700;
    pub const DMA_LEN: usize = 0x100;
    pub const DIAG_ADDR: u32 = 0x2000_f800;
    pub const DIAG_LEN: usize = 0x100;
    pub const RPC_ADDR: u32 = 0x2000_f900;
    pub const RPC_LEN: usize = 0x200;
    pub const SCMI_ADDR: u32 = 0x2000_fb00;
    pub const SCMI_LEN: usize = 0x100;
    pub const D1RP_ADDR: u32 = 0x2000_fc00;
    pub const D1RP_LEN: usize = 0x300;
    pub const OFFICIAL_ADDR: u32 = 0x2000_ff00;
    pub const OFFICIAL_LEN: usize = 0x100;
    pub const END: u32 = 0x2001_0000;
}

pub const SNAPSHOT_CHECKSUM_SEED: u32 = 0x811c_9dc5;
pub const SNAPSHOT_RESPONSE_MAGIC: u32 = u32::from_le_bytes(*b"S1RP");
pub const SNAPSHOT_FORMAT_VERSION: u32 = 2;
pub const SNAPSHOT_MAX_ENTRIES: usize = 8;
pub const SNAPSHOT_RESPONSE_HEADER_WORDS: usize = 6;
pub const SNAPSHOT_ENTRY_WORDS: usize = 3;
pub const SNAPSHOT_RESPONSE_MAX_WORDS: usize =
    SNAPSHOT_RESPONSE_HEADER_WORDS + SNAPSHOT_MAX_ENTRIES * SNAPSHOT_ENTRY_WORDS + 1;
pub const SNAPSHOT_RESPONSE_MAX_LEN: usize = SNAPSHOT_RESPONSE_MAX_WORDS * 4;

pub const MAGIC: u32 = u32::from_le_bytes(*b"D1RP");
pub const VERSION: u32 = 1;

pub mod command {
    pub const NONE: u32 = 0;
    pub const PING: u32 = 1;
    pub const GET_REGS: u32 = 2;
    pub const READ_MEM: u32 = 3;
    pub const WRITE_MEM: u32 = 4;
    pub const CONTINUE: u32 = 5;
    pub const HALT: u32 = 6;
    /// Private RP1 debugger command namespace (`"RPD\0"`).
    pub const PRIVATE_PREFIX: u32 = 0x5250_4400;
    pub const PRIVATE_PREFIX_MASK: u32 = 0xffff_ff00;
    pub const READ_SNAPSHOT_ALLOWLISTED: u32 = PRIVATE_PREFIX | 0x01;
}

pub mod state {
    pub const OFFLINE: u32 = 0;
    pub const RUNNING: u32 = 1;
    pub const STOPPED: u32 = 2;
    pub const FAULTED: u32 = 3;
}

pub mod stop_reason {
    pub const NONE: u32 = 0;
    pub const HOST_HALT: u32 = 1;
    pub const EXCEPTION: u32 = 2;
    pub const PANIC: u32 = 3;
}

pub mod status {
    pub const OK: u32 = 0;
    pub const BAD_COMMAND: u32 = 1;
    pub const BAD_LENGTH: u32 = 2;
    pub const BAD_CHECKSUM: u32 = 3;
    pub const BAD_SNAPSHOT_ID: u32 = 4;
}

pub mod snapshot {
    use super::MAILBOX_ADDR;

    /// Private RP1 debugger snapshot namespace (`"RPS\0"`).
    pub const PRIVATE_PREFIX: u32 = 0x5250_5300;
    pub const PRIVATE_PREFIX_MASK: u32 = 0xffff_ff00;
    pub const CORE_STATUS: u32 = PRIVATE_PREFIX | 0x01;
    pub const PERIPHERAL_STATUS: u32 = PRIVATE_PREFIX | 0x02;
    pub const ENTRY_OK: u32 = 0;

    pub const CORE_STATUS_ADDRESSES: [u32; super::SNAPSHOT_MAX_ENTRIES] = [
        MAILBOX_ADDR,
        MAILBOX_ADDR + 0x04,
        MAILBOX_ADDR + 0x08,
        MAILBOX_ADDR + 0x14,
        MAILBOX_ADDR + 0x18,
        MAILBOX_ADDR + 0x1c,
        MAILBOX_ADDR + 0x20,
        MAILBOX_ADDR + 0x2c,
    ];

    /// Side-effect-free status registers from initialized RP1 peripherals.
    pub const PERIPHERAL_STATUS_ADDRESSES: [u32; super::SNAPSHOT_MAX_ENTRIES] = [
        0x4002_0000, // PLL_SYS.CS
        0x4001_401c, // RESETS.DONE1
        0x400a_c028, // TIMER.RAW_LOW
        0x4009_8000, // PWM0.GLOBAL_CTRL
        0x4009_8060, // PWM0.INTS
        0x4003_0018, // UART0.FR
        0x4007_4070, // I2C1.IC_STATUS
        0x4005_0028, // SPI0.SR
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotEntry {
    pub local_address: u32,
    pub value: u32,
    pub status: u32,
}

pub const fn snapshot_checksum_update(checksum: u32, word: u32) -> u32 {
    (checksum ^ word).rotate_left(5).wrapping_mul(0x9e37_79b1)
}

pub fn snapshot_checksum_words(words: &[u32]) -> u32 {
    let mut checksum = SNAPSHOT_CHECKSUM_SEED;
    for &word in words {
        checksum = snapshot_checksum_update(checksum, word);
    }
    checksum
}

pub fn snapshot_request_checksum(sequence: u32, snapshot_id: u32) -> u32 {
    snapshot_checksum_words(&[
        MAGIC,
        VERSION,
        command::READ_SNAPSHOT_ALLOWLISTED,
        snapshot_id,
        sequence,
    ])
}

pub fn encode_snapshot_response(
    output: &mut [u8],
    snapshot_id: u32,
    sequence: u32,
    response_status: u32,
    entries: &[SnapshotEntry],
) -> Option<usize> {
    if entries.len() > SNAPSHOT_MAX_ENTRIES {
        return None;
    }

    let word_count = SNAPSHOT_RESPONSE_HEADER_WORDS + entries.len() * SNAPSHOT_ENTRY_WORDS + 1;
    let byte_len = word_count * 4;
    if output.len() < byte_len {
        return None;
    }

    let mut word_index = 0usize;
    let mut checksum = SNAPSHOT_CHECKSUM_SEED;
    for word in [
        SNAPSHOT_RESPONSE_MAGIC,
        SNAPSHOT_FORMAT_VERSION,
        snapshot_id,
        sequence,
        response_status,
        entries.len() as u32,
    ] {
        write_word(output, word_index, word);
        checksum = snapshot_checksum_update(checksum, word);
        word_index += 1;
    }

    for entry in entries {
        for word in [entry.local_address, entry.value, entry.status] {
            write_word(output, word_index, word);
            checksum = snapshot_checksum_update(checksum, word);
            word_index += 1;
        }
    }

    write_word(output, word_index, checksum);
    Some(byte_len)
}

pub fn snapshot_response_is_valid(data: &[u8]) -> bool {
    if data.len() < (SNAPSHOT_RESPONSE_HEADER_WORDS + 1) * 4 || data.len() % 4 != 0 {
        return false;
    }
    if read_word(data, 0) != Some(SNAPSHOT_RESPONSE_MAGIC)
        || read_word(data, 1) != Some(SNAPSHOT_FORMAT_VERSION)
    {
        return false;
    }

    let Some(count) = read_word(data, 5).map(|value| value as usize) else {
        return false;
    };
    if count > SNAPSHOT_MAX_ENTRIES {
        return false;
    }
    let expected_words = SNAPSHOT_RESPONSE_HEADER_WORDS + count * SNAPSHOT_ENTRY_WORDS + 1;
    if data.len() != expected_words * 4 {
        return false;
    }

    let mut checksum = SNAPSHOT_CHECKSUM_SEED;
    for index in 0..expected_words - 1 {
        let Some(word) = read_word(data, index) else {
            return false;
        };
        checksum = snapshot_checksum_update(checksum, word);
    }
    read_word(data, expected_words - 1) == Some(checksum)
}

pub fn snapshot_response_word(data: &[u8], index: usize) -> Option<u32> {
    read_word(data, index)
}

fn write_word(output: &mut [u8], index: usize, word: u32) {
    let offset = index * 4;
    output[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

fn read_word(data: &[u8], index: usize) -> Option<u32> {
    let offset = index.checked_mul(4)?;
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DebugMailbox {
    pub magic: u32,
    pub version: u32,
    pub size: u32,
    pub flags: u32,
    pub seq: u32,
    pub ack: u32,
    pub state: u32,
    pub stop_reason: u32,
    pub command: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub status: u32,
    pub regs: [u32; MAILBOX_REG_COUNT],
    pub data_len: u32,
    pub data: [u8; MAILBOX_DATA_LEN],
}

impl DebugMailbox {
    pub const fn new() -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            size: core::mem::size_of::<Self>() as u32,
            flags: 0,
            seq: 0,
            ack: 0,
            state: state::RUNNING,
            stop_reason: stop_reason::NONE,
            command: command::NONE,
            arg0: 0,
            arg1: 0,
            status: status::OK,
            regs: [0; MAILBOX_REG_COUNT],
            data_len: 0,
            data: [0; MAILBOX_DATA_LEN],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_v1_layout_is_unchanged() {
        assert_eq!(core::mem::size_of::<DebugMailbox>(), 380);
        assert!(core::mem::size_of::<DebugMailbox>() <= MAILBOX_V1_SIZE);
        assert_eq!(MAILBOX_ADDR, shared_sram::D1RP_ADDR);
        assert_eq!(
            MAILBOX_ADDR + MAILBOX_V1_SIZE as u32,
            shared_sram::OFFICIAL_ADDR
        );
        assert_eq!(core::mem::offset_of!(DebugMailbox, seq), 16);
        assert_eq!(core::mem::offset_of!(DebugMailbox, ack), 20);
        assert_eq!(core::mem::offset_of!(DebugMailbox, command), 32);
        assert_eq!(core::mem::offset_of!(DebugMailbox, status), 44);
        assert_eq!(core::mem::offset_of!(DebugMailbox, data_len), 120);
        assert_eq!(core::mem::offset_of!(DebugMailbox, data), 124);
    }

    #[test]
    fn shared_sram_v2_layout_is_contiguous() {
        assert_eq!(
            shared_sram::APP_START + shared_sram::APP_LEN_V2 as u32,
            shared_sram::DMA_ADDR
        );
        assert_eq!(
            shared_sram::DMA_ADDR + shared_sram::DMA_LEN as u32,
            shared_sram::DIAG_ADDR
        );
        assert_eq!(
            shared_sram::DIAG_ADDR + shared_sram::DIAG_LEN as u32,
            shared_sram::RPC_ADDR
        );
        assert_eq!(
            shared_sram::RPC_ADDR + shared_sram::RPC_LEN as u32,
            shared_sram::SCMI_ADDR
        );
        assert_eq!(
            shared_sram::SCMI_ADDR + shared_sram::SCMI_LEN as u32,
            shared_sram::D1RP_ADDR
        );
        assert_eq!(
            shared_sram::D1RP_ADDR + shared_sram::D1RP_LEN as u32,
            shared_sram::OFFICIAL_ADDR
        );
        assert_eq!(
            shared_sram::OFFICIAL_ADDR + shared_sram::OFFICIAL_LEN as u32,
            shared_sram::END
        );
        assert_eq!(shared_sram::STACK_TOP_V2, shared_sram::DMA_ADDR);
    }

    #[test]
    fn snapshot_request_checksum_covers_id_and_sequence() {
        let base = snapshot_request_checksum(1, snapshot::CORE_STATUS);
        assert_ne!(base, snapshot_request_checksum(2, snapshot::CORE_STATUS));
        assert_ne!(
            base,
            snapshot_request_checksum(1, snapshot::CORE_STATUS + 1)
        );
    }

    #[test]
    fn debugger_extensions_use_private_namespaces() {
        assert_eq!(
            command::READ_SNAPSHOT_ALLOWLISTED & command::PRIVATE_PREFIX_MASK,
            command::PRIVATE_PREFIX
        );
        assert_eq!(
            snapshot::CORE_STATUS & snapshot::PRIVATE_PREFIX_MASK,
            snapshot::PRIVATE_PREFIX
        );
        assert_eq!(
            snapshot::PERIPHERAL_STATUS & snapshot::PRIVATE_PREFIX_MASK,
            snapshot::PRIVATE_PREFIX
        );
        assert_ne!(command::READ_SNAPSHOT_ALLOWLISTED, 7);
        assert_ne!(snapshot::CORE_STATUS, 1);
    }

    #[test]
    fn snapshot_response_round_trip_and_corruption_detection() {
        let entries = [
            SnapshotEntry {
                local_address: MAILBOX_ADDR,
                value: MAGIC,
                status: snapshot::ENTRY_OK,
            },
            SnapshotEntry {
                local_address: MAILBOX_ADDR + 4,
                value: VERSION,
                status: snapshot::ENTRY_OK,
            },
        ];
        let mut data = [0u8; SNAPSHOT_RESPONSE_MAX_LEN];
        let len =
            encode_snapshot_response(&mut data, snapshot::CORE_STATUS, 7, status::OK, &entries)
                .unwrap();
        assert!(snapshot_response_is_valid(&data[..len]));
        assert_eq!(
            snapshot_response_word(&data[..len], 2),
            Some(snapshot::CORE_STATUS)
        );
        assert_eq!(snapshot_response_word(&data[..len], 3), Some(7));
        assert_eq!(snapshot_response_word(&data[..len], 5), Some(2));

        data[24] ^= 1;
        assert!(!snapshot_response_is_valid(&data[..len]));
    }

    #[test]
    fn rejected_snapshot_response_is_checksummed_and_empty() {
        let mut data = [0u8; SNAPSHOT_RESPONSE_MAX_LEN];
        let len = encode_snapshot_response(&mut data, 0xffff_fffe, 9, status::BAD_SNAPSHOT_ID, &[])
            .unwrap();
        assert!(snapshot_response_is_valid(&data[..len]));
        assert_eq!(
            snapshot_response_word(&data[..len], 4),
            Some(status::BAD_SNAPSHOT_ID)
        );
        assert_eq!(snapshot_response_word(&data[..len], 5), Some(0));
    }

    #[test]
    fn core_status_allowlist_is_bounded_to_mailbox_header() {
        assert_eq!(snapshot::CORE_STATUS_ADDRESSES.len(), SNAPSHOT_MAX_ENTRIES);
        assert!(
            snapshot::CORE_STATUS_ADDRESSES
                .iter()
                .all(|&address| address >= MAILBOX_ADDR && address < MAILBOX_ADDR + 48)
        );
    }

    #[test]
    fn peripheral_status_allowlist_is_exact_and_bounded() {
        assert_eq!(
            snapshot::PERIPHERAL_STATUS_ADDRESSES,
            [
                0x4002_0000,
                0x4001_401c,
                0x400a_c028,
                0x4009_8000,
                0x4009_8060,
                0x4003_0018,
                0x4007_4070,
                0x4005_0028,
            ]
        );
        assert_eq!(
            snapshot::PERIPHERAL_STATUS_ADDRESSES.len(),
            SNAPSHOT_MAX_ENTRIES
        );
    }
}

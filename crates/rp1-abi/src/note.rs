pub const RP1_NOTE_NAME: &[u8; 4] = b"RP1\0";
pub const RP1_NOTE_TYPE_BOOT_V1: u32 = 0x5250_3101;
pub const RP1_NOTE_MAGIC: [u8; 8] = *b"RP1NOTE\0";
pub const RP1_NOTE_ABI_VERSION: u16 = 1;

pub const RP1_VERSION_NON_PIO: u32 = 0;
pub const RP1_VERSION_PIO: u32 = 1;

pub const RP1_MAILBOX_FLAG_ENABLE: u32 = 1 << 0;
pub const RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1: u32 = 1 << 1;
pub const RP1_MAILBOX_FLAGS_SUPPORTED_MASK: u32 =
    RP1_MAILBOX_FLAG_ENABLE | RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rp1NoteHeader {
    pub magic: [u8; 8],
    pub abi_version: u16,
    pub header_size: u16,
    pub flags: u32,
    pub desc_crc32: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rp1BootInfoV1 {
    pub header: Rp1NoteHeader,

    pub entry: u32,
    pub stack_top: u32,
    pub vector_table: u32,

    pub load_base: u32,
    pub image_min_addr: u32,
    pub image_max_addr: u32,

    pub owner_rp1: u64,
    pub owner_linux: u64,
    pub owner_disabled: u64,

    pub mailbox_flags: u32,
    pub firmware_version_kind: u32,

    pub config_hash: [u8; 32],
    pub firmware_hash: [u8; 32],

    pub reserved: [u32; 8],
}

impl Rp1BootInfoV1 {
    pub const SIZE: usize = core::mem::size_of::<Self>();

    pub fn has_supported_abi(&self) -> bool {
        self.header.magic == RP1_NOTE_MAGIC
            && self.header.abi_version == RP1_NOTE_ABI_VERSION
            && self.header.header_size as usize == Self::SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_info_v1_layout_is_unchanged() {
        assert_eq!(core::mem::size_of::<Rp1BootInfoV1>(), 176);
        assert_eq!(core::mem::offset_of!(Rp1BootInfoV1, mailbox_flags), 72);
    }

    #[test]
    fn mailbox_flag_contract_is_stable() {
        assert_eq!(RP1_MAILBOX_FLAG_ENABLE, 0x1);
        assert_eq!(RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1, 0x2);
        assert_eq!(RP1_MAILBOX_FLAGS_SUPPORTED_MASK, 0x3);
    }
}

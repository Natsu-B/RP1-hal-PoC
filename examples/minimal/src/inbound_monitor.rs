#![allow(dead_code)]

use core::hint::spin_loop;

const MAILBOX_ADDR: usize = rp1_hal::debug::MAILBOX_ADDR as usize;
const MAILBOX_SIZE: usize = rp1_hal::debug::MAILBOX_SIZE;

pub const MAGIC: u32 = 0x4d42_4e49; // INBM
#[cfg(not(feature = "rp1-pcie-4k-protection-proof"))]
pub const VERSION: u32 = 2;
#[cfg(all(
    feature = "rp1-pcie-4k-protection-proof",
    not(feature = "rp1-iatu-second-spare-programming-proof"),
    not(feature = "rp1-bar1-interior-64k-hole-proof")
))]
pub const VERSION: u32 = 3;
#[cfg(all(
    feature = "rp1-iatu-second-spare-programming-proof",
    not(feature = "rp1-iatu-64k-address-mask-characterization"),
    not(feature = "rp1-bar1-interior-64k-hole-proof")
))]
pub const VERSION: u32 = 4;
#[cfg(all(
    feature = "rp1-iatu-64k-address-mask-characterization",
    not(feature = "rp1-bar1-interior-64k-hole-proof")
))]
pub const VERSION: u32 = 5;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
pub const VERSION: u32 = 6;

pub const MODE_IDLE: u32 = 1;
pub const MODE_BAR2_READ: u32 = 2;
pub const MODE_BAR2_WRITE: u32 = 3;
pub const MODE_BAR1_READ: u32 = 4;
pub const MODE_BLOCK_BAR1: u32 = 5;
pub const MODE_DONE: u32 = 6;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
pub const MODE_REDIRECT_4K: u32 = 7;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
pub const MODE_HOLE_4K: u32 = 8;
#[cfg(feature = "rp1-pcie-64k-hole-proof")]
pub const MODE_HOLE_64K: u32 = 9;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
pub const MODE_PROGRAM_SECOND_SPARE: u32 = 10;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
pub const MODE_CHARACTERIZE_ADDRESS_MASK: u32 = 11;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
pub const MODE_INTERIOR_HOLE_64K: u32 = 12;

#[cfg(all(
    feature = "rp1-iatu-second-spare-programming-proof",
    feature = "rp1-pcie-64k-hole-proof"
))]
compile_error!("the disabled second-spare proof cannot include active 64 KiB hole modes");

#[cfg(all(
    feature = "rp1-bar1-interior-64k-hole-proof",
    any(
        feature = "rp1-iatu-second-spare-programming-proof",
        feature = "rp1-pcie-64k-hole-proof"
    )
))]
compile_error!("the interior 64 KiB hole proof is a distinct terminal iATU mode");

pub const PHASE_READY: u32 = 1;
pub const PHASE_ACKED: u32 = 2;
pub const PHASE_WAIT_GO: u32 = 3;
pub const PHASE_SAMPLING: u32 = 4;
pub const PHASE_DONE: u32 = 5;
pub const PHASE_QUIET: u32 = 6;

pub const BLOCK_PHASE_IDLE: u32 = 0;
pub const BLOCK_PHASE_PRECONDITION_OK: u32 = 1;
pub const BLOCK_PHASE_PRECONDITION_FAIL: u32 = 2;
pub const BLOCK_PHASE_DISABLED: u32 = 3;
pub const BLOCK_PHASE_RESTORED: u32 = 4;
pub const BLOCK_PHASE_RESTORING: u32 = 5;
pub const BLOCK_PHASE_REJECTED: u32 = 6;

pub const COMPLETION_IDLE: u32 = 0x4944_4c45; // IDLE
pub const COMPLETION_DONE: u32 = 0x444f_4e45; // DONE
pub const COMPLETION_REJECTED: u32 = 0x524a_4354; // RJCT
pub const COMPLETION_PRECONDITION: u32 = 0x5052_4543; // PREC
pub const COMPLETION_GO_TIMEOUT: u32 = 0x544f_5547; // GOUT

const FLAG_CTRL2_PRECONDITION_OK: u32 = 1 << 0;
const FLAG_CTRL2_WRITTEN: u32 = 1 << 1;
const FLAG_CTRL2_BLOCK_READBACK_OK: u32 = 1 << 2;
const FLAG_CTRL2_RESTORED: u32 = 1 << 3;
const FLAG_SELECTOR_RESTORED: u32 = 1 << 4;
const FLAG_SCRATCH_RESTORED: u32 = 1 << 5;

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_DBI_BARS_VALID: u32 = 1 << 0;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_ORIGINAL_BAR1_VALID: u32 = 1 << 1;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_SPARES_UNUSED: u32 = 1 << 2;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_DUMMY_VALID: u32 = 1 << 3;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_PROGRAM_READBACK: u32 = 1 << 4;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_ACTIVE: u32 = 1 << 5;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_SPARE_DISABLED: u32 = 1 << 6;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_SPARE_RESTORED: u32 = 1 << 7;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_SELECTOR_RESTORED: u32 = 1 << 8;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_LOCAL_TARGET_STABLE: u32 = 1 << 9;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_LOCAL_CONTROL_STABLE: u32 = 1 << 10;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const PROTECT_FLAG_ORIGINAL_RESTORED: u32 = 1 << 11;

#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_PRECONDITION: u32 = 1 << 0;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_CTRL2_ZERO_BEFORE: u32 = 1 << 1;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_CTRL2_ZERO_AFTER: u32 = 1 << 2;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_PROGRAM_READBACK: u32 = 1 << 3;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_RESTORED: u32 = 1 << 4;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_SELECTOR_RESTORED: u32 = 1 << 5;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_BAR1_UNCHANGED: u32 = 1 << 6;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_BAR2_UNCHANGED: u32 = 1 << 7;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_A3_UNCHANGED: u32 = 1 << 8;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_ENDPOINT_UNCHANGED: u32 = 1 << 9;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_FLAG_CHIP_ID_STABLE: u32 = 1 << 10;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_REQUIRED_FLAGS: u32 = SECOND_SPARE_FLAG_PRECONDITION
    | SECOND_SPARE_FLAG_CTRL2_ZERO_BEFORE
    | SECOND_SPARE_FLAG_CTRL2_ZERO_AFTER
    | SECOND_SPARE_FLAG_PROGRAM_READBACK
    | SECOND_SPARE_FLAG_RESTORED
    | SECOND_SPARE_FLAG_SELECTOR_RESTORED
    | SECOND_SPARE_FLAG_BAR1_UNCHANGED
    | SECOND_SPARE_FLAG_BAR2_UNCHANGED
    | SECOND_SPARE_FLAG_A3_UNCHANGED
    | SECOND_SPARE_FLAG_ENDPOINT_UNCHANGED
    | SECOND_SPARE_FLAG_CHIP_ID_STABLE;

#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_PRECONDITION: u32 = 1 << 0;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_CTRL2_SAFE_ALL: u32 = 1 << 1;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_ANCHOR_EXACT: u32 = 1 << 2;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_BASE_EXECUTED: u32 = 1 << 3;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_LIMIT_EXECUTED: u32 = 1 << 4;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_TARGET_EXECUTED: u32 = 1 << 5;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_BASE_EXPECTED: u32 = 1 << 6;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_LIMIT_EXPECTED: u32 = 1 << 7;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_TARGET_EXPECTED: u32 = 1 << 8;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_E3_RESTORED: u32 = 1 << 9;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_ACTIVE_REGIONS_UNCHANGED: u32 = 1 << 10;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_ENDPOINT_UNCHANGED: u32 = 1 << 11;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_SELECTOR_RESTORED: u32 = 1 << 12;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_CHIP_STABLE: u32 = 1 << 13;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_FLAG_ANCHOR_RESTORED_BETWEEN: u32 = 1 << 14;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_REQUIRED_FLAGS: u32 = ADDRESS_MASK_FLAG_PRECONDITION
    | ADDRESS_MASK_FLAG_CTRL2_SAFE_ALL
    | ADDRESS_MASK_FLAG_ANCHOR_EXACT
    | ADDRESS_MASK_FLAG_BASE_EXECUTED
    | ADDRESS_MASK_FLAG_LIMIT_EXECUTED
    | ADDRESS_MASK_FLAG_TARGET_EXECUTED
    | ADDRESS_MASK_FLAG_E3_RESTORED
    | ADDRESS_MASK_FLAG_ACTIVE_REGIONS_UNCHANGED
    | ADDRESS_MASK_FLAG_ENDPOINT_UNCHANGED
    | ADDRESS_MASK_FLAG_SELECTOR_RESTORED
    | ADDRESS_MASK_FLAG_CHIP_STABLE
    | ADDRESS_MASK_FLAG_ANCHOR_RESTORED_BETWEEN;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_EXPECTED_FLAGS: u32 = ADDRESS_MASK_FLAG_BASE_EXPECTED
    | ADDRESS_MASK_FLAG_LIMIT_EXPECTED
    | ADDRESS_MASK_FLAG_TARGET_EXPECTED;

#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_PRECONDITION: u32 = 1 << 0;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_A3_UPPER_EXACT: u32 = 1 << 1;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_E3_LOWER_EXACT: u32 = 1 << 2;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_BOTH_READY: u32 = 1 << 3;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_ORIGINAL_DISABLED: u32 = 1 << 4;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_DURING_REACHED: u32 = 1 << 5;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_ORIGINAL_RESTORED: u32 = 1 << 6;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_E3_RESTORED: u32 = 1 << 7;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_A3_RESTORED: u32 = 1 << 8;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_SELECTOR_RESTORED: u32 = 1 << 9;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_BAR2_UNCHANGED: u32 = 1 << 10;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_ENDPOINT_UNCHANGED: u32 = 1 << 11;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_CHIP_ID_STABLE: u32 = 1 << 12;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_FINAL_SNAPSHOTS_EXACT: u32 = 1 << 13;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_FLAG_CLEANUP_COMPLETE: u32 = 1 << 14;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_REQUIRED_FLAGS: u32 = (1 << 15) - 1;

const HEALTH_PCIE_MONITOR_CAPTURED: u32 = 1 << 0;
const HEALTH_AXISHIM_CFG_UNCHANGED: u32 = 1 << 1;
const HEALTH_SAMPLED: u32 = 1 << 2;
const HEALTH_NO_OVERFLOW: u32 = 1 << 3;
const HEALTH_SCRATCH_RESTORED: u32 = 1 << 4;

const SAMPLE_US: u64 = 100_000;
const BLOCK_AT_US: u64 = 20_000;
const BLOCK_HOLD_US: u64 = 50_000;
const GO_TIMEOUT_US: u64 = 1_000_000;
const BAR1_SELECTOR: u32 = 0x23;
const CTRL2_REQUIRED: u32 = 0xc000_0100;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const SPARE_SELECTOR: u32 = 0xa3;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const SECOND_SPARE_SELECTOR: u32 = 0xe3;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const BAR2_SELECTOR: u32 = 0x63;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_ENABLE: u32 = 1 << 31;
#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const SECOND_SPARE_PROGRAMMED: IatuRegionSnapshot = IatuRegionSnapshot {
    selector: SECOND_SPARE_SELECTOR,
    ctrl1: 0,
    ctrl2: 0,
    lower_base: 0x0001_0000,
    upper_base: 0,
    limit: 0x003f_ffff,
    lower_target: 0x4001_0000,
    upper_target: 0x0000_00c0,
    upper_limit: 0,
};
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_BASE_CHALLENGE: u32 = 0x0001_ffff;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_BASE_EXPECTED: u32 = 0x0001_0000;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_LIMIT_CHALLENGE: u32 = 0x003f_0000;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_LIMIT_EXPECTED: u32 = 0x003f_ffff;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_TARGET_CHALLENGE: u32 = 0x4001_ffff;
#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const ADDRESS_MASK_TARGET_EXPECTED: u32 = 0x4001_0000;
#[cfg(all(
    feature = "rp1-bar1-interior-64k-hole-proof",
    not(feature = "rp1-bar1-second-interior-64k-page-proof")
))]
const INTERIOR_HOLE_OFFSET: u32 = 0x0003_0000;
#[cfg(feature = "rp1-bar1-second-interior-64k-page-proof")]
const INTERIOR_HOLE_OFFSET: u32 = 0x0004_0000;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_PID0_LOCAL: u32 = 0x4000_0fe0 + INTERIOR_HOLE_OFFSET;
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_ORIGINAL_BAR1: IatuRegionSnapshot = IatuRegionSnapshot {
    selector: BAR1_SELECTOR,
    ctrl1: 0,
    ctrl2: 0xc000_0100,
    lower_base: 0,
    upper_base: 0,
    limit: 0x0000_ffff,
    lower_target: 0x4000_0000,
    upper_target: 0x0000_00c0,
    upper_limit: 0,
};
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_A3_UPPER: IatuRegionSnapshot = IatuRegionSnapshot {
    selector: SPARE_SELECTOR,
    ctrl1: 0,
    ctrl2: IATU_ENABLE,
    lower_base: INTERIOR_HOLE_OFFSET + 0x0001_0000,
    upper_base: 0,
    limit: 0x003f_ffff,
    lower_target: 0x4000_0000 + INTERIOR_HOLE_OFFSET + 0x0001_0000,
    upper_target: 0x0000_00c0,
    upper_limit: 0,
};
#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const INTERIOR_E3_LOWER: IatuRegionSnapshot = IatuRegionSnapshot {
    selector: SECOND_SPARE_SELECTOR,
    ctrl1: 0,
    ctrl2: IATU_ENABLE,
    lower_base: 0,
    upper_base: 0,
    limit: INTERIOR_HOLE_OFFSET - 1,
    lower_target: 0x4000_0000,
    upper_target: 0x0000_00c0,
    upper_limit: 0,
};
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const DUMMY_MAGIC: u32 = 0x344b_5052;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const DUMMY_CANARY_XOR: u32 = 0xa5a5_5a5a;
const CHECKSUM_SEED: u32 = 0x811c_9dc5;
const CHECKSUM_MUL: u32 = 0x9e37_79b1;
#[cfg(not(feature = "rp1-pcie-4k-protection-proof"))]
const CHECKSUM_WORDS: usize = 142;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const CHECKSUM_WORDS: usize = 241;
const CHECKSUM_EXCLUDED_COMPLETION_SEQ_WORD: usize = 9;
const CHECKSUM_EXCLUDED_CHECKSUM_WORD: usize = 12;
const CHECKSUM_EXCLUDED_ARG0_WORD: usize = 20;
const CHECKSUM_EXCLUDED_ARG1_WORD: usize = 21;

const DBI_SELECTOR: *mut u32 = 0x4010_8000 as *mut u32;
const CTRL2: *mut u32 = 0x4010_9104 as *mut u32;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const DBI_WINDOW: usize = 0x4010_9000;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_CTRL1: usize = DBI_WINDOW + 0x100;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_CTRL2: usize = DBI_WINDOW + 0x104;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_LOWER_BASE: usize = DBI_WINDOW + 0x108;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_UPPER_BASE: usize = DBI_WINDOW + 0x10c;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_LIMIT: usize = DBI_WINDOW + 0x110;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_LOWER_TARGET: usize = DBI_WINDOW + 0x114;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_UPPER_TARGET: usize = DBI_WINDOW + 0x118;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const IATU_UPPER_LIMIT: usize = DBI_WINDOW + 0x120;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const CHIP_ID: usize = 0x4000_0000;
#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const MONITOR2: usize = 0x4010_81a4;
const PCIE_MONITOR: [usize; 3] = [0x4010_819c, 0x4010_81a0, 0x4010_81a4];
const AXISHIM: [(usize, usize); 12] = [
    (0x400c_4000, 0x400c_4040),
    (0x400c_4054, 0x400c_4094),
    (0x400c_40a4, 0x400c_40e4),
    (0x400c_40f4, 0x400c_4134),
    (0x400c_4144, 0x400c_4184),
    (0x400c_4194, 0x400c_41d4),
    (0x400c_41e4, 0x400c_4224),
    (0x400c_4234, 0x400c_4274),
    (0x400c_42d4, 0x400c_4314),
    (0x400c_4324, 0x400c_4364),
    (0x400c_4374, 0x400c_43b4),
    (0x400c_43c4, 0x400c_4404),
];

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RegisterStats {
    pub or_value: u32,
    pub max_value: u32,
    pub count: u32,
    pub first_us: u32,
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BarAssignment {
    pub bar0: u32,
    pub bar1: u32,
    pub bar2: u32,
    pub bar1_bus_base: u32,
    pub command: u32,
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IatuRegionSnapshot {
    pub selector: u32,
    pub ctrl1: u32,
    pub ctrl2: u32,
    pub lower_base: u32,
    pub upper_base: u32,
    pub limit: u32,
    pub lower_target: u32,
    pub upper_target: u32,
    pub upper_limit: u32,
}

#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
const _: () = {
    assert!(SECOND_SPARE_PROGRAMMED.selector == SECOND_SPARE_SELECTOR);
    assert!(SECOND_SPARE_PROGRAMMED.ctrl2 == 0);
    assert!(SECOND_SPARE_PROGRAMMED.lower_base & 0xffff == 0);
    assert!(SECOND_SPARE_PROGRAMMED.limit & 0xffff == 0xffff);
    assert!(SECOND_SPARE_PROGRAMMED.lower_target & 0xffff == 0);
};

#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
const _: () = {
    assert!(ADDRESS_MASK_BASE_CHALLENGE == 0x0001_ffff);
    assert!(ADDRESS_MASK_BASE_EXPECTED == SECOND_SPARE_PROGRAMMED.lower_base);
    assert!(ADDRESS_MASK_BASE_EXPECTED == ADDRESS_MASK_BASE_CHALLENGE & 0xffff_0000);
    assert!(ADDRESS_MASK_LIMIT_CHALLENGE == 0x003f_0000);
    assert!(ADDRESS_MASK_LIMIT_EXPECTED == SECOND_SPARE_PROGRAMMED.limit);
    assert!(ADDRESS_MASK_LIMIT_EXPECTED == ADDRESS_MASK_LIMIT_CHALLENGE | 0x0000_ffff);
    assert!(ADDRESS_MASK_TARGET_CHALLENGE == 0x4001_ffff);
    assert!(ADDRESS_MASK_TARGET_EXPECTED == SECOND_SPARE_PROGRAMMED.lower_target);
    assert!(ADDRESS_MASK_TARGET_EXPECTED == ADDRESS_MASK_TARGET_CHALLENGE & 0xffff_0000);
};

#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const _: () = {
    assert!(INTERIOR_HOLE_OFFSET & 0x0000_ffff == 0);
    assert!(INTERIOR_PID0_LOCAL == 0x4000_0fe0 + INTERIOR_HOLE_OFFSET);
    assert!(INTERIOR_ORIGINAL_BAR1.ctrl2 == CTRL2_REQUIRED);
    assert!(interior_original_bar1_exact(&INTERIOR_ORIGINAL_BAR1));
    assert!(!interior_original_bar1_exact(&IatuRegionSnapshot {
        limit: 0,
        ..INTERIOR_ORIGINAL_BAR1
    }));
    assert!(INTERIOR_A3_UPPER.selector == SPARE_SELECTOR);
    assert!(INTERIOR_A3_UPPER.ctrl2 == IATU_ENABLE);
    assert!(INTERIOR_A3_UPPER.lower_base == INTERIOR_HOLE_OFFSET + 0x0001_0000);
    assert!(INTERIOR_A3_UPPER.limit == 0x003f_ffff);
    assert!(INTERIOR_A3_UPPER.lower_target == 0x4000_0000 + INTERIOR_HOLE_OFFSET + 0x0001_0000);
    assert!(INTERIOR_E3_LOWER.selector == SECOND_SPARE_SELECTOR);
    assert!(INTERIOR_E3_LOWER.ctrl2 == IATU_ENABLE);
    assert!(INTERIOR_E3_LOWER.lower_base == 0);
    assert!(INTERIOR_E3_LOWER.limit == INTERIOR_HOLE_OFFSET - 1);
    assert!(INTERIOR_E3_LOWER.lower_target == 0x4000_0000);
};

#[cfg(feature = "rp1-bar1-second-interior-64k-page-proof")]
const _: () = {
    assert!(INTERIOR_HOLE_OFFSET == 0x0004_0000);
    assert!(INTERIOR_PID0_LOCAL == 0x4004_0fe0);
    assert!(INTERIOR_A3_UPPER.lower_base == 0x0005_0000);
    assert!(INTERIOR_A3_UPPER.lower_target == 0x4005_0000);
    assert!(INTERIOR_E3_LOWER.limit == 0x0003_ffff);
};

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
#[repr(C)]
pub struct Protection4kResult {
    pub target_page_offset: u32,
    pub bar1_bus_base: u32,
    pub dummy_local_base: u32,
    pub original_bar1: IatuRegionSnapshot,
    pub spare_before: [IatuRegionSnapshot; 2],
    pub spare_programmed: [IatuRegionSnapshot; 2],
    pub spare_readback: [IatuRegionSnapshot; 2],
    pub spare_restored: [IatuRegionSnapshot; 2],
    pub enable_us: u32,
    pub disable_us: u32,
    pub restore_us: u32,
    pub target_before: u32,
    pub target_during: u32,
    pub target_after: u32,
    pub control_before: u32,
    pub control_during: u32,
    pub control_after: u32,
    pub flags: u32,
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
struct ProtectionContext {
    saved_selector: u32,
    bars: BarAssignment,
    original: IatuRegionSnapshot,
    spares: [IatuRegionSnapshot; 2],
    dummy_base: u32,
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
#[repr(C, align(4096))]
struct AlignedPage([u32; 1024]);

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
const _: () = assert!(core::mem::size_of::<AlignedPage>() == 0x1000);

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
#[unsafe(link_section = ".inbound_dummy_page")]
#[used]
static mut INBOUND_DUMMY_PAGE: AlignedPage = AlignedPage([0; 1024]);

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe extern "C" {
    static __inbound_dummy_page_start: u8;
    static __inbound_dummy_page_end: u8;
}

#[repr(C)]
pub struct InboundMonitorMailbox {
    pub magic: u32,
    pub version: u32,
    pub size: u32,
    pub seq: u32,
    pub ack: u32,
    pub go: u32,
    pub mode: u32,
    pub phase: u32,
    pub completion: u32,
    pub completion_seq: u32,
    pub flags: u32,
    pub result: u32,
    pub checksum: u32,
    pub overflow_count: u32,
    pub started_us_lo: u32,
    pub started_us_hi: u32,
    pub ended_us_lo: u32,
    pub ended_us_hi: u32,
    pub elapsed_us: u32,
    pub sample_count: u32,
    pub arg0: u32,
    pub arg1: u32,
    pub health_flags: u32,
    pub config_change_count: u32,
    pub monitor0_or: u32,
    pub monitor0_max: u32,
    pub monitor1_or: u32,
    pub monitor1_max: u32,
    pub monitor2_or: u32,
    pub monitor2_max: u32,
    pub monitor2_bit23_count: u32,
    pub monitor2_bit22_count: u32,
    pub monitor2_bit21_count: u32,
    pub monitor2_bit23_first_us: u32,
    pub monitor2_bit22_first_us: u32,
    pub monitor2_bit21_first_us: u32,
    pub scratch_restore_ok: u32,
    pub scratch_change_count: u32,
    pub scratch_last_change_us: u32,
    pub block_phase: u32,
    pub block_disable_us: u32,
    pub block_restore_us: u32,
    pub selector_saved: u32,
    pub selector_restore_readback: u32,
    pub ctrl2_before: u32,
    pub ctrl2_block_value: u32,
    pub ctrl2_block_readback: u32,
    pub ctrl2_restore_readback: u32,
    pub pcie_cfg_before: [u32; 3],
    pub pcie_cfg_after: [u32; 3],
    pub axishim_cfg_before: [u32; 12],
    pub axishim_cfg_after: [u32; 12],
    pub axishim_status: [RegisterStats; 12],
    pub scratch: [u32; 4],
    pub scratch_initial: [u32; 4],
    pub scratch_last: [u32; 4],
    pub scratch_final: [u32; 4],
    #[cfg(feature = "rp1-pcie-4k-protection-proof")]
    pub bar_assignment: BarAssignment,
    #[cfg(feature = "rp1-pcie-4k-protection-proof")]
    pub protection: Protection4kResult,
    #[cfg(feature = "rp1-pcie-4k-protection-proof")]
    pub reserved: [u32; 15],
    #[cfg(not(feature = "rp1-pcie-4k-protection-proof"))]
    pub reserved: [u32; 114],
}

const _: () = assert!(core::mem::size_of::<InboundMonitorMailbox>() == MAILBOX_SIZE);

#[inline(always)]
unsafe fn read32(address: usize) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
#[inline(always)]
unsafe fn write32(address: usize, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) };
    dsb();
}

#[inline(always)]
fn dsb() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

#[inline(always)]
fn raw_timer_us() -> u64 {
    const RAW_HIGH: *const u32 = 0x400a_c024 as *const u32;
    const RAW_LOW: *const u32 = 0x400a_c028 as *const u32;

    loop {
        let high_before = unsafe { core::ptr::read_volatile(RAW_HIGH) };
        let low = unsafe { core::ptr::read_volatile(RAW_LOW) };
        let high_after = unsafe { core::ptr::read_volatile(RAW_HIGH) };
        if high_before == high_after {
            return (u64::from(high_before) << 32) | u64::from(low);
        }
    }
}

#[inline(always)]
fn elapsed32(start: u64) -> u32 {
    core::cmp::min(raw_timer_us().wrapping_sub(start), u64::from(u32::MAX)) as u32
}

#[inline(always)]
unsafe fn mailbox() -> *mut InboundMonitorMailbox {
    MAILBOX_ADDR as *mut InboundMonitorMailbox
}

unsafe fn write_phase(m: *mut InboundMonitorMailbox, phase: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).phase), phase);
        dsb();
    }
}

unsafe fn write_block_phase(m: *mut InboundMonitorMailbox, phase: u32) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).block_phase), phase);
        dsb();
    }
}

unsafe fn initialize_mailbox() {
    let m = unsafe { mailbox() };
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).magic), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).version), VERSION);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).size),
            core::mem::size_of::<InboundMonitorMailbox>() as u32,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ack), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).phase), PHASE_READY);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).completion), COMPLETION_IDLE);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).completion_seq), 0);
        dsb();
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).magic), MAGIC);
        dsb();
    }
}

unsafe fn clear_result(m: *mut InboundMonitorMailbox) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).completion), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).completion_seq), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).flags), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).result), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).checksum), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).overflow_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_hi), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ended_us_lo), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ended_us_hi), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).elapsed_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).sample_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).health_flags), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).config_change_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor0_or), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor0_max), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor1_or), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor1_max), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_or), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_max), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_bit23_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_bit22_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_bit21_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_bit23_first_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_bit22_first_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_bit21_first_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_restore_ok), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_change_count), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_last_change_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).block_phase), BLOCK_PHASE_IDLE);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).block_disable_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).block_restore_us), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).selector_saved), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).selector_restore_readback), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_before), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_value), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_readback), 0);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_restore_readback), 0);
        for index in 0..12 {
            let stats = core::ptr::addr_of_mut!((*m).axishim_status[index]);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*stats).or_value), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*stats).max_value), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*stats).count), 0);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*stats).first_us), 0);
        }
        #[cfg(feature = "rp1-pcie-4k-protection-proof")]
        for index in 142..241 {
            core::ptr::write_volatile(m.cast::<u32>().add(index), 0);
        }
    }
}

unsafe fn read_config_before(m: *mut InboundMonitorMailbox) {
    unsafe {
        for (index, address) in PCIE_MONITOR.into_iter().enumerate() {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).pcie_cfg_before[index]),
                read32(address),
            );
        }
        for (index, (config, _status)) in AXISHIM.into_iter().enumerate() {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).axishim_cfg_before[index]),
                read32(config),
            );
        }
    }
}

unsafe fn read_config_after(m: *mut InboundMonitorMailbox) {
    unsafe {
        for (index, address) in PCIE_MONITOR.into_iter().enumerate() {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).pcie_cfg_after[index]),
                read32(address),
            );
        }
        for (index, (config, _status)) in AXISHIM.into_iter().enumerate() {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).axishim_cfg_after[index]),
                read32(config),
            );
        }
    }
}

unsafe fn count_axishim_config_changes(m: *mut InboundMonitorMailbox) -> u32 {
    let mut axishim_changes = 0u32;
    unsafe {
        for index in 0..12 {
            let before =
                core::ptr::read_volatile(core::ptr::addr_of!((*m).axishim_cfg_before[index]));
            let after =
                core::ptr::read_volatile(core::ptr::addr_of!((*m).axishim_cfg_after[index]));
            axishim_changes = axishim_changes.wrapping_add(u32::from(before != after));
        }
    }
    axishim_changes
}

unsafe fn capture_scratch_initial(m: *mut InboundMonitorMailbox) -> [u32; 4] {
    let mut scratch = [0u32; 4];
    unsafe {
        for (index, value) in scratch.iter_mut().enumerate() {
            *value = core::ptr::read_volatile(core::ptr::addr_of!((*m).scratch[index]));
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_initial[index]), *value);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_last[index]), *value);
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_final[index]), *value);
        }
    }
    scratch
}

unsafe fn capture_bar1_ctrl2_precondition(m: *mut InboundMonitorMailbox) -> bool {
    unsafe {
        let selector_saved = core::ptr::read_volatile(DBI_SELECTOR);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).selector_saved), selector_saved);
        core::ptr::write_volatile(DBI_SELECTOR, BAR1_SELECTOR);
        dsb();
        let ctrl2_before = core::ptr::read_volatile(CTRL2);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_before), ctrl2_before);
        core::ptr::write_volatile(DBI_SELECTOR, selector_saved);
        dsb();
        let selector_restore = core::ptr::read_volatile(DBI_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_restore,
        );
        ctrl2_before == CTRL2_REQUIRED && selector_restore == selector_saved
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
fn snapshot_equal(a: &IatuRegionSnapshot, b: &IatuRegionSnapshot) -> bool {
    a.selector == b.selector
        && a.ctrl1 == b.ctrl1
        && a.ctrl2 == b.ctrl2
        && a.lower_base == b.lower_base
        && a.upper_base == b.upper_base
        && a.limit == b.limit
        && a.lower_target == b.lower_target
        && a.upper_target == b.upper_target
        && a.upper_limit == b.upper_limit
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
fn spare_snapshot_unused(value: &IatuRegionSnapshot, selector: u32) -> bool {
    value.selector == selector
        && value.ctrl1 == 0
        && value.ctrl2 == 0
        && value.lower_base == 0
        && value.upper_base == 0
        // Read-only commissioning on this RP1 established the disabled-region
        // reset contract: LIMIT retains 0x0000ffff while every other payload
        // field is zero.  Preserve and restore this exact snapshot.
        && value.limit == 0x0000_ffff
        && value.lower_target == 0
        && value.upper_target == 0
        && value.upper_limit == 0
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn snapshot_iatu(selector: u32) -> IatuRegionSnapshot {
    unsafe {
        if !select_dbi_exact(selector) {
            return IatuRegionSnapshot {
                selector: u32::MAX,
                ctrl1: u32::MAX,
                ctrl2: u32::MAX,
                lower_base: u32::MAX,
                upper_base: u32::MAX,
                limit: u32::MAX,
                lower_target: u32::MAX,
                upper_target: u32::MAX,
                upper_limit: u32::MAX,
            };
        }
        IatuRegionSnapshot {
            selector,
            ctrl1: read32(IATU_CTRL1),
            ctrl2: read32(IATU_CTRL2),
            lower_base: read32(IATU_LOWER_BASE),
            upper_base: read32(IATU_UPPER_BASE),
            limit: read32(IATU_LIMIT),
            lower_target: read32(IATU_LOWER_TARGET),
            upper_target: read32(IATU_UPPER_TARGET),
            upper_limit: read32(IATU_UPPER_LIMIT),
        }
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn select_dbi_exact(selector: u32) -> bool {
    unsafe {
        for _ in 0..3 {
            core::ptr::write_volatile(DBI_SELECTOR, selector);
            dsb();
            if core::ptr::read_volatile(DBI_SELECTOR) == selector {
                return true;
            }
        }
        false
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn program_iatu(value: &IatuRegionSnapshot) -> bool {
    unsafe {
        if !select_dbi_exact(value.selector) {
            return false;
        }
        write32(IATU_CTRL2, value.ctrl2 & !IATU_ENABLE);
        write32(IATU_LOWER_BASE, value.lower_base);
        write32(IATU_UPPER_BASE, value.upper_base);
        write32(IATU_LIMIT, value.limit);
        write32(IATU_UPPER_LIMIT, value.upper_limit);
        write32(IATU_LOWER_TARGET, value.lower_target);
        write32(IATU_UPPER_TARGET, value.upper_target);
        write32(IATU_CTRL1, value.ctrl1);
        write32(IATU_CTRL2, value.ctrl2);
        true
    }
}

#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
unsafe fn program_disabled_second_spare(value: &IatuRegionSnapshot) -> (bool, bool) {
    unsafe {
        if value.selector != SECOND_SPARE_SELECTOR
            || value.ctrl2 != 0
            || !select_dbi_exact(SECOND_SPARE_SELECTOR)
        {
            return (false, false);
        }

        write32(IATU_CTRL2, 0);
        let ctrl2_zero_before = read32(IATU_CTRL2) == 0;
        if ctrl2_zero_before {
            write32(IATU_LOWER_BASE, value.lower_base);
            write32(IATU_UPPER_BASE, value.upper_base);
            write32(IATU_LIMIT, value.limit);
            write32(IATU_UPPER_LIMIT, value.upper_limit);
            write32(IATU_LOWER_TARGET, value.lower_target);
            write32(IATU_UPPER_TARGET, value.upper_target);
            write32(IATU_CTRL1, value.ctrl1);
        }
        write32(IATU_CTRL2, 0);
        (ctrl2_zero_before, read32(IATU_CTRL2) == 0)
    }
}

#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
unsafe fn challenge_disabled_second_spare(
    address: usize,
    value: u32,
) -> Option<IatuRegionSnapshot> {
    unsafe {
        if !matches!(address, IATU_LOWER_BASE | IATU_LIMIT | IATU_LOWER_TARGET)
            || !select_dbi_exact(SECOND_SPARE_SELECTOR)
        {
            return None;
        }

        write32(IATU_CTRL2, 0);
        if read32(IATU_CTRL2) != 0 {
            return None;
        }
        write32(address, value);
        if core::ptr::read_volatile(DBI_SELECTOR) != SECOND_SPARE_SELECTOR {
            return None;
        }
        write32(IATU_CTRL2, 0);
        if read32(IATU_CTRL2) != 0 || !select_dbi_exact(SECOND_SPARE_SELECTOR) {
            return None;
        }
        let readback = snapshot_iatu(SECOND_SPARE_SELECTOR);
        if readback.selector == SECOND_SPARE_SELECTOR && readback.ctrl2 == 0 {
            Some(readback)
        } else {
            None
        }
    }
}

#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
unsafe fn restore_second_spare_anchor() -> bool {
    unsafe {
        let (ctrl2_zero_before, ctrl2_zero_after) =
            program_disabled_second_spare(&SECOND_SPARE_PROGRAMMED);
        ctrl2_zero_before
            && ctrl2_zero_after
            && snapshot_equal(
                &SECOND_SPARE_PROGRAMMED,
                &snapshot_iatu(SECOND_SPARE_SELECTOR),
            )
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn capture_bar_assignment(saved_selector: u32) -> BarAssignment {
    unsafe {
        if !select_dbi_exact(0) {
            return BarAssignment {
                bar0: u32::MAX,
                bar1: u32::MAX,
                bar2: u32::MAX,
                bar1_bus_base: u32::MAX,
                command: u32::MAX,
            };
        }
        let command = read32(DBI_WINDOW + 0x004);
        let bar0 = read32(DBI_WINDOW + 0x010);
        let bar1 = read32(DBI_WINDOW + 0x014);
        let bar2 = read32(DBI_WINDOW + 0x018);
        let _ = select_dbi_exact(saved_selector);
        BarAssignment {
            bar0,
            bar1,
            bar2,
            bar1_bus_base: bar1 & !0xf,
            command,
        }
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
fn bar_assignment_valid(value: &BarAssignment) -> bool {
    value.bar0 == 0x0080_0000
        && value.bar1 == 0
        && value.bar2 == 0x0040_0000
        && value.bar1_bus_base == 0
        && value.command & 0x2 != 0
}

#[cfg(any(
    feature = "rp1-iatu-second-spare-programming-proof",
    feature = "rp1-bar1-interior-64k-hole-proof"
))]
const fn bar_assignment_equal(a: &BarAssignment, b: &BarAssignment) -> bool {
    a.bar0 == b.bar0
        && a.bar1 == b.bar1
        && a.bar2 == b.bar2
        && a.bar1_bus_base == b.bar1_bus_base
        // DBI +0x004 is PCI Command (low 16) plus volatile PCI Status (high 16).
        && (a.command & 0xffff) == (b.command & 0xffff)
}

#[cfg(any(
    feature = "rp1-iatu-second-spare-programming-proof",
    feature = "rp1-bar1-interior-64k-hole-proof"
))]
const _: () = {
    let before = BarAssignment {
        bar0: 0x0080_0000,
        bar1: 0,
        bar2: 0x0040_0000,
        bar1_bus_base: 0,
        command: 0x0010_0406,
    };
    assert!(bar_assignment_equal(
        &before,
        &BarAssignment {
            command: 0x0020_0406,
            ..before
        }
    ));
    assert!(!bar_assignment_equal(
        &before,
        &BarAssignment {
            command: 0x0010_0404,
            ..before
        }
    ));
    assert!(!bar_assignment_equal(
        &before,
        &BarAssignment { bar2: 0, ..before }
    ));
};

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
fn original_bar1_valid(value: &IatuRegionSnapshot) -> bool {
    value.selector == BAR1_SELECTOR
        && value.ctrl1 == 0
        && value.ctrl2 == CTRL2_REQUIRED
        && value.lower_target == 0x4000_0000
        && value.upper_target == 0x0000_00c0
}

#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
const fn interior_original_bar1_exact(value: &IatuRegionSnapshot) -> bool {
    value.selector == INTERIOR_ORIGINAL_BAR1.selector
        && value.ctrl1 == INTERIOR_ORIGINAL_BAR1.ctrl1
        && value.ctrl2 == INTERIOR_ORIGINAL_BAR1.ctrl2
        && value.upper_base == INTERIOR_ORIGINAL_BAR1.upper_base
        && value.lower_base == INTERIOR_ORIGINAL_BAR1.lower_base
        && value.limit == INTERIOR_ORIGINAL_BAR1.limit
        && value.upper_target == INTERIOR_ORIGINAL_BAR1.upper_target
        && value.lower_target == INTERIOR_ORIGINAL_BAR1.lower_target
        && value.upper_limit == INTERIOR_ORIGINAL_BAR1.upper_limit
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
fn dummy_page_bounds() -> Option<(usize, usize)> {
    let start = core::ptr::addr_of!(__inbound_dummy_page_start) as usize;
    let end = core::ptr::addr_of!(__inbound_dummy_page_end) as usize;
    if start & 0xfff == 0
        && end.wrapping_sub(start) == 0x1000
        && start >= 0x2000_0000
        && end <= 0x2000_f800
        && end <= MAILBOX_ADDR
    {
        Some((start, end))
    } else {
        None
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn prepare_dummy_page(m: *mut InboundMonitorMailbox, seq: u32) {
    let Some((start, _end)) = dummy_page_bounds() else {
        return;
    };
    let words = start as *mut u32;
    unsafe {
        for index in 0..1024usize {
            let value = match index {
                0 => DUMMY_MAGIC,
                1 => seq,
                2 => !seq,
                3 => 0,
                _ => {
                    (start as u32)
                        .wrapping_add((index as u32).wrapping_mul(4))
                        .rotate_left(7)
                        ^ DUMMY_CANARY_XOR
                }
            };
            core::ptr::write_volatile(words.add(index), value);
        }
        let mut checksum = CHECKSUM_SEED;
        for index in 0..1024usize {
            let value = if index == 3 {
                0
            } else {
                core::ptr::read_volatile(words.add(index))
            };
            checksum = (checksum ^ value).rotate_left(5).wrapping_mul(CHECKSUM_MUL);
        }
        core::ptr::write_volatile(words.add(3), checksum);
        dsb();
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_page_offset),
            0,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.dummy_local_base),
            start as u32,
        );
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn dummy_page_valid(seq: u32, start: usize) -> bool {
    if dummy_page_bounds().map(|v| v.0) != Some(start) {
        return false;
    }
    let words = start as *const u32;
    let mut checksum = CHECKSUM_SEED;
    unsafe {
        for index in 0..1024usize {
            let actual = core::ptr::read_volatile(words.add(index));
            let expected = match index {
                0 => DUMMY_MAGIC,
                1 => seq,
                2 => !seq,
                3 => 0,
                _ => {
                    (start as u32)
                        .wrapping_add((index as u32).wrapping_mul(4))
                        .rotate_left(7)
                        ^ DUMMY_CANARY_XOR
                }
            };
            if index != 3 && actual != expected {
                return false;
            }
            checksum = (checksum ^ expected)
                .rotate_left(5)
                .wrapping_mul(CHECKSUM_MUL);
        }
        core::ptr::read_volatile(words.add(3)) == checksum
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn protection_precondition(
    m: *mut InboundMonitorMailbox,
    seq: u32,
) -> Option<ProtectionContext> {
    unsafe {
        let saved_selector = core::ptr::read_volatile(DBI_SELECTOR);
        let bars = capture_bar_assignment(saved_selector);
        let original = snapshot_iatu(BAR1_SELECTOR);
        let spares = [
            snapshot_iatu(SPARE_SELECTOR),
            snapshot_iatu(SECOND_SPARE_SELECTOR),
        ];
        let selector_restore_ok = select_dbi_exact(saved_selector);
        let selector_restore = core::ptr::read_volatile(DBI_SELECTOR);
        let dummy_base = dummy_page_bounds().map(|v| v.0 as u32).unwrap_or(0);

        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).bar_assignment), bars);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.bar1_bus_base),
            bars.bar1_bus_base,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.dummy_local_base),
            dummy_base,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.original_bar1),
            original,
        );
        for index in 0..2 {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.spare_before[index]),
                spares[index],
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.spare_programmed[index]),
                spares[index],
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.spare_readback[index]),
                spares[index],
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.spare_restored[index]),
                spares[index],
            );
        }
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).selector_saved), saved_selector);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_restore,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_before), original.ctrl2);
        let target_before = read32(CHIP_ID);
        let control_before = read32(MONITOR2);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_before),
            target_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_before),
            control_before,
        );

        let mut flags = 0;
        flags |= u32::from(bar_assignment_valid(&bars)) * PROTECT_FLAG_DBI_BARS_VALID;
        flags |= u32::from(original_bar1_valid(&original)) * PROTECT_FLAG_ORIGINAL_BAR1_VALID;
        flags |= u32::from(
            spare_snapshot_unused(&spares[0], SPARE_SELECTOR)
                && spare_snapshot_unused(&spares[1], SECOND_SPARE_SELECTOR),
        ) * PROTECT_FLAG_SPARES_UNUSED;
        flags |= u32::from(dummy_base != 0 && dummy_page_valid(seq, dummy_base as usize))
            * PROTECT_FLAG_DUMMY_VALID;
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).protection.flags), flags);

        let required = PROTECT_FLAG_DBI_BARS_VALID
            | PROTECT_FLAG_ORIGINAL_BAR1_VALID
            | PROTECT_FLAG_SPARES_UNUSED
            | PROTECT_FLAG_DUMMY_VALID;
        if flags & required == required
            && selector_restore_ok
            && selector_restore == saved_selector
            && target_before == 0x2000_1927
            && control_before & 0x0019_0000 == 0x0019_0000
        {
            Some(ProtectionContext {
                saved_selector,
                bars,
                original,
                spares,
                dummy_base,
            })
        } else {
            None
        }
    }
}

unsafe fn block_bar1_once(m: *mut InboundMonitorMailbox, sample_start: u64) -> u32 {
    unsafe {
        let selector_saved = core::ptr::read_volatile(core::ptr::addr_of!((*m).selector_saved));
        let ctrl2_before = core::ptr::read_volatile(core::ptr::addr_of!((*m).ctrl2_before));
        let block_value = ctrl2_before & !(1 << 31);
        core::ptr::write_volatile(DBI_SELECTOR, BAR1_SELECTOR);
        dsb();
        core::ptr::write_volatile(CTRL2, block_value);
        dsb();
        let block_readback = core::ptr::read_volatile(CTRL2);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_value), block_value);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_block_readback),
            block_readback,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).block_disable_us),
            elapsed32(sample_start),
        );
        if block_readback == block_value {
            write_block_phase(m, BLOCK_PHASE_DISABLED);
        }

        let hold_start = raw_timer_us();
        while raw_timer_us().wrapping_sub(hold_start) < BLOCK_HOLD_US {
            spin_loop();
        }

        core::ptr::write_volatile(CTRL2, ctrl2_before);
        dsb();
        let restore_readback = core::ptr::read_volatile(CTRL2);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_restore_readback),
            restore_readback,
        );
        core::ptr::write_volatile(DBI_SELECTOR, selector_saved);
        dsb();
        let selector_restore = core::ptr::read_volatile(DBI_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_restore,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).block_restore_us),
            elapsed32(sample_start),
        );
        write_block_phase(m, BLOCK_PHASE_RESTORED);

        FLAG_CTRL2_WRITTEN
            | (u32::from(block_readback == block_value) * FLAG_CTRL2_BLOCK_READBACK_OK)
            | (u32::from(restore_readback == ctrl2_before) * FLAG_CTRL2_RESTORED)
            | (u32::from(selector_restore == selector_saved) * FLAG_SELECTOR_RESTORED)
    }
}

#[cfg(feature = "rp1-pcie-4k-protection-proof")]
unsafe fn finish_protection_timing(m: *mut InboundMonitorMailbox, start: u64) {
    let end = raw_timer_us();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ended_us_lo), end as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ended_us_hi),
            (end >> 32) as u32,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).elapsed_us), elapsed32(start));
    }
}

#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
unsafe fn run_second_spare_programming(m: *mut InboundMonitorMailbox) -> u32 {
    let start = raw_timer_us();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), start as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).started_us_hi),
            (start >> 32) as u32,
        );

        let saved_selector = core::ptr::read_volatile(DBI_SELECTOR);
        let bars_before = capture_bar_assignment(saved_selector);
        let bar1_before = snapshot_iatu(BAR1_SELECTOR);
        let bar2_before = snapshot_iatu(BAR2_SELECTOR);
        let a3_before = snapshot_iatu(SPARE_SELECTOR);
        let e3_before = snapshot_iatu(SECOND_SPARE_SELECTOR);
        let precondition_selector_ok = select_dbi_exact(saved_selector);
        let precondition_selector = core::ptr::read_volatile(DBI_SELECTOR);
        let chip_id_before = read32(CHIP_ID);

        // VERSION=4 reuses the fixed VERSION=3 proof slots without growing the
        // 1 KiB mailbox: original=23 before; spare_before=63/a3 before;
        // spare_programmed=e3 before/23 after; spare_readback=e3 programmed/63
        // after; spare_restored=e3 restored/a3 after.
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).bar_assignment), bars_before);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.original_bar1),
            bar1_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_before[0]),
            bar2_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_before[1]),
            a3_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[0]),
            e3_before,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).selector_saved), saved_selector);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_before), e3_before.ctrl2);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_before),
            chip_id_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_before),
            bars_before.command,
        );

        let precondition = bar_assignment_valid(&bars_before)
            && bar1_before.selector == BAR1_SELECTOR
            && bar2_before.selector == BAR2_SELECTOR
            && a3_before.selector == SPARE_SELECTOR
            && spare_snapshot_unused(&e3_before, SECOND_SPARE_SELECTOR)
            && precondition_selector_ok
            && precondition_selector == saved_selector
            && chip_id_before == 0x2000_1927;
        let mut flags = 0;
        let mut programmed_readback = e3_before;
        let mut restore_written = false;

        if precondition {
            flags |= SECOND_SPARE_FLAG_PRECONDITION;
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_OK);
            let (ctrl2_zero_before, ctrl2_zero_after) =
                program_disabled_second_spare(&SECOND_SPARE_PROGRAMMED);
            flags |= u32::from(ctrl2_zero_before) * SECOND_SPARE_FLAG_CTRL2_ZERO_BEFORE;
            flags |= u32::from(ctrl2_zero_after) * SECOND_SPARE_FLAG_CTRL2_ZERO_AFTER;
            programmed_readback = snapshot_iatu(SECOND_SPARE_SELECTOR);
            if ctrl2_zero_before
                && ctrl2_zero_after
                && snapshot_equal(&SECOND_SPARE_PROGRAMMED, &programmed_readback)
            {
                flags |= SECOND_SPARE_FLAG_PROGRAM_READBACK;
            }
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.enable_us),
                elapsed32(start),
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.target_during),
                read32(CHIP_ID),
            );
            write_block_phase(m, BLOCK_PHASE_RESTORING);

            let (restore_zero_before, restore_zero_after) =
                program_disabled_second_spare(&e3_before);
            restore_written = restore_zero_before && restore_zero_after;
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.restore_us),
                elapsed32(start),
            );
        } else {
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
        }

        let bar1_after = snapshot_iatu(BAR1_SELECTOR);
        let bar2_after = snapshot_iatu(BAR2_SELECTOR);
        let a3_after = snapshot_iatu(SPARE_SELECTOR);
        let e3_after = snapshot_iatu(SECOND_SPARE_SELECTOR);
        let bars_after = capture_bar_assignment(saved_selector);
        let selector_restore_ok = select_dbi_exact(saved_selector);
        let selector_after = core::ptr::read_volatile(DBI_SELECTOR);
        let chip_id_after = read32(CHIP_ID);

        let restored_e3 = e3_after;
        flags |= u32::from(restore_written && snapshot_equal(&e3_before, &restored_e3))
            * SECOND_SPARE_FLAG_RESTORED;
        flags |= u32::from(selector_restore_ok && selector_after == saved_selector)
            * SECOND_SPARE_FLAG_SELECTOR_RESTORED;
        flags |=
            u32::from(snapshot_equal(&bar1_before, &bar1_after)) * SECOND_SPARE_FLAG_BAR1_UNCHANGED;
        flags |=
            u32::from(snapshot_equal(&bar2_before, &bar2_after)) * SECOND_SPARE_FLAG_BAR2_UNCHANGED;
        flags |= u32::from(snapshot_equal(&a3_before, &a3_after)) * SECOND_SPARE_FLAG_A3_UNCHANGED;
        flags |= u32::from(bar_assignment_equal(&bars_before, &bars_after))
            * SECOND_SPARE_FLAG_ENDPOINT_UNCHANGED;
        flags |= u32::from(chip_id_after == chip_id_before) * SECOND_SPARE_FLAG_CHIP_ID_STABLE;

        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[1]),
            bar1_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[0]),
            programmed_readback,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[1]),
            bar2_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_restored[0]),
            restored_e3,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_restored[1]),
            a3_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_after,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_value), 0);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_block_readback),
            programmed_readback.ctrl2,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_restore_readback),
            restored_e3.ctrl2,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_after),
            chip_id_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_after),
            bars_after.command,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).protection.flags), flags);
        if flags & (SECOND_SPARE_FLAG_RESTORED | SECOND_SPARE_FLAG_SELECTOR_RESTORED)
            == SECOND_SPARE_FLAG_RESTORED | SECOND_SPARE_FLAG_SELECTOR_RESTORED
        {
            write_block_phase(m, BLOCK_PHASE_RESTORED);
        }
        finish_protection_timing(m, start);
        flags
    }
}

#[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
unsafe fn run_iatu_address_mask_characterization(m: *mut InboundMonitorMailbox) -> u32 {
    let start = raw_timer_us();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), start as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).started_us_hi),
            (start >> 32) as u32,
        );

        let saved_selector = core::ptr::read_volatile(DBI_SELECTOR);
        let bars_before = capture_bar_assignment(saved_selector);
        let bar1_before = snapshot_iatu(BAR1_SELECTOR);
        let bar2_before = snapshot_iatu(BAR2_SELECTOR);
        let a3_before = snapshot_iatu(SPARE_SELECTOR);
        let e3_before = snapshot_iatu(SECOND_SPARE_SELECTOR);
        let precondition_selector_ok = select_dbi_exact(saved_selector);
        let precondition_selector = core::ptr::read_volatile(DBI_SELECTOR);
        let chip_id_before = read32(CHIP_ID);

        // VERSION=5 keeps the fixed 1 KiB mailbox.  The nine snapshot slots are:
        // original=23 before; spare_before=63/a3 before;
        // spare_programmed=e3 before/anchor readback;
        // spare_readback=23/63 after; spare_restored=a3/e3 after.
        // Scalar reuse: target_page_offset/base_bus=BASE write/read;
        // dummy_local_base/target_before=LIMIT write/read;
        // target_during/target_after=TARGET write/read; enable/disable/restore_us
        // are phase timings; control_before/during/after are CHIP_ID snapshots.
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).bar_assignment), bars_before);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.original_bar1),
            bar1_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_before[0]),
            bar2_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_before[1]),
            a3_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[0]),
            e3_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_page_offset),
            ADDRESS_MASK_BASE_CHALLENGE,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.bar1_bus_base),
            u32::MAX,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.dummy_local_base),
            ADDRESS_MASK_LIMIT_CHALLENGE,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_before),
            u32::MAX,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_during),
            ADDRESS_MASK_TARGET_CHALLENGE,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_after),
            u32::MAX,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_before),
            chip_id_before,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).selector_saved), saved_selector);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_before), e3_before.ctrl2);

        let precondition = bar_assignment_valid(&bars_before)
            && bar1_before.selector == BAR1_SELECTOR
            && bar2_before.selector == BAR2_SELECTOR
            && a3_before.selector == SPARE_SELECTOR
            && spare_snapshot_unused(&e3_before, SECOND_SPARE_SELECTOR)
            && precondition_selector_ok
            && precondition_selector == saved_selector
            && chip_id_before == 0x2000_1927;
        let mut flags = 0;
        let mut anchor_readback = e3_before;
        let mut ctrl2_safe_all = false;
        let mut base_anchor_restored = false;
        let mut limit_anchor_restored = false;
        let mut restore_written = false;

        if precondition {
            flags |= ADDRESS_MASK_FLAG_PRECONDITION;
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_OK);

            let (anchor_zero_before, anchor_zero_after) =
                program_disabled_second_spare(&SECOND_SPARE_PROGRAMMED);
            anchor_readback = snapshot_iatu(SECOND_SPARE_SELECTOR);
            let anchor_exact = anchor_zero_before
                && anchor_zero_after
                && snapshot_equal(&SECOND_SPARE_PROGRAMMED, &anchor_readback);
            ctrl2_safe_all = anchor_zero_before && anchor_zero_after;
            let mut proceed = anchor_exact;
            flags |= u32::from(anchor_exact) * ADDRESS_MASK_FLAG_ANCHOR_EXACT;
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.enable_us),
                elapsed32(start),
            );

            if proceed {
                match challenge_disabled_second_spare(IATU_LOWER_BASE, ADDRESS_MASK_BASE_CHALLENGE)
                {
                    Some(readback) => {
                        core::ptr::write_volatile(
                            core::ptr::addr_of_mut!((*m).protection.bar1_bus_base),
                            readback.lower_base,
                        );
                        let mut normalized = readback;
                        normalized.lower_base = SECOND_SPARE_PROGRAMMED.lower_base;
                        let invariants_exact =
                            snapshot_equal(&SECOND_SPARE_PROGRAMMED, &normalized);
                        flags |= u32::from(invariants_exact) * ADDRESS_MASK_FLAG_BASE_EXECUTED;
                        flags |= u32::from(
                            invariants_exact && readback.lower_base == ADDRESS_MASK_BASE_EXPECTED,
                        ) * ADDRESS_MASK_FLAG_BASE_EXPECTED;
                        proceed = invariants_exact;
                    }
                    None => {
                        ctrl2_safe_all = false;
                        proceed = false;
                    }
                }
                base_anchor_restored = restore_second_spare_anchor();
                ctrl2_safe_all &= base_anchor_restored;
                proceed &= base_anchor_restored;
            }

            if proceed {
                match challenge_disabled_second_spare(IATU_LIMIT, ADDRESS_MASK_LIMIT_CHALLENGE) {
                    Some(readback) => {
                        core::ptr::write_volatile(
                            core::ptr::addr_of_mut!((*m).protection.target_before),
                            readback.limit,
                        );
                        let mut normalized = readback;
                        normalized.limit = SECOND_SPARE_PROGRAMMED.limit;
                        let invariants_exact =
                            snapshot_equal(&SECOND_SPARE_PROGRAMMED, &normalized);
                        flags |= u32::from(invariants_exact) * ADDRESS_MASK_FLAG_LIMIT_EXECUTED;
                        flags |= u32::from(
                            invariants_exact && readback.limit == ADDRESS_MASK_LIMIT_EXPECTED,
                        ) * ADDRESS_MASK_FLAG_LIMIT_EXPECTED;
                        proceed = invariants_exact;
                    }
                    None => {
                        ctrl2_safe_all = false;
                        proceed = false;
                    }
                }
                limit_anchor_restored = restore_second_spare_anchor();
                ctrl2_safe_all &= limit_anchor_restored;
                proceed &= limit_anchor_restored;
            }

            if proceed {
                match challenge_disabled_second_spare(
                    IATU_LOWER_TARGET,
                    ADDRESS_MASK_TARGET_CHALLENGE,
                ) {
                    Some(readback) => {
                        core::ptr::write_volatile(
                            core::ptr::addr_of_mut!((*m).protection.target_after),
                            readback.lower_target,
                        );
                        let mut normalized = readback;
                        normalized.lower_target = SECOND_SPARE_PROGRAMMED.lower_target;
                        let invariants_exact =
                            snapshot_equal(&SECOND_SPARE_PROGRAMMED, &normalized);
                        flags |= u32::from(invariants_exact) * ADDRESS_MASK_FLAG_TARGET_EXECUTED;
                        flags |= u32::from(
                            invariants_exact
                                && readback.lower_target == ADDRESS_MASK_TARGET_EXPECTED,
                        ) * ADDRESS_MASK_FLAG_TARGET_EXPECTED;
                    }
                    None => ctrl2_safe_all = false,
                }
            }

            flags |= u32::from(base_anchor_restored && limit_anchor_restored)
                * ADDRESS_MASK_FLAG_ANCHOR_RESTORED_BETWEEN;
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.disable_us),
                elapsed32(start),
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.control_during),
                read32(CHIP_ID),
            );
            write_block_phase(m, BLOCK_PHASE_RESTORING);

            let (restore_zero_before, restore_zero_after) =
                program_disabled_second_spare(&e3_before);
            restore_written = restore_zero_before && restore_zero_after;
            ctrl2_safe_all &= restore_written;
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.restore_us),
                elapsed32(start),
            );
        } else {
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
        }

        let chip_id_during =
            core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.control_during));
        let bar1_after = snapshot_iatu(BAR1_SELECTOR);
        let bar2_after = snapshot_iatu(BAR2_SELECTOR);
        let a3_after = snapshot_iatu(SPARE_SELECTOR);
        let e3_after = snapshot_iatu(SECOND_SPARE_SELECTOR);
        let bars_after = capture_bar_assignment(saved_selector);
        let selector_restore_ok = select_dbi_exact(saved_selector);
        let selector_after = core::ptr::read_volatile(DBI_SELECTOR);
        let chip_id_after = read32(CHIP_ID);

        flags |= u32::from(ctrl2_safe_all) * ADDRESS_MASK_FLAG_CTRL2_SAFE_ALL;
        flags |= u32::from(restore_written && snapshot_equal(&e3_before, &e3_after))
            * ADDRESS_MASK_FLAG_E3_RESTORED;
        flags |= u32::from(
            snapshot_equal(&bar1_before, &bar1_after)
                && snapshot_equal(&bar2_before, &bar2_after)
                && snapshot_equal(&a3_before, &a3_after),
        ) * ADDRESS_MASK_FLAG_ACTIVE_REGIONS_UNCHANGED;
        flags |= u32::from(bar_assignment_equal(&bars_before, &bars_after))
            * ADDRESS_MASK_FLAG_ENDPOINT_UNCHANGED;
        flags |= u32::from(selector_restore_ok && selector_after == saved_selector)
            * ADDRESS_MASK_FLAG_SELECTOR_RESTORED;
        flags |= u32::from(chip_id_during == chip_id_before && chip_id_after == chip_id_before)
            * ADDRESS_MASK_FLAG_CHIP_STABLE;

        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[1]),
            anchor_readback,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[0]),
            bar1_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[1]),
            bar2_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_restored[0]),
            a3_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_restored[1]),
            e3_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_after,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_value), 0);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_block_readback),
            anchor_readback.ctrl2,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_restore_readback),
            e3_after.ctrl2,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_after),
            chip_id_after,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).protection.flags), flags);
        if flags & (ADDRESS_MASK_FLAG_E3_RESTORED | ADDRESS_MASK_FLAG_SELECTOR_RESTORED)
            == ADDRESS_MASK_FLAG_E3_RESTORED | ADDRESS_MASK_FLAG_SELECTOR_RESTORED
        {
            write_block_phase(m, BLOCK_PHASE_RESTORED);
        }
        finish_protection_timing(m, start);
        flags
    }
}

#[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
unsafe fn run_interior_64k_hole(m: *mut InboundMonitorMailbox, seq: u32) -> u32 {
    let start = raw_timer_us();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), start as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).started_us_hi),
            (start >> 32) as u32,
        );

        let Some(context) = protection_precondition(m, seq) else {
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
            finish_protection_timing(m, start);
            return 0;
        };
        let bar2_before = snapshot_iatu(BAR2_SELECTOR);
        let selector_precondition_ok = select_dbi_exact(context.saved_selector);
        let selector_precondition = core::ptr::read_volatile(DBI_SELECTOR);
        let chip_id_before = read32(CHIP_ID);
        let monitor2_before = read32(MONITOR2);

        // VERSION=6 preserves the fixed VERSION=3 proof layout.  Snapshot slots
        // are original=23 before; spare_before=A3/E3 before;
        // spare_programmed=A3-upper/E3-lower expected;
        // spare_readback=A3/E3 enabled readback; spare_restored=A3/E3 final.
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[0]),
            INTERIOR_A3_UPPER,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[1]),
            INTERIOR_E3_LOWER,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_page_offset),
            INTERIOR_HOLE_OFFSET,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.bar1_bus_base),
            context.bars.bar1_bus_base,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.dummy_local_base),
            INTERIOR_PID0_LOCAL,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_before),
            chip_id_before,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_before),
            monitor2_before,
        );

        let precondition = context.bars.bar1_bus_base == 0
            && interior_original_bar1_exact(&context.original)
            && bar2_before.selector == BAR2_SELECTOR
            && bar2_before.ctrl2 & IATU_ENABLE != 0
            && selector_precondition_ok
            && selector_precondition == context.saved_selector
            && chip_id_before == 0x2000_1927;
        if !precondition {
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
            finish_protection_timing(m, start);
            return 0;
        }

        let mut flags = INTERIOR_FLAG_PRECONDITION;
        let mut original_write_attempted = false;
        write_block_phase(m, BLOCK_PHASE_PRECONDITION_OK);

        let a3_programmed = program_iatu(&INTERIOR_A3_UPPER);
        let a3_readback = snapshot_iatu(SPARE_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[0]),
            a3_readback,
        );
        let a3_exact = a3_programmed && snapshot_equal(&INTERIOR_A3_UPPER, &a3_readback);
        flags |= u32::from(a3_exact) * INTERIOR_FLAG_A3_UPPER_EXACT;

        let mut e3_readback = context.spares[1];
        let e3_exact = if a3_exact {
            let e3_programmed = program_iatu(&INTERIOR_E3_LOWER);
            e3_readback = snapshot_iatu(SECOND_SPARE_SELECTOR);
            e3_programmed && snapshot_equal(&INTERIOR_E3_LOWER, &e3_readback)
        } else {
            false
        };
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[1]),
            e3_readback,
        );
        flags |= u32::from(e3_exact) * INTERIOR_FLAG_E3_LOWER_EXACT;

        let both_ready = a3_exact && e3_exact;
        flags |= u32::from(both_ready) * INTERIOR_FLAG_BOTH_READY;
        if both_ready {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.enable_us),
                elapsed32(start),
            );

            let disabled = context.original.ctrl2 & !IATU_ENABLE;
            let original_write_ready = interior_original_bar1_exact(&snapshot_iatu(BAR1_SELECTOR));
            let selected = original_write_ready && select_dbi_exact(BAR1_SELECTOR);
            if selected {
                original_write_attempted = true;
                write32(IATU_CTRL2, disabled);
            }
            let disabled_readback = if selected {
                read32(IATU_CTRL2)
            } else {
                u32::MAX
            };
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_value), disabled);
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).ctrl2_block_readback),
                disabled_readback,
            );
            let original_disabled = selected && disabled_readback == disabled;
            flags |= u32::from(original_disabled) * INTERIOR_FLAG_ORIGINAL_DISABLED;

            if original_disabled {
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*m).block_disable_us),
                    elapsed32(start),
                );
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*m).protection.disable_us),
                    elapsed32(start),
                );
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).protection.flags), flags);
                write_block_phase(m, BLOCK_PHASE_DISABLED);
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*m).protection.target_during),
                    read32(CHIP_ID),
                );
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!((*m).protection.control_during),
                    read32(MONITOR2),
                );
                let hold_start = raw_timer_us();
                while raw_timer_us().wrapping_sub(hold_start) < BLOCK_HOLD_US {
                    spin_loop();
                }
                flags |= INTERIOR_FLAG_DURING_REACHED;
            }
        }

        write_block_phase(m, BLOCK_PHASE_RESTORING);

        // The original mapping is restored before either replacement is
        // disabled.  If it was never written, leave it untouched.
        if original_write_attempted && select_dbi_exact(BAR1_SELECTOR) {
            write32(IATU_CTRL2, context.original.ctrl2);
        }
        let original_restore_readback = if select_dbi_exact(BAR1_SELECTOR) {
            read32(IATU_CTRL2)
        } else {
            u32::MAX
        };
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ctrl2_restore_readback),
            original_restore_readback,
        );
        let original_restored_first = original_restore_readback == context.original.ctrl2
            && snapshot_equal(&context.original, &snapshot_iatu(BAR1_SELECTOR));
        flags |= u32::from(original_restored_first) * INTERIOR_FLAG_ORIGINAL_RESTORED;

        let e3_restore_programmed = program_iatu(&context.spares[1]);
        let a3_restore_programmed = program_iatu(&context.spares[0]);

        let original_after = snapshot_iatu(BAR1_SELECTOR);
        let bar2_after = snapshot_iatu(BAR2_SELECTOR);
        let a3_after = snapshot_iatu(SPARE_SELECTOR);
        let e3_after = snapshot_iatu(SECOND_SPARE_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_restored[0]),
            a3_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_restored[1]),
            e3_after,
        );

        let e3_restored = e3_restore_programmed && snapshot_equal(&context.spares[1], &e3_after);
        let a3_restored = a3_restore_programmed && snapshot_equal(&context.spares[0], &a3_after);
        flags |= u32::from(e3_restored) * INTERIOR_FLAG_E3_RESTORED;
        flags |= u32::from(a3_restored) * INTERIOR_FLAG_A3_RESTORED;

        let bars_after = capture_bar_assignment(context.saved_selector);
        let selector_restore_ok = select_dbi_exact(context.saved_selector);
        let selector_after = core::ptr::read_volatile(DBI_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_after,
        );
        let selector_restored = selector_restore_ok && selector_after == context.saved_selector;
        let bar2_unchanged = snapshot_equal(&bar2_before, &bar2_after);
        let endpoint_unchanged = bar_assignment_equal(&context.bars, &bars_after);
        let final_snapshots_exact = snapshot_equal(&context.original, &original_after)
            && snapshot_equal(&context.spares[0], &a3_after)
            && snapshot_equal(&context.spares[1], &e3_after);
        flags |= u32::from(selector_restored) * INTERIOR_FLAG_SELECTOR_RESTORED;
        flags |= u32::from(bar2_unchanged) * INTERIOR_FLAG_BAR2_UNCHANGED;
        flags |= u32::from(endpoint_unchanged) * INTERIOR_FLAG_ENDPOINT_UNCHANGED;
        flags |= u32::from(final_snapshots_exact) * INTERIOR_FLAG_FINAL_SNAPSHOTS_EXACT;

        let chip_id_during =
            core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.target_during));
        let chip_id_after = read32(CHIP_ID);
        let monitor2_after = read32(MONITOR2);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_after),
            chip_id_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_after),
            monitor2_after,
        );
        let chip_id_stable = chip_id_before == 0x2000_1927
            && chip_id_during == chip_id_before
            && chip_id_after == chip_id_before;
        flags |= u32::from(chip_id_stable) * INTERIOR_FLAG_CHIP_ID_STABLE;

        let cleanup_complete = original_restored_first
            && e3_restored
            && a3_restored
            && selector_restored
            && bar2_unchanged
            && endpoint_unchanged
            && final_snapshots_exact;
        flags |= u32::from(cleanup_complete) * INTERIOR_FLAG_CLEANUP_COMPLETE;

        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.restore_us),
            elapsed32(start),
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).block_restore_us),
            elapsed32(start),
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).protection.flags), flags);
        finish_protection_timing(m, start);
        if flags & INTERIOR_REQUIRED_FLAGS == INTERIOR_REQUIRED_FLAGS {
            write_block_phase(m, BLOCK_PHASE_RESTORED);
        } else {
            write_block_phase(m, BLOCK_PHASE_REJECTED);
        }
        flags
    }
}

#[cfg(all(
    feature = "rp1-pcie-4k-protection-proof",
    not(feature = "rp1-iatu-second-spare-programming-proof")
))]
unsafe fn run_protection(m: *mut InboundMonitorMailbox, seq: u32, mode: u32) -> bool {
    let start = raw_timer_us();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), start as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).started_us_hi),
            (start >> 32) as u32,
        );

        let Some(context) = protection_precondition(m, seq) else {
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
            finish_protection_timing(m, start);
            return false;
        };
        write_block_phase(m, BLOCK_PHASE_PRECONDITION_OK);

        #[cfg(feature = "rp1-pcie-64k-hole-proof")]
        let hole_64k = mode == MODE_HOLE_64K;
        #[cfg(not(feature = "rp1-pcie-64k-hole-proof"))]
        let hole_64k = false;
        let replacement = mode == MODE_HOLE_4K || hole_64k;
        let hole_offset = if hole_64k { 0x1_0000 } else { 0x1000 };
        let expected = if replacement {
            IatuRegionSnapshot {
                selector: SPARE_SELECTOR,
                ctrl1: 0,
                ctrl2: IATU_ENABLE,
                lower_base: context.bars.bar1_bus_base.wrapping_add(hole_offset),
                upper_base: 0,
                limit: context.bars.bar1_bus_base.wrapping_add(0x003f_ffff),
                lower_target: 0x4000_0000u32.wrapping_add(hole_offset),
                upper_target: 0x0000_00c0,
                upper_limit: 0,
            }
        } else {
            IatuRegionSnapshot {
                selector: SPARE_SELECTOR,
                ctrl1: 0,
                ctrl2: IATU_ENABLE,
                lower_base: context.bars.bar1_bus_base,
                upper_base: 0,
                limit: context.bars.bar1_bus_base.wrapping_add(0x0fff),
                lower_target: context.dummy_base,
                upper_target: 0x0000_00c0,
                upper_limit: 0,
            }
        };
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_programmed[0]),
            expected,
        );
        let programmed = program_iatu(&expected);
        let programmed_readback = snapshot_iatu(SPARE_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.spare_readback[0]),
            programmed_readback,
        );

        let mut flags = core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.flags));
        let mut active_ok = programmed && snapshot_equal(&expected, &programmed_readback);
        if active_ok {
            flags |= PROTECT_FLAG_PROGRAM_READBACK;
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.enable_us),
                elapsed32(start),
            );
        }

        if replacement && active_ok {
            let disabled = context.original.ctrl2 & !IATU_ENABLE;
            let selected = select_dbi_exact(BAR1_SELECTOR);
            if selected {
                write32(IATU_CTRL2, disabled);
            }
            let disabled_readback = if selected {
                read32(IATU_CTRL2)
            } else {
                u32::MAX
            };
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ctrl2_block_value), disabled);
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).ctrl2_block_readback),
                disabled_readback,
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).block_disable_us),
                elapsed32(start),
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.disable_us),
                elapsed32(start),
            );
            active_ok = selected && disabled_readback == disabled;
        }

        if active_ok {
            flags |= PROTECT_FLAG_ACTIVE;
            write_block_phase(m, BLOCK_PHASE_DISABLED);
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.target_during),
                read32(CHIP_ID),
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.control_during),
                read32(MONITOR2),
            );
            let hold_start = raw_timer_us();
            while raw_timer_us().wrapping_sub(hold_start) < BLOCK_HOLD_US {
                spin_loop();
            }
            write_block_phase(m, BLOCK_PHASE_RESTORING);
        }

        if replacement {
            let selected = select_dbi_exact(BAR1_SELECTOR);
            if selected {
                write32(IATU_CTRL2, context.original.ctrl2);
            }
            let original_restore = if selected {
                read32(IATU_CTRL2)
            } else {
                u32::MAX
            };
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).ctrl2_restore_readback),
                original_restore,
            );
            if original_restore == context.original.ctrl2 {
                flags |= PROTECT_FLAG_ORIGINAL_RESTORED;
            }
        }

        let spare_selected = select_dbi_exact(SPARE_SELECTOR);
        if spare_selected {
            write32(IATU_CTRL2, context.spares[0].ctrl2 & !IATU_ENABLE);
        }
        if spare_selected && read32(IATU_CTRL2) == (context.spares[0].ctrl2 & !IATU_ENABLE) {
            flags |= PROTECT_FLAG_SPARE_DISABLED;
        }
        if !replacement {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.disable_us),
                elapsed32(start),
            );
        }
        let spare_restore_programmed = program_iatu(&context.spares[0]);

        let original_after = snapshot_iatu(BAR1_SELECTOR);
        let spares_after = [
            snapshot_iatu(SPARE_SELECTOR),
            snapshot_iatu(SECOND_SPARE_SELECTOR),
        ];
        for (index, value) in spares_after.into_iter().enumerate() {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).protection.spare_restored[index]),
                value,
            );
        }
        if spare_restore_programmed
            && snapshot_equal(&context.spares[0], &spares_after[0])
            && snapshot_equal(&context.spares[1], &spares_after[1])
        {
            flags |= PROTECT_FLAG_SPARE_RESTORED;
        }
        if snapshot_equal(&context.original, &original_after) {
            flags |= PROTECT_FLAG_ORIGINAL_RESTORED;
        }

        let selector_restore_ok = select_dbi_exact(context.saved_selector);
        let selector_after = core::ptr::read_volatile(DBI_SELECTOR);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).selector_restore_readback),
            selector_after,
        );
        if selector_restore_ok && selector_after == context.saved_selector {
            flags |= PROTECT_FLAG_SELECTOR_RESTORED;
        }

        let target_after = read32(CHIP_ID);
        let control_after = read32(MONITOR2);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.target_after),
            target_after,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.control_after),
            control_after,
        );
        let target_before =
            core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.target_before));
        let target_during =
            core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.target_during));
        let control_before =
            core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.control_before));
        let control_during =
            core::ptr::read_volatile(core::ptr::addr_of!((*m).protection.control_during));
        if target_before == 0x2000_1927
            && target_during == 0x2000_1927
            && target_after == 0x2000_1927
        {
            flags |= PROTECT_FLAG_LOCAL_TARGET_STABLE;
        }
        if control_before & 0x0019_0000 == 0x0019_0000
            && control_during & 0x0019_0000 == 0x0019_0000
            && control_after & 0x0019_0000 == 0x0019_0000
        {
            flags |= PROTECT_FLAG_LOCAL_CONTROL_STABLE;
        }

        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).protection.restore_us),
            elapsed32(start),
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).block_restore_us),
            elapsed32(start),
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).protection.flags), flags);
        finish_protection_timing(m, start);

        let required = PROTECT_FLAG_DBI_BARS_VALID
            | PROTECT_FLAG_ORIGINAL_BAR1_VALID
            | PROTECT_FLAG_SPARES_UNUSED
            | PROTECT_FLAG_DUMMY_VALID
            | PROTECT_FLAG_PROGRAM_READBACK
            | PROTECT_FLAG_ACTIVE
            | PROTECT_FLAG_SPARE_DISABLED
            | PROTECT_FLAG_SPARE_RESTORED
            | PROTECT_FLAG_SELECTOR_RESTORED
            | PROTECT_FLAG_LOCAL_TARGET_STABLE
            | PROTECT_FLAG_LOCAL_CONTROL_STABLE
            | PROTECT_FLAG_ORIGINAL_RESTORED;
        let success = active_ok && flags & required == required;
        if success {
            write_block_phase(m, BLOCK_PHASE_RESTORED);
        } else {
            write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
        }
        success
    }
}

#[inline(always)]
fn count_bit(value: u32, bit: u32, count: &mut u32, first_us: &mut u32, elapsed: u32) -> bool {
    if value & (1 << bit) == 0 {
        return false;
    }
    let (next, overflow) = count.overflowing_add(1);
    *count = next;
    if *first_us == 0 {
        *first_us = elapsed;
    }
    overflow
}

unsafe fn sample_100ms(
    m: *mut InboundMonitorMailbox,
    mode: u32,
    focused_channel: Option<usize>,
) -> u32 {
    unsafe {
        let start = raw_timer_us();
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), start as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).started_us_hi),
            (start >> 32) as u32,
        );
        read_config_before(m);

        let scratch_initial = capture_scratch_initial(m);
        let mut scratch_last = scratch_initial;
        let mut scratch_change_count = 0u32;
        let mut scratch_last_change_us = 0u32;
        let mut scratch_restore_ok = 0u32;
        let mut overflow_count = 0u32;
        let mut flags = 0u32;
        let mut block_attempted = false;
        let mut block_allowed = false;
        if mode == MODE_BLOCK_BAR1 {
            block_allowed = capture_bar1_ctrl2_precondition(m);
            if block_allowed {
                flags |= FLAG_CTRL2_PRECONDITION_OK;
                write_block_phase(m, BLOCK_PHASE_PRECONDITION_OK);
            } else {
                write_block_phase(m, BLOCK_PHASE_PRECONDITION_FAIL);
            }
        }

        let mut monitor_or = [0u32; 3];
        let mut monitor_max = [0u32; 3];
        let mut monitor2_bit23_count = 0u32;
        let mut monitor2_bit22_count = 0u32;
        let mut monitor2_bit21_count = 0u32;
        let mut monitor2_bit23_first_us = 0u32;
        let mut monitor2_bit22_first_us = 0u32;
        let mut monitor2_bit21_first_us = 0u32;
        let mut status = [RegisterStats {
            or_value: 0,
            max_value: 0,
            count: 0,
            first_us: 0,
        }; 12];
        let mut samples = 0u32;

        loop {
            let elapsed = raw_timer_us().wrapping_sub(start);
            let elapsed_u32 = core::cmp::min(elapsed, u64::from(u32::MAX)) as u32;
            if mode == MODE_BLOCK_BAR1
                && block_allowed
                && !block_attempted
                && elapsed >= BLOCK_AT_US
            {
                block_attempted = true;
                flags |= block_bar1_once(m, start);
            }

            for (index, address) in PCIE_MONITOR.into_iter().enumerate() {
                let value = read32(address);
                monitor_or[index] |= value;
                if value > monitor_max[index] {
                    monitor_max[index] = value;
                }
                if index == 2 {
                    overflow_count += u32::from(count_bit(
                        value,
                        23,
                        &mut monitor2_bit23_count,
                        &mut monitor2_bit23_first_us,
                        elapsed_u32,
                    ));
                    overflow_count += u32::from(count_bit(
                        value,
                        22,
                        &mut monitor2_bit22_count,
                        &mut monitor2_bit22_first_us,
                        elapsed_u32,
                    ));
                    overflow_count += u32::from(count_bit(
                        value,
                        21,
                        &mut monitor2_bit21_count,
                        &mut monitor2_bit21_first_us,
                        elapsed_u32,
                    ));
                }
            }

            let channels = focused_channel.map_or(0..AXISHIM.len(), |index| index..index + 1);
            for index in channels {
                let status_addr = AXISHIM[index].1;
                let value = read32(status_addr);
                let low16 = value & 0xffff;
                let stats = &mut status[index];
                stats.or_value |= value;
                if low16 > stats.max_value {
                    stats.max_value = low16;
                }
                if low16 != 0 {
                    let (next, overflow) = stats.count.overflowing_add(1);
                    stats.count = next;
                    overflow_count += u32::from(overflow);
                    if stats.first_us == 0 {
                        stats.first_us = elapsed_u32;
                    }
                }
            }

            for (index, last) in scratch_last.iter_mut().enumerate() {
                let value = core::ptr::read_volatile(core::ptr::addr_of!((*m).scratch[index]));
                if value != *last {
                    let (next, overflow) = scratch_change_count.overflowing_add(1);
                    scratch_change_count = next;
                    overflow_count += u32::from(overflow);
                    scratch_last_change_us = elapsed_u32;
                    *last = value;
                }
            }

            let (next_samples, overflow) = samples.overflowing_add(1);
            samples = next_samples;
            overflow_count += u32::from(overflow);
            if elapsed >= SAMPLE_US {
                break;
            }
            spin_loop();
        }

        let end = raw_timer_us();
        read_config_after(m);
        let config_change_count = count_axishim_config_changes(m);
        for (index, value) in scratch_last.into_iter().enumerate() {
            let final_value = core::ptr::read_volatile(core::ptr::addr_of!((*m).scratch[index]));
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).scratch_last[index]), value);
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).scratch_final[index]),
                final_value,
            );
            scratch_restore_ok |= u32::from(final_value == scratch_initial[index]) << index;
        }
        if scratch_restore_ok == 0xf {
            flags |= FLAG_SCRATCH_RESTORED;
        }

        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ended_us_lo), end as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ended_us_hi),
            (end >> 32) as u32,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).elapsed_us), elapsed32(start));
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).sample_count), samples);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).overflow_count), overflow_count);
        let health_flags = (u32::from(samples != 0) * HEALTH_PCIE_MONITOR_CAPTURED)
            | (u32::from(config_change_count == 0) * HEALTH_AXISHIM_CFG_UNCHANGED)
            | (u32::from(samples != 0) * HEALTH_SAMPLED)
            | (u32::from(overflow_count == 0) * HEALTH_NO_OVERFLOW)
            | (u32::from(scratch_restore_ok == 0xf) * HEALTH_SCRATCH_RESTORED);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).health_flags), health_flags);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).config_change_count),
            config_change_count,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor0_or), monitor_or[0]);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor0_max), monitor_max[0]);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor1_or), monitor_or[1]);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor1_max), monitor_max[1]);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_or), monitor_or[2]);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).monitor2_max), monitor_max[2]);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).monitor2_bit23_count),
            monitor2_bit23_count,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).monitor2_bit22_count),
            monitor2_bit22_count,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).monitor2_bit21_count),
            monitor2_bit21_count,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).monitor2_bit23_first_us),
            monitor2_bit23_first_us,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).monitor2_bit22_first_us),
            monitor2_bit22_first_us,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).monitor2_bit21_first_us),
            monitor2_bit21_first_us,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).scratch_restore_ok),
            scratch_restore_ok,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).scratch_change_count),
            scratch_change_count,
        );
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).scratch_last_change_us),
            scratch_last_change_us,
        );
        for (index, stats) in status.into_iter().enumerate() {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).axishim_status[index].or_value),
                stats.or_value,
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).axishim_status[index].max_value),
                stats.max_value,
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).axishim_status[index].count),
                stats.count,
            );
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!((*m).axishim_status[index].first_us),
                stats.first_us,
            );
        }
        flags
    }
}

unsafe fn checksum_mailbox(m: *mut InboundMonitorMailbox) -> u32 {
    let words = m.cast::<u32>();
    let mut checksum = CHECKSUM_SEED;
    for index in 0..CHECKSUM_WORDS {
        let mut value = unsafe { core::ptr::read_volatile(words.add(index)) };
        if index == CHECKSUM_EXCLUDED_COMPLETION_SEQ_WORD
            || index == CHECKSUM_EXCLUDED_CHECKSUM_WORD
            || index == CHECKSUM_EXCLUDED_ARG0_WORD
            || index == CHECKSUM_EXCLUDED_ARG1_WORD
        {
            value = 0;
        }
        checksum = (checksum ^ value).rotate_left(5).wrapping_mul(CHECKSUM_MUL);
    }
    checksum
}

unsafe fn stamp_instant_result(m: *mut InboundMonitorMailbox) {
    let now = raw_timer_us();
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).started_us_lo), now as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).started_us_hi),
            (now >> 32) as u32,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ended_us_lo), now as u32);
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!((*m).ended_us_hi),
            (now >> 32) as u32,
        );
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).elapsed_us), 0);
    }
}

unsafe fn publish_completion_with_result(
    m: *mut InboundMonitorMailbox,
    seq: u32,
    completion: u32,
    result: u32,
) {
    unsafe {
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).completion), completion);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).result), result);
        write_phase(m, PHASE_DONE);
        let checksum = checksum_mailbox(m);
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).checksum), checksum);
        dsb();
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).completion_seq), seq);
        dsb();
    }
}

unsafe fn publish_completion(m: *mut InboundMonitorMailbox, seq: u32, completion: u32) {
    unsafe { publish_completion_with_result(m, seq, completion, completion) }
}

#[cfg(not(feature = "rp1-iatu-second-spare-programming-proof"))]
unsafe fn run_mode(m: *mut InboundMonitorMailbox, seq: u32, mode: u32) -> bool {
    unsafe {
        if mode == MODE_DONE {
            stamp_instant_result(m);
            publish_completion(m, seq, COMPLETION_DONE);
            return true;
        }
        #[cfg(feature = "rp1-bar1-interior-64k-hole-proof")]
        if mode == MODE_INTERIOR_HOLE_64K {
            write_phase(m, PHASE_SAMPLING);
            let flags = run_interior_64k_hole(m, seq);
            let success = flags & INTERIOR_REQUIRED_FLAGS == INTERIOR_REQUIRED_FLAGS;
            let completion = if success {
                COMPLETION_DONE
            } else if flags & INTERIOR_FLAG_PRECONDITION != 0 {
                COMPLETION_REJECTED
            } else {
                COMPLETION_PRECONDITION
            };
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).flags), flags);
            publish_completion_with_result(m, seq, completion, u32::from(success));
            return false;
        }
        let mut flags = 0u32;
        let completion = match mode {
            MODE_IDLE | MODE_BAR2_READ | MODE_BAR2_WRITE | MODE_BAR1_READ | MODE_BLOCK_BAR1 => {
                write_phase(m, PHASE_SAMPLING);
                #[cfg(feature = "rp1-axishim-focused-sampling-proof")]
                let focused_channel = {
                    let index = core::ptr::read_volatile(core::ptr::addr_of!((*m).arg0)) as usize;
                    if index >= AXISHIM.len() {
                        stamp_instant_result(m);
                        core::ptr::write_volatile(
                            core::ptr::addr_of_mut!((*m).flags),
                            COMPLETION_REJECTED,
                        );
                        publish_completion(m, seq, COMPLETION_REJECTED);
                        return false;
                    }
                    Some(index)
                };
                #[cfg(not(feature = "rp1-axishim-focused-sampling-proof"))]
                let focused_channel = None;
                flags = sample_100ms(m, mode, focused_channel);
                if mode == MODE_IDLE {
                    COMPLETION_IDLE
                } else if mode == MODE_BLOCK_BAR1 && flags & FLAG_CTRL2_PRECONDITION_OK == 0 {
                    COMPLETION_PRECONDITION
                } else {
                    COMPLETION_DONE
                }
            }
            #[cfg(feature = "rp1-pcie-4k-protection-proof")]
            MODE_REDIRECT_4K | MODE_HOLE_4K => {
                write_phase(m, PHASE_SAMPLING);
                if run_protection(m, seq, mode) {
                    COMPLETION_DONE
                } else {
                    COMPLETION_PRECONDITION
                }
            }
            #[cfg(feature = "rp1-pcie-64k-hole-proof")]
            MODE_HOLE_64K => {
                write_phase(m, PHASE_SAMPLING);
                if run_protection(m, seq, mode) {
                    COMPLETION_DONE
                } else {
                    COMPLETION_PRECONDITION
                }
            }
            _ => {
                stamp_instant_result(m);
                COMPLETION_REJECTED
            }
        };
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).flags), flags);
        publish_completion(m, seq, completion);
        false
    }
}

#[cfg(feature = "rp1-iatu-second-spare-programming-proof")]
unsafe fn run_mode(m: *mut InboundMonitorMailbox, seq: u32, mode: u32) -> bool {
    unsafe {
        if mode == MODE_DONE {
            stamp_instant_result(m);
            publish_completion(m, seq, COMPLETION_DONE);
            return true;
        }
        #[cfg(feature = "rp1-iatu-64k-address-mask-characterization")]
        if mode == MODE_CHARACTERIZE_ADDRESS_MASK {
            write_phase(m, PHASE_SAMPLING);
            let flags = run_iatu_address_mask_characterization(m);
            let safe_complete = flags & ADDRESS_MASK_REQUIRED_FLAGS == ADDRESS_MASK_REQUIRED_FLAGS;
            let completion = if safe_complete {
                COMPLETION_DONE
            } else {
                COMPLETION_PRECONDITION
            };
            let result = if !safe_complete {
                0
            } else if flags & ADDRESS_MASK_EXPECTED_FLAGS == ADDRESS_MASK_EXPECTED_FLAGS {
                1
            } else {
                2
            };
            core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).flags), flags);
            publish_completion_with_result(m, seq, completion, result);
            return false;
        }
        let (flags, completion, result) = if mode == MODE_PROGRAM_SECOND_SPARE {
            write_phase(m, PHASE_SAMPLING);
            let flags = run_second_spare_programming(m);
            let completion = if flags & SECOND_SPARE_REQUIRED_FLAGS == SECOND_SPARE_REQUIRED_FLAGS {
                COMPLETION_DONE
            } else {
                COMPLETION_PRECONDITION
            };
            (flags, completion, completion)
        } else {
            stamp_instant_result(m);
            (0, COMPLETION_REJECTED, COMPLETION_REJECTED)
        };
        core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).flags), flags);
        publish_completion_with_result(m, seq, completion, result);
        false
    }
}

fn quiet_stop() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

pub fn run() -> ! {
    unsafe {
        initialize_mailbox();
        let m = mailbox();
        loop {
            let seq = core::ptr::read_volatile(core::ptr::addr_of!((*m).seq));
            let ack = core::ptr::read_volatile(core::ptr::addr_of!((*m).ack));
            if seq != ack {
                clear_result(m);
                #[cfg(all(
                    feature = "rp1-pcie-4k-protection-proof",
                    not(feature = "rp1-iatu-second-spare-programming-proof")
                ))]
                prepare_dummy_page(m, seq);
                core::ptr::write_volatile(core::ptr::addr_of_mut!((*m).ack), seq);
                write_phase(m, PHASE_ACKED);
                write_phase(m, PHASE_WAIT_GO);

                let wait_start = raw_timer_us();
                while core::ptr::read_volatile(core::ptr::addr_of!((*m).go)) != seq {
                    if raw_timer_us().wrapping_sub(wait_start) >= GO_TIMEOUT_US {
                        stamp_instant_result(m);
                        publish_completion(m, seq, COMPLETION_GO_TIMEOUT);
                        break;
                    }
                    spin_loop();
                }

                if core::ptr::read_volatile(core::ptr::addr_of!((*m).go)) == seq {
                    let mode = core::ptr::read_volatile(core::ptr::addr_of!((*m).mode));
                    if run_mode(m, seq, mode) {
                        quiet_stop();
                    }
                }
            }
            spin_loop();
        }
    }
}

#![cfg_attr(not(target_arch = "arm"), allow(dead_code))]

pub const MAGIC: u32 = u32::from_le_bytes(*b"S0M1");
pub const VERSION: u32 = 1;
pub const WORDS: usize = 16;

const UNAVAILABLE: u32 = u32::MAX;
const REG_COUNT: usize = 4;
const STAGE_COUNT: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Snapshot {
    valid: u32,
    regs: [[u32; REG_COUNT]; STAGE_COUNT],
}

impl Snapshot {
    pub const fn new() -> Self {
        Self {
            valid: 0,
            regs: [[UNAVAILABLE; REG_COUNT]; STAGE_COUNT],
        }
    }

    pub fn record(&mut self, stage: usize, regs: [u32; REG_COUNT]) {
        self.regs[stage] = regs;
        self.valid |= 1 << stage;
    }
}

pub fn pack(decision: u32, snapshot: Snapshot) -> [u32; WORDS] {
    [
        MAGIC,
        VERSION,
        decision,
        snapshot.valid,
        snapshot.regs[0][0],
        snapshot.regs[0][1],
        snapshot.regs[0][2],
        snapshot.regs[0][3],
        snapshot.regs[1][0],
        snapshot.regs[1][1],
        snapshot.regs[1][2],
        snapshot.regs[1][3],
        snapshot.regs[2][0],
        snapshot.regs[2][1],
        snapshot.regs[2][2],
        snapshot.regs[2][3],
    ]
}

#[cfg(feature = "spi0-miso-configured-hold")]
pub const HOLD_MAGIC: u32 = u32::from_le_bytes(*b"S0H1");

#[cfg(feature = "spi0-miso-configured-hold")]
pub fn pack_hold(setup_ok: bool, snapshot: Snapshot) -> [u32; WORDS] {
    // READY means terminal software configuration only, never drive permission.
    let state = if !setup_ok {
        0x380 // SETUP_FAILED
    } else if snapshot.valid != 0b011 {
        0x381 // INCOMPLETE_SNAPSHOT
    } else {
        1 // CONFIGURED_HOLD_READY
    };
    let mut words = pack(state, snapshot);
    words[0] = HOLD_MAGIC;
    words[12] = u32::from(state == 1); // explicit READY; not an after-IRQ sample
    words[13..].fill(0); // reserved; no transfer or IRQ outcome
    words
}

#[cfg(target_arch = "arm")]
fn read_observation_regs() -> [u32; REG_COUNT] {
    const GPIO9_CTRL: *const u32 = 0x400d_004c as *const u32;
    const GPIO9_PAD: *const u32 = 0x400f_0028 as *const u32;
    const GPIO9_STATUS: *const u32 = 0x400d_0048 as *const u32;
    const SYS_RIO_OE: *const u32 = 0x400e_0004 as *const u32;

    unsafe {
        [
            core::ptr::read_volatile(GPIO9_CTRL),
            core::ptr::read_volatile(GPIO9_PAD),
            core::ptr::read_volatile(GPIO9_STATUS),
            core::ptr::read_volatile(SYS_RIO_OE),
        ]
    }
}

#[cfg(target_arch = "arm")]
pub fn record(snapshot: &mut Snapshot, stage: usize) {
    snapshot.record(stage, read_observation_regs());
}

#[cfg(all(target_arch = "arm", feature = "spi0-miso-input-observation"))]
pub fn publish(decision: u32, snapshot: Snapshot) -> u32 {
    const _: () = assert!(WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    let words = pack(decision, snapshot);
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, MAGIC);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    decision
}

#[cfg(all(target_arch = "arm", feature = "spi0-miso-configured-hold"))]
pub fn publish_hold(setup_ok: bool, snapshot: Snapshot) -> bool {
    const _: () = assert!(WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    let words = pack_hold(setup_ok, snapshot);
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, words[0]);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    words[12] == 1
}

#[cfg(feature = "spi0-miso-guarded-input-bias")]
pub const BIAS_MAGIC: u32 = u32::from_le_bytes(*b"S0B1");

#[cfg(any(
    feature = "spi0-miso-guarded-input-bias",
    feature = "spi0-timed-peer-zero-irq-proof",
    feature = "spi0-low-high-irq-rearm-proof",
    feature = "spi0-retained-rx-ser-proof",
    feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"
))]
fn input_route_disabled(regs: [u32; REG_COUNT]) -> bool {
    regs[0] == 0x80 && regs[2] & (1 << 13) == 0 && regs[3] & (1 << 9) == 0
}

#[cfg(any(
    feature = "spi0-miso-guarded-input-bias",
    feature = "spi0-timed-peer-zero-irq-proof",
    feature = "spi0-low-high-irq-rearm-proof",
    feature = "spi0-retained-rx-ser-proof",
    feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"
))]
fn guarded_pad(regs: [u32; REG_COUNT]) -> Option<u32> {
    if !input_route_disabled(regs) || regs[1] & 0xff != 0x73 {
        return None;
    }
    // Existing GPIO input_pull_up contract: OD=1, IE=1, PUE=1, PDE=0.
    // Preserve drive strength, Schmitt, slew and all fields outside this mask.
    Some((regs[1] & !(1 << 2)) | (1 << 7) | (1 << 6) | (1 << 3))
}

#[cfg(all(
    target_arch = "arm",
    any(
        feature = "spi0-miso-guarded-input-bias",
        feature = "spi0-timed-peer-zero-irq-proof",
        feature = "spi0-low-high-irq-rearm-proof",
        feature = "spi0-retained-rx-ser-proof",
        feature = "spi0-fifo-capacity-irq-proof", feature = "spi0-varied-peer-irq-proof"
    )
))]
#[inline(never)]
pub fn apply_guarded_bias() -> Result<u32, u32> {
    let expected_pad = guarded_pad(read_observation_regs()).ok_or(0x391u32)?;
    const GPIO9_PAD: *mut u32 = 0x400f_0028 as *mut u32;
    unsafe {
        core::ptr::write_volatile(GPIO9_PAD, expected_pad);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    Ok(expected_pad)
}

#[cfg(feature = "spi0-miso-guarded-input-bias")]
pub fn bias_readback_decision(expected_pad: u32, snapshot: Snapshot) -> u32 {
    if snapshot.valid != 0b111 {
        0x393 // INCOMPLETE_SNAPSHOT
    } else if guarded_pad(snapshot.regs[1]) != Some(expected_pad)
        || !input_route_disabled(snapshot.regs[2])
        || snapshot.regs[2][1] != expected_pad
    {
        0x392 // GUARD_READBACK_FAILED (includes changes outside the allowed PAD mask)
    } else {
        1 // GUARD_READY: configuration/readback only, never a required input HIGH.
    }
}

#[cfg(feature = "spi0-miso-guarded-input-bias")]
pub fn pack_bias(decision: u32, snapshot: Snapshot) -> [u32; WORDS] {
    let decision = if decision == 1 && snapshot.valid != 0b111 {
        0x393
    } else {
        decision
    };
    let mut words = pack(decision, snapshot);
    words[0] = BIAS_MAGIC;
    words
}

#[cfg(all(target_arch = "arm", feature = "spi0-miso-guarded-input-bias"))]
pub fn publish_bias(decision: u32, snapshot: Snapshot) -> bool {
    const _: () = assert!(WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);
    let out = rp1_hal::debug::MAILBOX_ADDR as *mut u32;
    let words = pack_bias(decision, snapshot);
    unsafe {
        core::ptr::write_volatile(out, 0);
        for (i, word) in words.iter().enumerate().skip(1) {
            core::ptr::write_volatile(out.add(i), *word);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        core::ptr::write_volatile(out, words[0]);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    words[2] == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "spi0-miso-guarded-input-bias")]
    #[test]
    fn bias_guard_and_schema_require_all_stages_without_requiring_high() {
        let normal = [0x80, 0xabcd_0073, 0x0440_0000, 0x0040_0000];
        let expected = guarded_pad(normal).unwrap();
        assert_eq!(expected, 0xabcd_00fb);
        assert_eq!((expected ^ normal[1]) & !0xcc, 0);
        for (reg, bit) in [
            (0, 0),
            (0, 12),
            (1, 2),
            (1, 3),
            (1, 6),
            (1, 7),
            (2, 13),
            (3, 9),
        ] {
            let mut unexpected = normal;
            unexpected[reg] ^= 1 << bit;
            assert_eq!(guarded_pad(unexpected), None);
        }
        let mut snapshot = Snapshot::new();
        assert_eq!(
            &pack_bias(0x390, snapshot)[0..4],
            &[BIAS_MAGIC, 1, 0x390, 0]
        );
        assert_eq!(&pack_bias(0x391, snapshot)[4..], &[UNAVAILABLE; 12]);
        assert_eq!(pack_bias(1, snapshot)[2], 0x393);
        snapshot.record(0, [0x9f, 0x96, 0x0440_0000, 0x0040_0000]);
        snapshot.record(1, normal);
        assert_eq!(bias_readback_decision(expected, snapshot), 0x393);
        assert_eq!(&pack_bias(0x391, snapshot)[12..], &[UNAVAILABLE; 4]);
        let mut guarded = normal;
        guarded[1] = expected;
        for level in [0, (1 << 17) | (1 << 18) | (1 << 19)] {
            guarded[2] = 0x0440_0000 | level;
            snapshot.record(2, guarded);
            assert_eq!(bias_readback_decision(expected, snapshot), 1);
            let words = pack_bias(1, snapshot);
            assert_eq!(&words[0..4], &[0x3142_3053, VERSION, 1, 7]);
            assert_eq!(&words[4..], &pack(1, snapshot)[4..]);
            assert_ne!(words[0], MAGIC);
        }
        for (reg, bit) in [
            (0, 0),
            (1, 0),
            (1, 2),
            (1, 3),
            (1, 6),
            (1, 7),
            (1, 31),
            (2, 13),
            (3, 9),
        ] {
            let mut unexpected = guarded;
            unexpected[reg] ^= 1 << bit;
            snapshot.record(2, unexpected);
            assert_eq!(bias_readback_decision(expected, snapshot), 0x392);
        }
        snapshot.record(2, guarded);
        assert_eq!(
            bias_readback_decision(expected ^ (1 << 31), snapshot),
            0x392
        );
        assert_eq!(pack_bias(0x390, snapshot)[2], 0x390);
    }

    #[cfg(feature = "spi0-miso-configured-hold")]
    #[test]
    fn hold_schema_never_promotes_missing_or_extra_stages_to_ready() {
        let mut snapshot = Snapshot::new();
        assert_eq!(
            &pack_hold(false, snapshot)[0..4],
            &[HOLD_MAGIC, 1, 0x380, 0]
        );
        assert_eq!(pack_hold(true, snapshot)[2], 0x381);
        snapshot.record(0, [0x9f, 0x96, 0x0440_0000, 0x0040_0000]);
        assert_eq!(pack_hold(true, snapshot)[12], 0);
        snapshot.record(1, [0x80, 0x73, 0x0440_0000, 0x0040_0000]);
        let words = pack_hold(true, snapshot);
        assert_ne!(words[0], MAGIC);
        assert_eq!(&words[0..4], &[HOLD_MAGIC, VERSION, 1, 3]);
        assert_eq!(&words[4..12], &pack(1, snapshot)[4..12]);
        assert_eq!(&words[12..], &[1, 0, 0, 0]);
        assert_eq!(pack_hold(false, snapshot)[12], 0);
        snapshot.record(2, [0; 4]);
        assert_eq!(pack_hold(true, snapshot)[2], 0x381);
        assert_eq!(pack_hold(true, snapshot)[12], 0);
    }

    #[test]
    fn packs_schema_raw_regs_and_missing_stages() {
        let mut snapshot = Snapshot::new();
        snapshot.record(0, [0x10, 0x11, 0x12, 0x13]);
        snapshot.record(2, [0x30, 0x31, 0x32, 0x33]);

        assert_eq!(
            pack(0x380, snapshot),
            [
                MAGIC,
                VERSION,
                0x380,
                0b101,
                0x10,
                0x11,
                0x12,
                0x13,
                UNAVAILABLE,
                UNAVAILABLE,
                UNAVAILABLE,
                UNAVAILABLE,
                0x30,
                0x31,
                0x32,
                0x33,
            ]
        );
    }
}

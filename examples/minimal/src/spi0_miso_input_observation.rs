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

#[cfg(target_arch = "arm")]
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

#[cfg(test)]
mod tests {
    use super::*;

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

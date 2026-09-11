//! Shared by the firmware and `rustc --test`; no hardware or RTOS dependencies.
pub const PERIOD_US: u32 = 200;
// Hardware calibration knob: minimum planned lead, not a measured WCET promise.
pub const ARM_GUARD_US: u32 = 40;
pub const SLOT_LIMIT: u32 = 100_000;
const _: () = assert!(ARM_GUARD_US < PERIOD_US && SLOT_LIMIT > 0
    && SLOT_LIMIT * PERIOD_US < 0x8000_0000);

/// Next point strictly beyond now + guard on the previous deadline's grid.
/// `previous` must already be due, less than half a raw-u32 wrap ago. Equality
/// at the guard skips that point. Quotient arithmetic never catches up in a loop.
pub const fn next_deadline(previous: u32, now: u32) -> Option<(u32, u32)> {
    let elapsed = now.wrapping_sub(previous);
    if elapsed >= 0x8000_0000 { return None; }
    let skipped = (elapsed + ARM_GUARD_US) / PERIOD_US;
    Some((previous.wrapping_add((skipped + 1) * PERIOD_US), skipped))
}

/// Count only skipped points inside this finite cohort. `true` means no next
/// alarm is allowed; the current IRQ is the final one, even if it drops a sample.
pub const fn bounded_skip(slot: u32, skipped: u32) -> Option<(u32, bool)> {
    if slot >= SLOT_LIMIT { return None; }
    let remaining = SLOT_LIMIT - 1 - slot;
    if skipped >= remaining { Some((remaining, true)) }
    else { Some((skipped, false)) }
}

#[test]
fn absolute_grid_wrap_equality_and_skips() {
    assert_eq!(next_deadline(1000, 1000), Some((1200, 0)));
    assert_eq!(next_deadline(1000, 1159), Some((1200, 0)));
    assert_eq!(next_deadline(1000, 1160), Some((1400, 1)));
    assert_eq!(next_deadline(1000, 1200), Some((1400, 1)));
    assert_eq!(next_deadline(1000, 1760), Some((2000, 4)));
    assert_eq!(next_deadline(0xffff_ff9c, 0xffff_ff9c), Some((100, 0)));
    assert_eq!(next_deadline(0xffff_ff9c, 60), Some((300, 1)));
    assert_eq!(next_deadline(1000, 999), None);
    assert_eq!(next_deadline(0, 0x8000_0000), None);
    assert_eq!(bounded_skip(0, 0), Some((0, false)));
    assert_eq!(bounded_skip(SLOT_LIMIT - 2, 0), Some((0, false)));
    assert_eq!(bounded_skip(SLOT_LIMIT - 2, 1), Some((1, true)));
    assert_eq!(bounded_skip(SLOT_LIMIT - 1, 0), Some((0, true)));
    assert_eq!(bounded_skip(SLOT_LIMIT - 2, u32::MAX), Some((1, true)));
    assert_eq!(bounded_skip(0, u32::MAX), Some((SLOT_LIMIT - 1, true)));
    assert_eq!(bounded_skip(SLOT_LIMIT, 0), None);
    for previous in [0u32, 1000, 0xffff_ff00, u32::MAX] {
        for elapsed in (0..2000).chain([0x7fff_ff00, 0x7fff_ffff]) {
            let now = previous.wrapping_add(elapsed);
            let (next, skipped) = next_deadline(previous, now).unwrap();
            let lead = next.wrapping_sub(now);
            assert!(lead > ARM_GUARD_US && lead <= ARM_GUARD_US + PERIOD_US);
            assert_eq!(next.wrapping_sub(previous) % PERIOD_US, 0);
            assert_eq!(next.wrapping_sub(previous) / PERIOD_US, skipped + 1);
        }
    }
}

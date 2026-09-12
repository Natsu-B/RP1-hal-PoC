//! Selected warm-only cold-reset UART preparation, never retained-state replay.
const PLL_RESET: u32 = 1 << 29;
const UART_RESET: u32 = 1 << 26;
#[cfg(target_arch = "arm")]
static mut SNAPSHOT: [u32; 16] = [0; 16];

#[cfg(target_arch = "arm")]
pub fn publish() {
    // run() clears the reserved telemetry AFTER preparation. Keep the saved
    // values in fresh BSS and mirror them only after that reserved-region clear.
    for i in 0..16 {
        let value = unsafe { core::ptr::addr_of!(SNAPSHOT).cast::<u32>().add(i).read_volatile() };
        super::put(108+i,value);
    }
}

fn initial_resets(ctrl0: u32, done0: u32, ctrl1: u32, done1: u32) -> bool {
    ctrl0 & PLL_RESET != 0 && done0 & PLL_RESET == 0
        && ctrl1 & UART_RESET != 0 && done1 & UART_RESET == 0
}

fn reset_state(ctrl0: u32, done0: u32, ctrl1: u32, done1: u32) -> u32 {
    u32::from(ctrl0 & PLL_RESET != 0) | (u32::from(done0 & PLL_RESET != 0) << 1)
        | (u32::from(ctrl1 & UART_RESET != 0) << 2) | (u32::from(done1 & UART_RESET != 0) << 3)
}

#[cfg(target_arch = "arm")]
pub fn prepare() -> Result<(), u32> {
    fn read(address: usize) -> u32 { unsafe { (address as *const u32).read_volatile() } }
    // All addresses already belong to the proven cold PLL/UART preparation.
    // Snapshot before ANY write. No UART body access until reset release/DONE.
    let before = [read(0x4001_4000),read(0x4001_4018),read(0x4001_4004),
        read(0x4001_401c),read(0x4002_0000),read(0x4002_0004),
        read(0x4002_0008),read(0x4002_000c),read(0x4002_0010),
        read(0x4002_0014),read(0x4001_8054),read(0x4001_8058),read(0x4001_8060)];
    for (i,value) in before.into_iter().enumerate() {
        unsafe { core::ptr::addr_of_mut!(SNAPSHOT).cast::<u32>().add(i).write_volatile(value); }
    }
    if !initial_resets(before[0],before[1],before[2],before[3]) {
        return Err(0x40 | reset_state(before[0],before[1],before[2],before[3]));
    }
    crate::release_pll_sys_reset_bit29().map_err(|_| 0x50u32)?;
    // Existing helper independently requires exact PLL reset-default fields.
    if crate::pll_sys_core_lock_transition().decision != crate::PllSysCoreLockDecision::Locked {
        return Err(0x51);
    }
    crate::enable_pll_sys_pri_ph_bit4().map_err(|_| 0x52u32)?;
    crate::release_uart0_reset_bank1_bit26().map_err(|_| 0x53u32)?;
    let pll = crate::read_pll_sys_snapshot();
    if pll.cs != 0x8000_0001 || pll.pwr != 4 || pll.fbdiv_int != 20
        || pll.fbdiv_frac != 0 || pll.prim != 0x0007_7010 || pll.sec != 0x8001_0000
        || read(0x4001_8054) & 0xfe0 != 0x840 || read(0x4001_8058) != 1
        || read(0x4001_4004) & UART_RESET != 0 || read(0x4001_401c) & UART_RESET == 0 {
        return Err(0x54);
    }
    // Remains a local receipt: no post-ACK host read or raw-register dump claim.
    for (i,value) in [u32::from_le_bytes(*b"WUP1"),read(0x4001_8054),read(0x4002_0010)].into_iter().enumerate() {
        unsafe { core::ptr::addr_of_mut!(SNAPSHOT).cast::<u32>().add(13+i).write_volatile(value); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_asserted_not_done_selected_resets() {
        assert!(initial_resets(PLL_RESET,0,UART_RESET,0));
        for p in 0..16u32 {
            let state=[if p&1!=0 {PLL_RESET}else{0},if p&2!=0 {PLL_RESET}else{0},
                if p&4!=0 {UART_RESET}else{0},if p&8!=0 {UART_RESET}else{0}];
            assert_eq!(initial_resets(state[0],state[1],state[2],state[3]),p==5);
            assert_eq!(reset_state(state[0],state[1],state[2],state[3]),p);
        }
        assert!(initial_resets(u32::MAX,!PLL_RESET,u32::MAX,!UART_RESET));
        assert!(!initial_resets(u32::MAX,u32::MAX,u32::MAX,u32::MAX));
        assert!(108+13<=121 && 123<124 && 159<184);
    }
}

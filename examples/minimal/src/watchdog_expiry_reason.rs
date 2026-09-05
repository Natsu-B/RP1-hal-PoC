pub type ResultFields = [u32; 22];

const WATCHDOG_BASE: usize = 0x4015_4000;
const CTRL: *mut u32 = WATCHDOG_BASE as *mut u32;
const LOAD: *mut u32 = (WATCHDOG_BASE + 0x04) as *mut u32;
const REASON: *const u32 = (WATCHDOG_BASE + 0x08) as *const u32;
const BOOT_MAGIC: *mut u32 = (WATCHDOG_BASE + 0x0c) as *mut u32;
const PROC0_ENTRY: *mut u32 = (WATCHDOG_BASE + 0x10) as *mut u32;
const PROC0_SP: *mut u32 = (WATCHDOG_BASE + 0x18) as *mut u32;
const SCRATCH7: *mut u32 = (WATCHDOG_BASE + 0x28) as *mut u32;

const VECTOR_BASE: usize = 0x2000_0000;
const SRAM_END: u32 = 0x2001_0000;
const BOOT_MAGIC_VALUE: u32 = 0xb007_c0de;
const CTRL_ENABLE: u32 = 1 << 30;
const CTRL_TRIGGER: u32 = 1 << 31;
const LOAD_VALUE: u32 = 200_000;

const READY_MAGIC: u32 = 0x3052_4457; // WDR0
const RESULT_MAGIC: u32 = 0x3152_4457; // WDR1
const REQUEST_MAGIC: u32 = 0x3151_4457; // WDQ1
const STATE_MAGIC: u32 = 0x3141_4457; // WDA1
const SCRATCH_SENTINEL: u32 = 0x3150_5857; // WXP1
const CONTROL_OFFSET: usize = 0x70;
const STATE_OFFSET: usize = 0x80;
const STATE_WORDS: usize = 13;

const FLAG_SP_VALID: u32 = 1 << 0;
const FLAG_ENTRY_VALID: u32 = 1 << 1;
const FLAG_REASON_TIMER_CLEAR: u32 = 1 << 2;
const FLAG_CTRL_DISABLED: u32 = 1 << 3;
const FLAG_NO_SENTINEL_COLLISION: u32 = 1 << 4;
const FLAG_RESUMED: u32 = 1 << 5;
const FLAG_STATE_VALID: u32 = 1 << 6;
const FLAG_ENTRY_CONSUMED: u32 = 1 << 7;
const FLAG_REASON_CHANGED: u32 = 1 << 8;
const FLAG_REASON_TIMER_SET: u32 = 1 << 9;
const FLAG_SCRATCH0_RESTORED: u32 = 1 << 10;
const FLAG_SCRATCH1_RESTORED: u32 = 1 << 11;
const FLAG_SCRATCH3_RESTORED: u32 = 1 << 12;
const FLAG_SCRATCH7_RESTORED: u32 = 1 << 13;
const FLAG_WATCHDOG_DISABLED: u32 = 1 << 14;
const FLAG_ENDPOINT_REPAIRED: u32 = 1 << 15;
const FLAG_PASS: u32 = 1 << 31;
const REQUIRED_FLAGS: u32 = (1 << 15) - 1;

const ENDPOINT_IATU_SELECTORS: [u32; 2] = [0x23, 0x63];
const ENDPOINT_IATU_OFFSETS: [usize; 4] = [0x114, 0x118, 0x100, 0x104];
const ENDPOINT_IATU_EXPECTED: [u32; 8] = [
    0x4000_0000,
    0x0000_00c0,
    0,
    0xc000_0100,
    0x2000_0000,
    0x0000_00c0,
    0,
    0xc000_0200,
];

fn mailbox() -> *mut u32 {
    rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32
}

unsafe fn read(register: *const u32) -> u32 {
    unsafe { core::ptr::read_volatile(register) }
}

unsafe fn write(register: *mut u32, value: u32) {
    unsafe { core::ptr::write_volatile(register, value) }
}

unsafe fn restore_endpoint_iatu(selector: *mut u32, dbi_base: usize) {
    for value in ENDPOINT_IATU_SELECTORS {
        unsafe {
            write(selector, value);
            write((dbi_base + ENDPOINT_IATU_OFFSETS[3]) as *mut u32, 0);
            for field in 0..3 {
                write((dbi_base + ENDPOINT_IATU_OFFSETS[field]) as *mut u32, 0);
            }
        }
    }
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)) };
}

unsafe fn repair_endpoint_if_reset() -> Result<bool, ()> {
    const DBI_SELECTOR: *mut u32 = 0x4010_8000 as *mut u32;
    const DBI_BASE: usize = 0x4010_9000;
    const CLASS_REVISION: *mut u32 = (DBI_BASE + 0x008) as *mut u32;
    const DBI_RO_WR_EN: *mut u32 = (DBI_BASE + 0x8bc) as *mut u32;

    let selector_before = unsafe { read(DBI_SELECTOR) };
    unsafe { write(DBI_SELECTOR, 0) };
    let class_before = unsafe { read(CLASS_REVISION) };
    if class_before & 0xffff_ff00 != 0 {
        unsafe { write(DBI_SELECTOR, selector_before) };
        return Ok(false);
    }
    if class_before != 0x0000_0002 {
        unsafe { write(DBI_SELECTOR, selector_before) };
        return Err(());
    }

    let mut iatu_before = [0; 8];
    for (region, selector) in ENDPOINT_IATU_SELECTORS.iter().enumerate() {
        unsafe { write(DBI_SELECTOR, *selector) };
        for (field, offset) in ENDPOINT_IATU_OFFSETS.iter().enumerate() {
            iatu_before[region * 4 + field] = unsafe { read((DBI_BASE + *offset) as *const u32) };
        }
    }
    if iatu_before != [0; 8] {
        unsafe { write(DBI_SELECTOR, selector_before) };
        return Err(());
    }

    let mut iatu_after = [0; 8];
    for (region, selector) in ENDPOINT_IATU_SELECTORS.iter().enumerate() {
        let base = region * 4;
        unsafe { write(DBI_SELECTOR, *selector) };
        for field in 0..3 {
            unsafe {
                write(
                    (DBI_BASE + ENDPOINT_IATU_OFFSETS[field]) as *mut u32,
                    ENDPOINT_IATU_EXPECTED[base + field],
                );
                iatu_after[base + field] =
                    read((DBI_BASE + ENDPOINT_IATU_OFFSETS[field]) as *const u32);
            }
        }
        if iatu_after[base..base + 3] != ENDPOINT_IATU_EXPECTED[base..base + 3] {
            unsafe {
                restore_endpoint_iatu(DBI_SELECTOR, DBI_BASE);
                write(DBI_SELECTOR, selector_before);
            }
            return Err(());
        }
        unsafe {
            write(
                (DBI_BASE + ENDPOINT_IATU_OFFSETS[3]) as *mut u32,
                ENDPOINT_IATU_EXPECTED[base + 3],
            );
            iatu_after[base + 3] = read((DBI_BASE + ENDPOINT_IATU_OFFSETS[3]) as *const u32);
        }
        if iatu_after[base + 3] != ENDPOINT_IATU_EXPECTED[base + 3] {
            unsafe {
                restore_endpoint_iatu(DBI_SELECTOR, DBI_BASE);
                write(DBI_SELECTOR, selector_before);
            }
            return Err(());
        }
    }

    unsafe {
        write(DBI_SELECTOR, 0);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let ro_write_before = unsafe { read(DBI_RO_WR_EN) };
    unsafe {
        write(DBI_RO_WR_EN, ro_write_before | 1);
        write(CLASS_REVISION, 0x0200_0002);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let class_after = unsafe { read(CLASS_REVISION) };
    unsafe {
        write(DBI_RO_WR_EN, ro_write_before);
        if class_after != 0x0200_0002 {
            restore_endpoint_iatu(DBI_SELECTOR, DBI_BASE);
        }
        write(DBI_SELECTOR, selector_before);
    }
    if class_after != 0x0200_0002 {
        return Err(());
    }
    Ok(true)
}

fn checksum(words: &[u32]) -> u32 {
    words.iter().fold(0x5744_4353, |sum, word| sum ^ word)
}

unsafe fn publish_record(magic: u32, fields: ResultFields) {
    const RESULT_WORDS: usize = 1 + core::mem::size_of::<ResultFields>() / 4;
    const _: () = assert!(RESULT_WORDS * 4 <= CONTROL_OFFSET);
    const _: () = assert!(STATE_OFFSET + STATE_WORDS * 4 <= rp1_hal::debug::MAILBOX_SIZE);

    let words = mailbox();
    unsafe {
        write(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            write(words.add(index + 1), value);
        }
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        write(words, magic);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

pub fn publish(fields: ResultFields) {
    unsafe { publish_record(RESULT_MAGIC, fields) }
}

fn snapshot_pre(fields: &mut ResultFields) -> u32 {
    unsafe {
        let ctrl = read(CTRL);
        let reason = read(REASON);
        let scratch0 = read(BOOT_MAGIC);
        let scratch1 = read(PROC0_ENTRY);
        let scratch3 = read(PROC0_SP);
        let scratch7 = read(SCRATCH7);
        let vector_sp = read(VECTOR_BASE as *const u32);
        let vector_entry = read((VECTOR_BASE + 4) as *const u32);
        let mut flags = 0;

        if vector_sp >= VECTOR_BASE as u32 && vector_sp <= SRAM_END && vector_sp & 3 == 0 {
            flags |= FLAG_SP_VALID;
        }
        if vector_entry & 1 != 0
            && (vector_entry & !1) >= VECTOR_BASE as u32
            && (vector_entry & !1) < SRAM_END
        {
            flags |= FLAG_ENTRY_VALID;
        }
        if reason & 1 == 0 {
            flags |= FLAG_REASON_TIMER_CLEAR;
        }
        if ctrl & CTRL_ENABLE == 0 {
            flags |= FLAG_CTRL_DISABLED;
        }
        if scratch7 != SCRATCH_SENTINEL {
            flags |= FLAG_NO_SENTINEL_COLLISION;
        }

        fields[0] = flags;
        fields[1] = 1;
        fields[2] = LOAD_VALUE;
        fields[3] = ctrl;
        fields[4] = reason;
        fields[5] = scratch0;
        fields[6] = scratch1;
        fields[7] = scratch3;
        fields[8] = scratch7;
        fields[9] = vector_sp;
        fields[10] = vector_entry;
        fields[21] = vector_entry ^ BOOT_MAGIC_VALUE;
        flags
    }
}

unsafe fn restore_handoff(state: &[u32; STATE_WORDS]) -> u32 {
    unsafe {
        write(BOOT_MAGIC, state[4]);
        write(PROC0_ENTRY, state[5]);
        write(PROC0_SP, state[6]);
        write(SCRATCH7, state[7]);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));

        let mut flags = 0;
        if read(BOOT_MAGIC) == state[4] {
            flags |= FLAG_SCRATCH0_RESTORED;
        }
        if read(PROC0_ENTRY) == state[5] {
            flags |= FLAG_SCRATCH1_RESTORED;
        }
        if read(PROC0_SP) == state[6] {
            flags |= FLAG_SCRATCH3_RESTORED;
        }
        if read(SCRATCH7) == state[7] {
            flags |= FLAG_SCRATCH7_RESTORED;
        }
        flags
    }
}

unsafe fn resume(state_words: *mut u32) -> ResultFields {
    let mut state = [0u32; STATE_WORDS];
    for (index, value) in state.iter_mut().enumerate() {
        *value = unsafe { read(state_words.add(index)) };
    }
    let observed_checksum = state[1];
    let expected_checksum = checksum(&state[2..]);
    let ctrl_after = unsafe { read(CTRL) };
    let reason_after = unsafe { read(REASON) };
    let scratch0_after = unsafe { read(BOOT_MAGIC) };
    let scratch1_after = unsafe { read(PROC0_ENTRY) };
    let scratch3_after = unsafe { read(PROC0_SP) };
    let scratch7_after = unsafe { read(SCRATCH7) };
    let ctrl_final = ctrl_after & !CTRL_ENABLE & !CTRL_TRIGGER;
    unsafe {
        write(CTRL, ctrl_final);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    let state_valid = state[0] == STATE_MAGIC && observed_checksum == expected_checksum;
    let mut flags = FLAG_RESUMED;
    if state_valid {
        flags |= state[11] | FLAG_STATE_VALID;
        if scratch1_after == 0 {
            flags |= FLAG_ENTRY_CONSUMED;
        }
        if reason_after != state[3] {
            flags |= FLAG_REASON_CHANGED;
        }
        if reason_after & 1 != 0 {
            flags |= FLAG_REASON_TIMER_SET;
        }
        flags |= unsafe { restore_handoff(&state) };
    }
    let ctrl_final = unsafe { read(CTRL) };
    if ctrl_final & CTRL_ENABLE == 0 {
        flags |= FLAG_WATCHDOG_DISABLED;
    }
    if flags & REQUIRED_FLAGS == REQUIRED_FLAGS {
        flags |= FLAG_PASS;
    }

    unsafe {
        write(state_words, 0);
        write(mailbox().add(CONTROL_OFFSET / 4), 0);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    [
        flags,
        if state_valid { 2 } else { 6 },
        state[10],
        state[2],
        state[3],
        state[4],
        state[5],
        state[6],
        state[7],
        state[8],
        state[9],
        ctrl_after,
        ctrl_final,
        reason_after,
        scratch0_after,
        scratch1_after,
        scratch3_after,
        scratch7_after,
        unsafe { read(SCRATCH7) },
        observed_checksum,
        expected_checksum,
        state[12],
    ]
}

pub fn run_or_arm() -> ResultFields {
    let words = mailbox();
    let control = unsafe { words.add(CONTROL_OFFSET / 4) };
    let state_words = unsafe { words.add(STATE_OFFSET / 4) };
    let state_magic = unsafe { read(state_words) };
    let scratch7 = unsafe { read(SCRATCH7) };

    if state_magic == STATE_MAGIC && scratch7 == SCRATCH_SENTINEL {
        return unsafe { resume(state_words) };
    }

    let mut fields = [0u32; 22];
    let flags = snapshot_pre(&mut fields);
    if state_magic == STATE_MAGIC || scratch7 == SCRATCH_SENTINEL || flags != 0x1f {
        fields[1] = 3;
        return fields;
    }

    unsafe {
        write(control, 0);
        publish_record(READY_MAGIC, fields);
        let mut endpoint_repaired = false;
        while read(control) != REQUEST_MAGIC {
            if !endpoint_repaired {
                match repair_endpoint_if_reset() {
                    Ok(false) => {}
                    Ok(true) => {
                        endpoint_repaired = true;
                        fields[0] |= FLAG_ENDPOINT_REPAIRED;
                    }
                    Err(()) => {
                        fields[1] = 7;
                        return fields;
                    }
                }
            }
            core::hint::spin_loop();
        }
        if !endpoint_repaired {
            fields[1] = 7;
            return fields;
        }
        write(control, 0);

        let state = [
            STATE_MAGIC,
            0,
            fields[3],
            fields[4],
            fields[5],
            fields[6],
            fields[7],
            fields[8],
            fields[9],
            fields[10],
            LOAD_VALUE,
            fields[0],
            fields[21],
        ];
        write(state_words, 0);
        for (index, value) in state[2..].iter().copied().enumerate() {
            write(state_words.add(index + 2), value);
        }
        write(state_words.add(1), checksum(&state[2..]));
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        write(state_words, STATE_MAGIC);

        write(SCRATCH7, SCRATCH_SENTINEL);
        write(PROC0_SP, fields[9]);
        write(PROC0_ENTRY, fields[21]);
        write(BOOT_MAGIC, BOOT_MAGIC_VALUE);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        if read(SCRATCH7) != SCRATCH_SENTINEL
            || read(PROC0_SP) != fields[9]
            || read(PROC0_ENTRY) != fields[21]
            || read(BOOT_MAGIC) != BOOT_MAGIC_VALUE
        {
            let mut restored = state;
            restored[1] = checksum(&restored[2..]);
            fields[0] |= restore_handoff(&restored);
            fields[1] = 4;
            write(state_words, 0);
            return fields;
        }

        write(LOAD, LOAD_VALUE);
        write(CTRL, (fields[3] & !CTRL_TRIGGER) | CTRL_ENABLE);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
        if read(CTRL) & CTRL_ENABLE == 0 {
            let mut restored = state;
            restored[1] = checksum(&restored[2..]);
            fields[0] |= restore_handoff(&restored);
            fields[1] = 5;
            write(state_words, 0);
            return fields;
        }
        loop {
            core::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
        }
    }
}

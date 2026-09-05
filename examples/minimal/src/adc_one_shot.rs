pub type ResultFields = [u32; 38];

const ADC_BASE: usize = 0x400c_8000;
const ADC_CS: *mut u32 = ADC_BASE as *mut u32;
const ADC_RESULT: *const u32 = (ADC_BASE + 0x04) as *const u32;
const ADC_FCS: *const u32 = (ADC_BASE + 0x08) as *const u32;
const ADC_INTE: *const u32 = (ADC_BASE + 0x18) as *const u32;
const ADC_INTS: *const u32 = (ADC_BASE + 0x20) as *const u32;
const ADC_CS_SET: *mut u32 = (ADC_BASE + 0x2000) as *mut u32;
const ADC_CS_CLR: *mut u32 = (ADC_BASE + 0x3000) as *mut u32;

const CLK_ADC_CTRL: *mut u32 = 0x4001_8144 as *mut u32;
const CLK_ADC_DIV_INT: *mut u32 = 0x4001_8148 as *mut u32;
const CLK_ADC_SEL: *const u32 = 0x4001_8150 as *const u32;

const RESET_CTRL0: *const u32 = 0x4001_4000 as *const u32;
const RESET_DONE0: *const u32 = 0x4001_4018 as *const u32;
const CLK_SYS: [*const u32; 3] = [
    0x4001_8014 as *const u32,
    0x4001_8018 as *const u32,
    0x4001_8020 as *const u32,
];
const POWER_008: *const u32 = 0x4001_0008 as *const u32;

const CLOCK_ENABLE: u32 = 1 << 11;
const CLOCK_AUXSRC_MASK: u32 = 0x1f << 5;
const ADC_CS_AINSEL_MASK: u32 = 0x7 << 12;
const ADC_CS_ERR_STICKY: u32 = 1 << 10;
const ADC_CS_ERR: u32 = 1 << 9;
const ADC_CS_READY: u32 = 1 << 8;
const ADC_CS_START_MANY: u32 = 1 << 3;
const ADC_CS_START_ONCE: u32 = 1 << 2;
const ADC_CS_TS_EN: u32 = 1 << 1;
const ADC_CS_EN: u32 = 1;
const ADC_FCS_EN: u32 = 1;
const ADC_CHANNEL: u32 = 4;
const POLL_LIMIT: u32 = 4096;

const FLAG_PRECONDITION: u32 = 1 << 0;
const FLAG_CLOCK_ACTIVE: u32 = 1 << 1;
const FLAG_XOSC_SELECTED: u32 = 1 << 2;
const FLAG_READY: u32 = 1 << 3;
const FLAG_NO_ERROR: u32 = 1 << 4;
const FLAG_RAW_12BIT: u32 = 1 << 5;
const FLAG_ADC_QUIESCED: u32 = 1 << 6;
const FLAG_CLOCK_RESTORED: u32 = 1 << 7;
const FLAG_PASS: u32 = 1 << 31;
const REQUIRED_FLAGS: u32 = (1 << 8) - 1;

const STAGE_PRECONDITION: u32 = 1;
const STAGE_CLOCK: u32 = 2;
const STAGE_CONVERSION: u32 = 3;
const STAGE_RESTORE: u32 = 4;
const STAGE_PASS: u32 = 5;
const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"ADC1");

unsafe fn read(register: *const u32) -> u32 {
    unsafe { core::ptr::read_volatile(register) }
}

unsafe fn write(register: *mut u32, value: u32) {
    unsafe {
        core::ptr::write_volatile(register, value);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn wait_for_sel(expected_set: bool) -> u32 {
    for polls in 1..=POLL_LIMIT {
        let selected = unsafe { read(CLK_ADC_SEL) } & 1 != 0;
        if selected == expected_set {
            return polls;
        }
        core::hint::spin_loop();
    }
    POLL_LIMIT
}

fn millivolts(raw: u32) -> u32 {
    (3300 * raw + 2047) / 4095
}

fn temperature_millidegrees(millivolts: u32) -> i32 {
    let delta = (millivolts as i64 - 706) * 1_000_000;
    let rounded = if delta >= 0 {
        (delta + 860) / 1721
    } else {
        (delta - 860) / 1721
    };
    27_000 - rounded as i32
}

fn checksum(words: &[u32]) -> u32 {
    words.iter().fold(0x4144_4353, |sum, word| sum ^ word)
}

pub fn publish(mut fields: ResultFields) {
    fields[37] = checksum(&fields[..37]);
    const RESULT_WORDS: usize = 1 + core::mem::size_of::<ResultFields>() / 4;
    const _: () = assert!(RESULT_WORDS * 4 <= 0x100);
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        write(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            write(words.add(index + 1), value);
        }
        write(words, RESULT_MAGIC);
    }
}

pub fn run() -> ResultFields {
    let mut fields = [0u32; 38];
    fields[0] = 2;
    fields[2] = STAGE_PRECONDITION;
    fields[3] = ADC_CHANNEL;

    let clk_ctrl_before = unsafe { read(CLK_ADC_CTRL) };
    let clk_div_before = unsafe { read(CLK_ADC_DIV_INT) };
    let clk_sel_before = unsafe { read(CLK_ADC_SEL) };
    let cs_before = unsafe { read(ADC_CS) };
    let result_before = unsafe { read(ADC_RESULT) };
    let fcs_before = unsafe { read(ADC_FCS) };
    let inte_before = unsafe { read(ADC_INTE) };
    fields[8] = clk_ctrl_before;
    fields[9] = clk_div_before;
    fields[10] = clk_sel_before;
    fields[11] = cs_before;
    fields[12] = result_before;
    fields[13] = fcs_before;
    fields[14] = inte_before;
    fields[27] = unsafe { read(RESET_CTRL0) };
    fields[28] = unsafe { read(RESET_DONE0) };
    for (index, register) in CLK_SYS.into_iter().enumerate() {
        fields[29 + index] = unsafe { read(register) };
    }
    fields[32] = unsafe { read(POWER_008) };

    let controlled =
        ADC_CS_AINSEL_MASK | ADC_CS_START_MANY | ADC_CS_START_ONCE | ADC_CS_TS_EN | ADC_CS_EN;
    let clock_was_enabled = clk_ctrl_before & CLOCK_ENABLE != 0;
    let precondition = cs_before & controlled == 0
        && fcs_before & ADC_FCS_EN == 0
        && inte_before == 0
        && clk_ctrl_before & CLOCK_AUXSRC_MASK == 0
        && (!clock_was_enabled || (clk_div_before == 1 && clk_sel_before & 1 != 0));
    if !precondition {
        return fields;
    }
    fields[1] |= FLAG_PRECONDITION;

    fields[2] = STAGE_CLOCK;
    if !clock_was_enabled {
        unsafe {
            write(CLK_ADC_DIV_INT, 1);
            write(CLK_ADC_CTRL, clk_ctrl_before | CLOCK_ENABLE);
        }
        wait_for_sel(true);
    }
    fields[15] = unsafe { read(CLK_ADC_CTRL) };
    fields[16] = unsafe { read(CLK_ADC_DIV_INT) };
    fields[17] = unsafe { read(CLK_ADC_SEL) };
    if fields[15] & CLOCK_ENABLE != 0 && fields[16] == 1 {
        fields[1] |= FLAG_CLOCK_ACTIVE;
    }
    if fields[17] & 1 != 0 {
        fields[1] |= FLAG_XOSC_SELECTED;
    }
    if fields[1] & (FLAG_CLOCK_ACTIVE | FLAG_XOSC_SELECTED)
        != FLAG_CLOCK_ACTIVE | FLAG_XOSC_SELECTED
    {
        if !clock_was_enabled {
            unsafe {
                write(CLK_ADC_CTRL, clk_ctrl_before);
                write(CLK_ADC_DIV_INT, clk_div_before);
            }
        }
        return fields;
    }

    fields[2] = STAGE_CONVERSION;
    unsafe {
        write(ADC_CS, ADC_CS_EN | ADC_CS_ERR_STICKY);
        fields[33] = read(ADC_CS);
        write(ADC_CS_CLR, ADC_CS_AINSEL_MASK);
        write(ADC_CS_SET, ADC_CHANNEL << 12);
        fields[34] = read(ADC_CS);
        write(ADC_CS_SET, ADC_CS_TS_EN);
        fields[35] = read(ADC_CS);
        write(ADC_CS_SET, ADC_CS_START_ONCE);
        fields[36] = read(ADC_CS);
    }
    let mut cs_after = 0;
    for polls in 1..=POLL_LIMIT {
        cs_after = unsafe { read(ADC_CS) };
        fields[4] = polls;
        if cs_after & ADC_CS_READY != 0 {
            fields[1] |= FLAG_READY;
            break;
        }
        core::hint::spin_loop();
    }
    let raw = unsafe { read(ADC_RESULT) };
    fields[5] = raw;
    fields[6] = millivolts(raw & 0x0fff);
    fields[7] = temperature_millidegrees(fields[6]) as u32;
    fields[18] = cs_after;
    fields[19] = raw;
    fields[20] = unsafe { read(ADC_FCS) };
    fields[21] = unsafe { read(ADC_INTE) };
    fields[22] = unsafe { read(ADC_INTS) };
    if cs_after & ADC_CS_ERR == 0 {
        fields[1] |= FLAG_NO_ERROR;
    }
    if raw <= 0x0fff {
        fields[1] |= FLAG_RAW_12BIT;
    }

    fields[2] = STAGE_RESTORE;
    unsafe { write(ADC_CS_CLR, controlled) };
    fields[23] = unsafe { read(ADC_CS) };
    if fields[23] & controlled == 0
        && unsafe { read(ADC_INTE) } == inte_before
        && unsafe { read(ADC_FCS) } & ADC_FCS_EN == 0
    {
        fields[1] |= FLAG_ADC_QUIESCED;
    }

    if !clock_was_enabled {
        unsafe {
            write(CLK_ADC_CTRL, clk_ctrl_before);
            write(CLK_ADC_DIV_INT, clk_div_before);
        }
        wait_for_sel(clk_sel_before & 1 != 0);
    }
    fields[24] = unsafe { read(CLK_ADC_CTRL) };
    fields[25] = unsafe { read(CLK_ADC_DIV_INT) };
    fields[26] = unsafe { read(CLK_ADC_SEL) };
    if fields[24] == clk_ctrl_before
        && fields[25] == clk_div_before
        && (clock_was_enabled || (fields[26] & 1) == (clk_sel_before & 1))
    {
        fields[1] |= FLAG_CLOCK_RESTORED;
    }

    if fields[1] & REQUIRED_FLAGS == REQUIRED_FLAGS {
        fields[1] |= FLAG_PASS;
        fields[2] = STAGE_PASS;
    }
    fields
}

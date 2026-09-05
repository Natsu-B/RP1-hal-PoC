pub type ResultFields = [u32; 39];

const RESET_CTRL0: *const u32 = 0x4001_4000 as *const u32;
const RESET_DONE0: *const u32 = 0x4001_4018 as *const u32;
const CLK_I2S_CTRL: *const u32 = 0x4001_80b4 as *const u32;
const CLK_I2S_DIV_INT: *const u32 = 0x4001_80b8 as *const u32;
const CLK_I2S_SEL: *const u32 = 0x4001_80c0 as *const u32;

const I2S_BASES: [usize; 3] = [0x400a_0000, 0x400a_4000, 0x400a_8000];
const IDENTITY_OFFSETS: [usize; 4] = [0x1f0, 0x1f4, 0x1f8, 0x1fc];
const SNAPSHOT_WORDS: usize = 17;

const FLAG_FIRST_CAPTURE: u32 = 1 << 0;
const FLAG_SECOND_CAPTURE: u32 = 1 << 1;
const FLAG_PREREQUISITES_STABLE: u32 = 1 << 2;
const FLAG_IDENTITY_STABLE: u32 = 1 << 3;
const FLAG_I2S_RESETS_ASSERTED: u32 = 1 << 4;
const FLAG_CLOCK_ENABLED: u32 = 1 << 5;
const FLAG_PASS: u32 = 1 << 31;

const I2S_RESET_MASK: u32 = (1 << 14) | (1 << 15) | (1 << 16);
const CLOCK_ENABLE: u32 = 1 << 11;
const RESULT_MAGIC: u32 = u32::from_le_bytes(*b"I2RO");

unsafe fn read(register: *const u32) -> u32 {
    unsafe { core::ptr::read_volatile(register) }
}

unsafe fn publish_word(register: *mut u32, value: u32) {
    unsafe {
        core::ptr::write_volatile(register, value);
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn barrier() {
    unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)) }
}

fn capture() -> [u32; SNAPSHOT_WORDS] {
    let mut words = [0u32; SNAPSHOT_WORDS];
    words[0] = unsafe { read(RESET_CTRL0) };
    words[1] = unsafe { read(RESET_DONE0) };
    words[2] = unsafe { read(CLK_I2S_CTRL) };
    words[3] = unsafe { read(CLK_I2S_DIV_INT) };
    words[4] = unsafe { read(CLK_I2S_SEL) };

    let mut index = 5;
    for base in I2S_BASES {
        for offset in IDENTITY_OFFSETS {
            words[index] = unsafe { read((base + offset) as *const u32) };
            index += 1;
        }
    }
    words
}

fn checksum(words: &[u32]) -> u32 {
    words.iter().fold(0x4932_5253, |sum, word| sum ^ word)
}

pub fn run() -> ResultFields {
    let mut fields = [0u32; 39];
    fields[0] = 1;
    fields[37] = I2S_BASES.len() as u32;

    let first = capture();
    fields[1] |= FLAG_FIRST_CAPTURE;
    fields[3..20].copy_from_slice(&first);
    barrier();
    let second = capture();
    fields[1] |= FLAG_SECOND_CAPTURE;
    fields[20..37].copy_from_slice(&second);

    let mut changed = 0u32;
    for index in 0..SNAPSHOT_WORDS {
        if first[index] != second[index] {
            changed |= 1 << index;
        }
    }
    fields[2] = changed;

    if first[..5] == second[..5] {
        fields[1] |= FLAG_PREREQUISITES_STABLE;
    }
    if first[5..] == second[5..] {
        fields[1] |= FLAG_IDENTITY_STABLE;
    }
    if first[0] & I2S_RESET_MASK == I2S_RESET_MASK && first[1] & I2S_RESET_MASK == 0 {
        fields[1] |= FLAG_I2S_RESETS_ASSERTED;
    }
    if first[2] & CLOCK_ENABLE != 0 {
        fields[1] |= FLAG_CLOCK_ENABLED;
    }
    if changed == 0 {
        fields[1] |= FLAG_PASS;
    }
    fields
}

pub fn publish(mut fields: ResultFields) {
    fields[38] = checksum(&fields[..38]);
    const RESULT_WORDS: usize = 1 + core::mem::size_of::<ResultFields>() / 4;
    const _: () = assert!(RESULT_WORDS * 4 <= 0x100);
    let words = rp1_hal::debug::MAILBOX_ADDR as usize as *mut u32;
    unsafe {
        publish_word(words, 0);
        for (index, value) in fields.into_iter().enumerate() {
            publish_word(words.add(index + 1), value);
        }
        publish_word(words, RESULT_MAGIC);
    }
}

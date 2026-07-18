pub const RP1_PERI_BASE: usize = 0x4000_0000;

pub const GPIO_BASE: usize = RP1_PERI_BASE + 0x000d_0000;
pub const UART0_BASE: usize = RP1_PERI_BASE + 0x0003_0000;
pub const UART1_BASE: usize = RP1_PERI_BASE + 0x0003_4000;
pub const I2C0_BASE: usize = RP1_PERI_BASE + 0x0007_0000;
pub const I2C1_BASE: usize = RP1_PERI_BASE + 0x0007_4000;
pub const SPI0_BASE: usize = RP1_PERI_BASE + 0x0005_0000;
pub const PIO0_BASE: usize = RP1_PERI_BASE + 0x0017_8000;
pub const DMA_BASE: usize = RP1_PERI_BASE + 0x0018_8000;

pub const RAW_TIMER_HIGH: usize = RP1_PERI_BASE + 0x000a_c024;
pub const RAW_TIMER_LOW: usize = RP1_PERI_BASE + 0x000a_c028;

// TODO: verify these offsets against the RP1 datasheet and the Linux DTB before
// making any register-level API perform writes.

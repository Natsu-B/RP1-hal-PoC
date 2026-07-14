use crate::mmio::Reg;
use core::sync::atomic::{AtomicU32, Ordering};

const IO_BANK0_BASE: usize = 0x400d_0000;
const PADS_BANK0_BASE: usize = 0x400f_0000;
const CLOCKS_MAIN_BASE: usize = 0x4001_8000;
const UART0_BASE: usize = 0x4003_0000;

const CLK_UART_CTRL: usize = 0x54;
const CLK_UART_DIV_INT: usize = 0x58;
const CLK_UART_CTRL_XOSC: u32 = 2 << 5;
const CLK_UART_CTRL_ENABLE: u32 = 1 << 11;

const UART_DR: usize = 0x00;
const UART_RSR_ECR: usize = 0x04;
const UART_FR: usize = 0x18;
const UART_IBRD: usize = 0x24;
const UART_FBRD: usize = 0x28;
const UART_LCRH: usize = 0x2c;
const UART_CR: usize = 0x30;
const UART_IFLS: usize = 0x34;
const UART_IMSC: usize = 0x38;
const UART_RIS: usize = 0x3c;
const UART_MIS: usize = 0x40;
const UART_ICR: usize = 0x44;

const UART_FR_BUSY: u32 = 1 << 3;
const UART_FR_RXFE: u32 = 1 << 4;
const UART_FR_TXFF: u32 = 1 << 5;
const UART_DR_DATA_MASK: u32 = 0xff;
const UART_DR_ERROR_MASK: u32 = 0x0f00;
const UART_CR_UARTEN: u32 = 1 << 0;
const UART_CR_TXE: u32 = 1 << 8;
const UART_CR_RXE: u32 = 1 << 9;
const UART_LCRH_FEN: u32 = 1 << 4;
const UART_LCRH_WLEN_8: u32 = 3 << 5;
const UART_LCRH_BRK: u32 = 1 << 0;
const UART_CR_ENABLE_MASK: u32 = UART_CR_UARTEN | UART_CR_TXE | UART_CR_RXE;
const UART_INT_RX: u32 = 1 << 4;
const UART_INT_RT: u32 = 1 << 6;
const UART_INT_RX_MASK: u32 = UART_INT_RX | UART_INT_RT;
const UART0_IRQ_PAYLOAD_MAX: usize = 32;
const UART0_IRQ_ENTRY_MAX: u32 = 32;

pub const UART0_IRQ_DECISION_PENDING: u32 = 0;
pub const UART0_IRQ_DECISION_COMPLETE: u32 = 1;
pub const UART0_IRQ_DECISION_DATA_ERROR: u32 = 2;
pub const UART0_IRQ_DECISION_OVERFLOW: u32 = 3;
pub const UART0_IRQ_DECISION_SPURIOUS: u32 = 4;
pub const UART0_IRQ_DECISION_STORM: u32 = 5;

const GPIO_CTRL_FUNCSEL_MASK: u32 = 0x1f;
const GPIO_CTRL_OUTOVER_MASK: u32 = 0x0000_3000;
const GPIO_CTRL_OEOVER_MASK: u32 = 0x0000_c000;
const GPIO_CTRL_INOVER_MASK: u32 = 0x0003_0000;
const GPIO_CTRL_OVERRIDE_MASK: u32 =
    GPIO_CTRL_OUTOVER_MASK | GPIO_CTRL_OEOVER_MASK | GPIO_CTRL_INOVER_MASK;
const GPIO_CTRL_FUNCSEL_UART0: u32 = 4;
const PAD_OD: u32 = 1 << 7;
const PAD_IE: u32 = 1 << 6;
const PAD_SCHMITT: u32 = 1 << 1;
const PAD_PULL_SHIFT: u32 = 2;
const PAD_PULL_MASK: u32 = 0b11 << PAD_PULL_SHIFT;
const PAD_PULL_UP: u32 = 2 << PAD_PULL_SHIFT;
const PAD_RX_MASK: u32 = PAD_SCHMITT | PAD_PULL_MASK | PAD_IE;

const UART0_TX_PIN: u8 = 14;
const UART0_RX_PIN: u8 = 15;
const UART0_IBRD_115200_50MHZ: u32 = 27;
const UART0_FBRD_115200_50MHZ: u32 = 8;
const UART_BUSY_TIMEOUT: u32 = 0x10000;
const UART_TX_TIMEOUT: u32 = 0x10000;
const UART_RX_TIMEOUT: u32 = 0x0100_0000;

pub struct Uart0 {
    _private: (),
}

pub struct Uart0Tx {
    first_byte_pending: bool,
    first_buffer_pending: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Uart0RxError {
    Timeout,
    DataError(u8),
}

#[derive(Clone, Copy)]
pub struct Uart0Snapshot {
    pub rsr: u32,
    pub fr: u32,
    pub ibrd: u32,
    pub fbrd: u32,
    pub lcr_h: u32,
    pub cr: u32,
    pub imsc: u32,
}

#[derive(Clone, Copy)]
pub struct Uart0IrqSnapshot {
    pub decision: u32,
    pub byte_count: u32,
    pub irq_count: u32,
    pub ipsr: u32,
    pub first_ris: u32,
    pub first_mis: u32,
    pub final_ris: u32,
    pub final_mis: u32,
    pub rsr: u32,
    pub final_imsc: u32,
    pub payload: [u8; UART0_IRQ_PAYLOAD_MAX],
}

static UART0_IRQ_DECISION: AtomicU32 = AtomicU32::new(UART0_IRQ_DECISION_PENDING);
static UART0_IRQ_EXPECTED_LEN: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_BYTE_COUNT: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_COUNT: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_IPSR: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_FIRST_RIS: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_FIRST_MIS: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_FINAL_RIS: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_FINAL_MIS: AtomicU32 = AtomicU32::new(0);
static UART0_IRQ_RSR: AtomicU32 = AtomicU32::new(0);
static mut UART0_IRQ_PAYLOAD: [u8; UART0_IRQ_PAYLOAD_MAX] = [0; UART0_IRQ_PAYLOAD_MAX];

impl Uart0Snapshot {
    const ZERO: Self = Self {
        rsr: 0,
        fr: 0,
        ibrd: 0,
        fbrd: 0,
        lcr_h: 0,
        cr: 0,
        imsc: 0,
    };

    pub fn encoded_bytes(self) -> [u8; 9] {
        [
            self.cr as u8,
            (self.cr >> 8) as u8,
            self.lcr_h as u8,
            self.ibrd as u8,
            self.fbrd as u8,
            self.fr as u8,
            self.imsc as u8,
            (self.imsc >> 8) as u8,
            self.rsr as u8,
        ]
    }
}

const UART0_SNAPSHOT_COUNT: usize = 11;
static mut UART0_SNAPSHOTS: [Uart0Snapshot; UART0_SNAPSHOT_COUNT] =
    [Uart0Snapshot::ZERO; UART0_SNAPSHOT_COUNT];

impl Uart0 {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    pub fn init_115200(self) -> Uart0Tx {
        self.init_115200_inner(true, true)
    }

    pub fn init_tx_115200_clock_ready(self) -> Uart0Tx {
        self.init_115200_inner(false, false)
    }

    pub fn init_tx_rx_115200_clock_ready(self) -> Uart0Tx {
        self.init_115200_inner(false, true)
    }

    fn init_115200_inner(self, configure_clock: bool, enable_rx: bool) -> Uart0Tx {
        if configure_clock {
            configure_uart0_clock_bootmain_50mhz();
        }
        record_snapshot(1);

        configure_uart0_pinmux_rpi_boot_parity();
        pinmux_init_boundary();

        record_snapshot(2);
        wait_for_uart0_not_busy();
        let control = reg(UART0_BASE + UART_CR).read();
        write_readback(UART0_BASE + UART_CR, control & !UART_CR_ENABLE_MASK);
        record_snapshot(3);
        let line_control = reg(UART0_BASE + UART_LCRH).read();
        write_readback(UART0_BASE + UART_LCRH, line_control & !UART_LCRH_BRK);
        write_readback(UART0_BASE + UART_IMSC, 0);
        write_barrier(UART0_BASE + UART_ICR, 0x7ff);
        write_readback(UART0_BASE + UART_RSR_ECR, 0);
        record_snapshot(4);
        write_readback(UART0_BASE + UART_IBRD, UART0_IBRD_115200_50MHZ);
        write_readback(UART0_BASE + UART_FBRD, UART0_FBRD_115200_50MHZ);
        record_snapshot(5);
        write_readback(UART0_BASE + UART_LCRH, UART_LCRH_WLEN_8 | UART_LCRH_FEN);
        record_snapshot(6);
        let mut enable = UART_CR_UARTEN | UART_CR_TXE;
        if enable_rx {
            enable |= UART_CR_RXE;
        }
        write_readback(UART0_BASE + UART_CR, enable);
        read_barrier(UART0_BASE + UART_IBRD);
        read_barrier(UART0_BASE + UART_FBRD);
        read_barrier(UART0_BASE + UART_LCRH);
        read_barrier(UART0_BASE + UART_CR);
        read_barrier(UART0_BASE + UART_FR);
        record_snapshot(7);

        Uart0Tx {
            first_byte_pending: true,
            first_buffer_pending: true,
        }
    }
}

fn wait_for_uart0_not_busy() {
    if reg(UART0_BASE + UART_CR).read() & UART_CR_UARTEN == 0 {
        return;
    }

    let mut timeout = UART_BUSY_TIMEOUT;
    while reg(UART0_BASE + UART_FR).read() & UART_FR_BUSY != 0 && timeout != 0 {
        timeout -= 1;
        core::hint::spin_loop();
    }
}

fn configure_uart0_clock_bootmain_50mhz() {
    write_readback(CLOCKS_MAIN_BASE + CLK_UART_DIV_INT, 1);
    write_readback(CLOCKS_MAIN_BASE + CLK_UART_CTRL, CLK_UART_CTRL_XOSC);
    write_readback(
        CLOCKS_MAIN_BASE + CLK_UART_CTRL,
        CLK_UART_CTRL_XOSC | CLK_UART_CTRL_ENABLE,
    );
}

fn configure_uart0_pinmux_rpi_boot_parity() {
    for pin in [UART0_TX_PIN, UART0_RX_PIN] {
        let address = gpio_ctrl_addr(pin);
        let control = reg(address).read();
        write_readback(address, uart0_ctrl_value(control));
    }

    let tx_pad_address = gpio_pad_addr(UART0_TX_PIN);
    let tx_pad = reg(tx_pad_address).read();
    write_readback(tx_pad_address, uart0_tx_pad_value(tx_pad));

    let rx_pad_address = gpio_pad_addr(UART0_RX_PIN);
    let rx_pad = reg(rx_pad_address).read();
    write_readback(rx_pad_address, uart0_rx_pad_value(rx_pad));
}

#[inline(always)]
fn uart0_ctrl_value(current: u32) -> u32 {
    (current & !(GPIO_CTRL_FUNCSEL_MASK | GPIO_CTRL_OVERRIDE_MASK)) | GPIO_CTRL_FUNCSEL_UART0
}

#[inline(always)]
fn uart0_tx_pad_value(current: u32) -> u32 {
    current & !(PAD_PULL_MASK | PAD_OD)
}

#[inline(always)]
fn uart0_rx_pad_value(current: u32) -> u32 {
    (current & !PAD_RX_MASK) | PAD_SCHMITT | PAD_PULL_UP | PAD_IE
}

#[inline(always)]
fn pinmux_init_boundary() {
    dsb_sy();
    isb();
}

impl Uart0Tx {
    pub fn write_byte(&mut self, byte: u8) -> bool {
        if self.first_byte_pending {
            record_snapshot(8);
        }
        let mut timeout = UART_TX_TIMEOUT;
        while reg(UART0_BASE + UART_FR).read() & UART_FR_TXFF != 0 {
            if timeout == 0 {
                return false;
            }
            timeout -= 1;
            core::hint::spin_loop();
        }
        reg(UART0_BASE + UART_DR).write(byte as u32);
        if self.first_byte_pending {
            record_snapshot(9);
            self.first_byte_pending = false;
        }
        true
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> usize {
        let mut written = 0;
        for byte in bytes {
            if !self.write_byte(*byte) {
                break;
            }
            written += 1;
        }
        if self.first_buffer_pending {
            record_snapshot(10);
            self.first_buffer_pending = false;
        }
        written
    }

    pub fn read_byte(&mut self) -> Result<u8, Uart0RxError> {
        let mut timeout = UART_RX_TIMEOUT;
        while reg(UART0_BASE + UART_FR).read() & UART_FR_RXFE != 0 {
            if timeout == 0 {
                return Err(Uart0RxError::Timeout);
            }
            timeout -= 1;
            core::hint::spin_loop();
        }

        let data = reg(UART0_BASE + UART_DR).read();
        let errors = ((data & UART_DR_ERROR_MASK) >> 8) as u8;
        if errors != 0 {
            return Err(Uart0RxError::DataError(errors));
        }
        Ok((data & UART_DR_DATA_MASK) as u8)
    }

    pub fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, Uart0RxError> {
        let mut received = 0;
        for byte in buffer {
            *byte = self.read_byte()?;
            received += 1;
        }
        Ok(received)
    }

    pub fn enable_rx_interrupt(&mut self, expected_len: usize) -> bool {
        if expected_len == 0 || expected_len > UART0_IRQ_PAYLOAD_MAX {
            return false;
        }

        reset_uart0_irq_state(expected_len);
        write_readback(UART0_BASE + UART_IMSC, 0);
        write_barrier(UART0_BASE + UART_ICR, 0x7ff);
        write_readback(UART0_BASE + UART_IFLS, 0);
        write_readback(UART0_BASE + UART_IMSC, UART_INT_RX_MASK);

        reg(UART0_BASE + UART_IFLS).read() == 0
            && reg(UART0_BASE + UART_IMSC).read() & UART_INT_RX_MASK == UART_INT_RX_MASK
            && reg(UART0_BASE + UART_CR).read() & (UART_CR_UARTEN | UART_CR_RXE)
                == UART_CR_UARTEN | UART_CR_RXE
    }

    pub fn mask_and_clear_rx_interrupt(&mut self) {
        mask_uart0_rx_interrupt();
        write_barrier(UART0_BASE + UART_ICR, UART_INT_RX_MASK);
    }

    pub unsafe fn service_rx_interrupt() {
        let entry = UART0_IRQ_COUNT.load(Ordering::Relaxed).wrapping_add(1);
        UART0_IRQ_COUNT.store(entry, Ordering::Relaxed);
        let ipsr = read_ipsr();
        let ris = reg(UART0_BASE + UART_RIS).read();
        let mis = reg(UART0_BASE + UART_MIS).read();

        if entry == 1 {
            UART0_IRQ_IPSR.store(ipsr, Ordering::Relaxed);
            UART0_IRQ_FIRST_RIS.store(ris, Ordering::Relaxed);
            UART0_IRQ_FIRST_MIS.store(mis, Ordering::Relaxed);
        }

        if entry > UART0_IRQ_ENTRY_MAX {
            finish_uart0_irq(UART0_IRQ_DECISION_STORM);
            return;
        }
        if mis & UART_INT_RX_MASK == 0 {
            finish_uart0_irq(UART0_IRQ_DECISION_SPURIOUS);
            return;
        }

        let expected = UART0_IRQ_EXPECTED_LEN.load(Ordering::Relaxed) as usize;
        let mut count = UART0_IRQ_BYTE_COUNT.load(Ordering::Relaxed) as usize;
        let mut terminal = UART0_IRQ_DECISION_PENDING;

        while reg(UART0_BASE + UART_FR).read() & UART_FR_RXFE == 0 {
            let data = reg(UART0_BASE + UART_DR).read();
            let errors = (data & UART_DR_ERROR_MASK) >> 8;
            if errors != 0 {
                UART0_IRQ_RSR.store(errors, Ordering::Relaxed);
                terminal = UART0_IRQ_DECISION_DATA_ERROR;
                break;
            }
            if count >= expected || count >= UART0_IRQ_PAYLOAD_MAX {
                terminal = UART0_IRQ_DECISION_OVERFLOW;
                break;
            }
            unsafe {
                core::ptr::addr_of_mut!(UART0_IRQ_PAYLOAD)
                    .cast::<u8>()
                    .add(count)
                    .write_volatile((data & UART_DR_DATA_MASK) as u8);
            }
            count += 1;
            UART0_IRQ_BYTE_COUNT.store(count as u32, Ordering::Relaxed);
        }

        if terminal == UART0_IRQ_DECISION_PENDING && count == expected {
            terminal = if reg(UART0_BASE + UART_FR).read() & UART_FR_RXFE == 0 {
                UART0_IRQ_DECISION_OVERFLOW
            } else {
                UART0_IRQ_DECISION_COMPLETE
            };
        }

        if terminal != UART0_IRQ_DECISION_PENDING {
            finish_uart0_irq(terminal);
        } else {
            write_barrier(UART0_BASE + UART_ICR, UART_INT_RX_MASK);
            UART0_IRQ_FINAL_RIS.store(reg(UART0_BASE + UART_RIS).read(), Ordering::Relaxed);
            UART0_IRQ_FINAL_MIS.store(reg(UART0_BASE + UART_MIS).read(), Ordering::Relaxed);
        }
    }

    pub fn rx_interrupt_snapshot() -> Uart0IrqSnapshot {
        let decision = UART0_IRQ_DECISION.load(Ordering::Acquire);
        let mut payload = [0u8; UART0_IRQ_PAYLOAD_MAX];
        if decision != UART0_IRQ_DECISION_PENDING {
            for (index, byte) in payload.iter_mut().enumerate() {
                unsafe {
                    *byte = core::ptr::addr_of!(UART0_IRQ_PAYLOAD)
                        .cast::<u8>()
                        .add(index)
                        .read_volatile();
                }
            }
        }

        Uart0IrqSnapshot {
            decision,
            byte_count: UART0_IRQ_BYTE_COUNT.load(Ordering::Relaxed),
            irq_count: UART0_IRQ_COUNT.load(Ordering::Relaxed),
            ipsr: UART0_IRQ_IPSR.load(Ordering::Relaxed),
            first_ris: UART0_IRQ_FIRST_RIS.load(Ordering::Relaxed),
            first_mis: UART0_IRQ_FIRST_MIS.load(Ordering::Relaxed),
            final_ris: UART0_IRQ_FINAL_RIS.load(Ordering::Relaxed),
            final_mis: UART0_IRQ_FINAL_MIS.load(Ordering::Relaxed),
            rsr: UART0_IRQ_RSR.load(Ordering::Relaxed),
            final_imsc: reg(UART0_BASE + UART_IMSC).read(),
            payload,
        }
    }

    pub fn readback_snapshot(&self, index: usize) -> Uart0Snapshot {
        assert!(index < UART0_SNAPSHOT_COUNT);
        // RP1 proc0 is the sole writer/reader during this single-core proof.
        unsafe { UART0_SNAPSHOTS[index] }
    }
}

fn reset_uart0_irq_state(expected_len: usize) {
    UART0_IRQ_DECISION.store(UART0_IRQ_DECISION_PENDING, Ordering::Relaxed);
    UART0_IRQ_EXPECTED_LEN.store(expected_len as u32, Ordering::Relaxed);
    UART0_IRQ_BYTE_COUNT.store(0, Ordering::Relaxed);
    UART0_IRQ_COUNT.store(0, Ordering::Relaxed);
    UART0_IRQ_IPSR.store(0, Ordering::Relaxed);
    UART0_IRQ_FIRST_RIS.store(0, Ordering::Relaxed);
    UART0_IRQ_FIRST_MIS.store(0, Ordering::Relaxed);
    UART0_IRQ_FINAL_RIS.store(0, Ordering::Relaxed);
    UART0_IRQ_FINAL_MIS.store(0, Ordering::Relaxed);
    UART0_IRQ_RSR.store(0, Ordering::Relaxed);
    for index in 0..UART0_IRQ_PAYLOAD_MAX {
        unsafe {
            core::ptr::addr_of_mut!(UART0_IRQ_PAYLOAD)
                .cast::<u8>()
                .add(index)
                .write_volatile(0);
        }
    }
    dsb_sy();
}

fn finish_uart0_irq(decision: u32) {
    mask_uart0_rx_interrupt();
    write_barrier(UART0_BASE + UART_ICR, UART_INT_RX_MASK);
    UART0_IRQ_FINAL_RIS.store(reg(UART0_BASE + UART_RIS).read(), Ordering::Relaxed);
    UART0_IRQ_FINAL_MIS.store(reg(UART0_BASE + UART_MIS).read(), Ordering::Relaxed);
    UART0_IRQ_DECISION.store(decision, Ordering::Release);
}

fn mask_uart0_rx_interrupt() {
    let imsc = reg(UART0_BASE + UART_IMSC).read();
    write_readback(UART0_BASE + UART_IMSC, imsc & !UART_INT_RX_MASK);
}

#[inline(always)]
fn read_ipsr() -> u32 {
    #[cfg(target_arch = "arm")]
    unsafe {
        let value: u32;
        core::arch::asm!("mrs {}, IPSR", out(reg) value, options(nomem, nostack, preserves_flags));
        value
    }

    #[cfg(not(target_arch = "arm"))]
    0
}

fn record_snapshot(index: usize) {
    let snapshot = Uart0Snapshot {
        rsr: reg(UART0_BASE + UART_RSR_ECR).read(),
        fr: reg(UART0_BASE + UART_FR).read(),
        ibrd: reg(UART0_BASE + UART_IBRD).read(),
        fbrd: reg(UART0_BASE + UART_FBRD).read(),
        lcr_h: reg(UART0_BASE + UART_LCRH).read(),
        cr: reg(UART0_BASE + UART_CR).read(),
        imsc: reg(UART0_BASE + UART_IMSC).read(),
    };
    // RP1 proc0 is the sole writer/reader during this single-core proof.
    unsafe {
        UART0_SNAPSHOTS[index] = snapshot;
    }
}

#[inline(always)]
fn reg(addr: usize) -> Reg<u32> {
    unsafe { Reg::new(addr) }
}

#[inline(always)]
fn write_readback(addr: usize, value: u32) {
    let register = reg(addr);
    register.write(value);
    let _ = register.read();
    dsb_sy();
}

#[inline(always)]
fn write_barrier(addr: usize, value: u32) {
    reg(addr).write(value);
    dsb_sy();
}

#[inline(always)]
fn read_barrier(addr: usize) {
    let _ = reg(addr).read();
    dsb_sy();
}

#[inline(always)]
fn dsb_sy() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }

    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

#[inline(always)]
fn isb() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("isb", options(nostack, preserves_flags));
    }

    #[cfg(not(target_arch = "arm"))]
    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
}

const fn gpio_ctrl_addr(n: u8) -> usize {
    IO_BANK0_BASE + 0x04 + (n as usize) * 8
}

const fn gpio_pad_addr(n: u8) -> usize {
    PADS_BANK0_BASE + 0x04 + (n as usize) * 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uart0_gpio_addresses_match_direct_uart_test() {
        assert_eq!(gpio_ctrl_addr(14), 0x400d_0074);
        assert_eq!(gpio_ctrl_addr(15), 0x400d_007c);
        assert_eq!(gpio_pad_addr(14), 0x400f_003c);
        assert_eq!(gpio_pad_addr(15), 0x400f_0040);
    }

    #[test]
    fn uart0_pinmux_matches_rpi_boot_rmw_discipline() {
        let preserved = 0xa5a4_0be0;
        let control = uart0_ctrl_value(preserved | GPIO_CTRL_OVERRIDE_MASK | 0x1f);
        assert_eq!(control & GPIO_CTRL_FUNCSEL_MASK, GPIO_CTRL_FUNCSEL_UART0);
        assert_eq!(control & GPIO_CTRL_OVERRIDE_MASK, 0);
        assert_eq!(
            control & !(GPIO_CTRL_FUNCSEL_MASK | GPIO_CTRL_OVERRIDE_MASK),
            preserved
        );

        let tx_pad = uart0_tx_pad_value(0xffff_ffff);
        assert_eq!(tx_pad & (PAD_PULL_MASK | PAD_OD), 0);
        assert_eq!(
            tx_pad & !(PAD_PULL_MASK | PAD_OD),
            !(PAD_PULL_MASK | PAD_OD)
        );

        let rx_pad = uart0_rx_pad_value(0xffff_ffff);
        assert_eq!(rx_pad & PAD_IE, PAD_IE);
        assert_eq!(rx_pad & PAD_SCHMITT, PAD_SCHMITT);
        assert_eq!(rx_pad & PAD_PULL_MASK, PAD_PULL_UP);
        assert_eq!(rx_pad & !PAD_RX_MASK, !PAD_RX_MASK);
    }

    #[test]
    fn uart0_clock_registers_match_bootmain_sequence() {
        assert_eq!(CLOCKS_MAIN_BASE + CLK_UART_CTRL, 0x4001_8054);
        assert_eq!(CLOCKS_MAIN_BASE + CLK_UART_DIV_INT, 0x4001_8058);
        assert_eq!(CLK_UART_CTRL_XOSC, 0x40);
        assert_eq!(CLK_UART_CTRL_XOSC | CLK_UART_CTRL_ENABLE, 0x840);
    }

    #[test]
    fn uart0_baud_divisors_match_50mhz_115200_contract() {
        assert_eq!(UART0_IBRD_115200_50MHZ, 27);
        assert_eq!(UART0_FBRD_115200_50MHZ, 8);
    }

    #[test]
    fn uart0_reinitialization_registers_match_pl011() {
        assert_eq!(UART_RSR_ECR, 0x04);
        assert_eq!(UART_IMSC, 0x38);
        assert_eq!(UART_ICR, 0x44);
        assert_eq!(UART_FR_BUSY, 1 << 3);
        assert_eq!(UART_LCRH_BRK, 1);
        assert_eq!(UART_CR_ENABLE_MASK, 0x301);
    }
}

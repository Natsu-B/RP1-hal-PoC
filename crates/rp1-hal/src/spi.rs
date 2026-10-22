use crate::addr::SPI0_BASE;
use crate::gpio::{ConfiguredPin, Function, Pin, configure_spi_cs_pin, configure_spi_data_pin};
use crate::mmio::Reg;

mod receive;
pub use receive::{Spi0IrqTransfer, Spi0RxError, Spi0RxState};

const CTRLR0: usize = 0x00;
const SSIENR: usize = 0x08;
const SER: usize = 0x10;
const BAUDR: usize = 0x14;
const TXFTLR: usize = 0x18;
const RXFTLR: usize = 0x1c;
const TXFLR: usize = 0x20;
const RXFLR: usize = 0x24;
const SR: usize = 0x28;
const IMR: usize = 0x2c;
const ISR: usize = 0x30;
const RISR: usize = 0x34;
const DMACR: usize = 0x4c;
const IDR: usize = 0x58;
const VERSION: usize = 0x5c;
const DR: usize = 0x60;
const RX_SAMPLE_DLY: usize = 0xf0;
const CS_OVERRIDE: usize = 0xf4;

const VERSION_4_02: u32 = 0x3430_322a;
const CTRLR0_DFS_8BIT_TX_ONLY_MODE0: u32 = (7 << 16) | (1 << 8);
const SSI_ENABLE: u32 = 1;
const IRQ_TXEI: u32 = 1 << 0;
const SER_CS0: u32 = 1;
const SR_BUSY: u32 = 1 << 0;
const SR_TX_NOT_FULL: u32 = 1 << 1;
const SR_TX_EMPTY: u32 = 1 << 2;
const BAUD_DIV_100KHZ_AT_200MHZ: u32 = 2_000;
const FIFO_DEPTH_LIMIT: u16 = 256;
const CONTROL_POLL_LIMIT: u32 = 100_000;
const TRANSFER_POLL_LIMIT: u32 = 10_000_000;

pub struct Spi0 {
    _private: (),
}

pub struct Spi0Host {
    _cs0: ConfiguredPin<8, Function<0>>,
    _miso: ConfiguredPin<9, Function<0>>,
    _mosi: ConfiguredPin<10, Function<0>>,
    _sclk: ConfiguredPin<11, Function<0>>,
    fifo_depth: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Spi0Error {
    Version(u32),
    DisableTimeout,
    EnableTimeout,
    FifoDepthUnknown,
    EmptyPayload,
    PayloadTooLong { len: usize, fifo_depth: u16 },
    TxFifoTimeout,
    TransferTimeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spi0Snapshot {
    pub version: u32,
    pub id: u32,
    pub control: u32,
    pub enable: u32,
    pub baud_divisor: u32,
    pub status: u32,
    pub tx_fifo_level: u32,
    pub rx_fifo_level: u32,
    pub rx_sample_delay: u32,
    pub cs_override: u32,
    pub fifo_depth: u16,
    pub bytes_queued: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spi0IrqSnapshot {
    pub version: u32,
    pub enable: u32,
    pub tx_fifo_threshold: u32,
    pub interrupt_mask: u32,
    pub raw_interrupt_status: u32,
    pub masked_interrupt_status: u32,
    pub tx_fifo_level: u32,
    pub status: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spi0IrqSourceSnapshot {
    pub raw_interrupt_status: u32,
    pub masked_interrupt_status: u32,
}

impl Spi0 {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    pub fn into_host_mode0_100khz(
        self,
        cs0: Pin<8>,
        miso: Pin<9>,
        mosi: Pin<10>,
        sclk: Pin<11>,
    ) -> Result<Spi0Host, Spi0Error> {
        let version = reg(VERSION).read();
        if version != VERSION_4_02 {
            return Err(Spi0Error::Version(version));
        }

        disable()?;
        reg(SER).write(0);
        let fifo_depth = detect_fifo_depth()?;
        reg(CTRLR0).write(CTRLR0_DFS_8BIT_TX_ONLY_MODE0);
        reg(BAUDR).write(BAUD_DIV_100KHZ_AT_200MHZ);
        reg(TXFTLR).write(0);
        reg(RXFTLR).write(0);
        reg(IMR).write(0);
        reg(DMACR).write(0);

        let cs0 = cs0.into_function::<0>();
        let miso = miso.into_function::<0>();
        let mosi = mosi.into_function::<0>();
        let sclk = sclk.into_function::<0>();
        configure_spi_cs_pin::<8>();
        configure_spi_data_pin::<9>();
        configure_spi_data_pin::<10>();
        configure_spi_data_pin::<11>();

        Ok(Spi0Host {
            _cs0: cs0,
            _miso: miso,
            _mosi: mosi,
            _sclk: sclk,
            fifo_depth,
        })
    }

    #[cfg(target_arch = "arm")]
    pub fn prepare_tx_empty_irq(&mut self) -> Result<Spi0IrqSnapshot, Spi0Error> {
        let version = reg(VERSION).read();
        if version != VERSION_4_02 {
            return Err(Spi0Error::Version(version));
        }

        disable()?;
        reg(SER).write(0);
        reg(CTRLR0).write(CTRLR0_DFS_8BIT_TX_ONLY_MODE0);
        reg(BAUDR).write(BAUD_DIV_100KHZ_AT_200MHZ);
        reg(TXFTLR).write(0);
        reg(RXFTLR).write(0);
        reg(IMR).write(0);
        reg(DMACR).write(0);
        reg(SSIENR).write(SSI_ENABLE);
        if !poll_until(|| reg(SSIENR).read() & SSI_ENABLE != 0, CONTROL_POLL_LIMIT) {
            stop();
            return Err(Spi0Error::EnableTimeout);
        }
        Ok(spi0_irq_snapshot())
    }
}

impl Spi0Host {
    pub fn write(&mut self, payload: &[u8]) -> Result<Spi0Snapshot, Spi0Error> {
        if payload.is_empty() {
            return Err(Spi0Error::EmptyPayload);
        }
        if payload.len() > usize::from(self.fifo_depth) {
            return Err(Spi0Error::PayloadTooLong {
                len: payload.len(),
                fifo_depth: self.fifo_depth,
            });
        }

        disable()?;
        reg(SER).write(0);
        reg(CTRLR0).write(CTRLR0_DFS_8BIT_TX_ONLY_MODE0);
        reg(BAUDR).write(BAUD_DIV_100KHZ_AT_200MHZ);
        reg(SSIENR).write(SSI_ENABLE);
        if !poll_until(|| reg(SSIENR).read() & SSI_ENABLE != 0, CONTROL_POLL_LIMIT) {
            stop();
            return Err(Spi0Error::EnableTimeout);
        }

        for byte in payload {
            if !poll_until(|| reg(SR).read() & SR_TX_NOT_FULL != 0, CONTROL_POLL_LIMIT) {
                stop();
                return Err(Spi0Error::TxFifoTimeout);
            }
            reg(DR).write(u32::from(*byte));
        }

        reg(SER).write(SER_CS0);
        if !poll_until(
            || {
                let status = reg(SR).read();
                status & SR_BUSY == 0 && status & SR_TX_EMPTY != 0 && reg(TXFLR).read() == 0
            },
            TRANSFER_POLL_LIMIT,
        ) {
            stop();
            return Err(Spi0Error::TransferTimeout);
        }

        reg(SER).write(0);
        let snapshot = Spi0Snapshot {
            version: reg(VERSION).read(),
            id: reg(IDR).read(),
            control: reg(CTRLR0).read(),
            enable: reg(SSIENR).read(),
            baud_divisor: reg(BAUDR).read(),
            status: reg(SR).read(),
            tx_fifo_level: reg(TXFLR).read(),
            rx_fifo_level: reg(RXFLR).read(),
            rx_sample_delay: reg(RX_SAMPLE_DLY).read(),
            cs_override: reg(CS_OVERRIDE).read(),
            fifo_depth: self.fifo_depth,
            bytes_queued: payload.len() as u16,
        };
        disable()?;
        Ok(snapshot)
    }
}

fn detect_fifo_depth() -> Result<u16, Spi0Error> {
    for depth in 1..FIFO_DEPTH_LIMIT {
        reg(TXFTLR).write(u32::from(depth));
        if reg(TXFTLR).read() != u32::from(depth) {
            reg(TXFTLR).write(0);
            return Ok(depth);
        }
    }
    reg(TXFTLR).write(0);
    Err(Spi0Error::FifoDepthUnknown)
}

fn disable() -> Result<(), Spi0Error> {
    reg(SSIENR).write(0);
    if poll_until(|| reg(SSIENR).read() & SSI_ENABLE == 0, CONTROL_POLL_LIMIT) {
        Ok(())
    } else {
        Err(Spi0Error::DisableTimeout)
    }
}

fn stop() {
    reg(SER).write(0);
    reg(SSIENR).write(0);
}

#[cfg(target_arch = "arm")]
pub fn spi0_irq_snapshot() -> Spi0IrqSnapshot {
    Spi0IrqSnapshot {
        version: reg(VERSION).read(),
        enable: reg(SSIENR).read(),
        tx_fifo_threshold: reg(TXFTLR).read(),
        interrupt_mask: reg(IMR).read(),
        raw_interrupt_status: reg(RISR).read(),
        masked_interrupt_status: reg(ISR).read(),
        tx_fifo_level: reg(TXFLR).read(),
        status: reg(SR).read(),
    }
}

#[cfg(target_arch = "arm")]
pub fn spi0_irq_source_snapshot() -> Spi0IrqSourceSnapshot {
    Spi0IrqSourceSnapshot {
        raw_interrupt_status: reg(RISR).read(),
        masked_interrupt_status: reg(ISR).read(),
    }
}

#[cfg(target_arch = "arm")]
pub fn spi0_unmask_tx_empty_irq() -> Spi0IrqSnapshot {
    reg(IMR).write(IRQ_TXEI);
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    spi0_irq_snapshot()
}

#[cfg(target_arch = "arm")]
pub fn spi0_mask_tx_empty_irq() {
    reg(IMR).write(0);
}

#[cfg(target_arch = "arm")]
pub fn spi0_cleanup_tx_empty_irq() {
    reg(IMR).write(0);
    reg(SSIENR).write(0);
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn poll_until(mut condition: impl FnMut() -> bool, limit: u32) -> bool {
    for _ in 0..limit {
        if condition() {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

#[inline(always)]
fn reg(offset: usize) -> Reg<u32> {
    unsafe { Reg::new(SPI0_BASE + offset) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_map_matches_rp1_designware_spi0() {
        assert_eq!(SPI0_BASE, 0x4005_0000);
        assert_eq!(SPI0_BASE + CTRLR0, 0x4005_0000);
        assert_eq!(SPI0_BASE + SR, 0x4005_0028);
        assert_eq!(SPI0_BASE + IMR, 0x4005_002c);
        assert_eq!(SPI0_BASE + ISR, 0x4005_0030);
        assert_eq!(SPI0_BASE + RISR, 0x4005_0034);
        assert_eq!(SPI0_BASE + VERSION, 0x4005_005c);
        assert_eq!(SPI0_BASE + DR, 0x4005_0060);
        assert_eq!(SPI0_BASE + CS_OVERRIDE, 0x4005_00f4);
    }

    #[test]
    fn mode0_transmit_only_control_uses_eight_bit_frames() {
        assert_eq!(CTRLR0_DFS_8BIT_TX_ONLY_MODE0, 0x0007_0100);
        assert_eq!(CTRLR0_DFS_8BIT_TX_ONLY_MODE0 & 0x300, 0x100);
        assert_eq!(CTRLR0_DFS_8BIT_TX_ONLY_MODE0 & 0xc0, 0);
    }

    #[test]
    fn baud_divisor_is_even_and_targets_100khz_from_200mhz() {
        assert_eq!(BAUD_DIV_100KHZ_AT_200MHZ, 2_000);
        assert_eq!(BAUD_DIV_100KHZ_AT_200MHZ & 1, 0);
        assert_eq!(200_000_000 / BAUD_DIV_100KHZ_AT_200MHZ, 100_000);
    }

    #[test]
    fn status_masks_match_designware_contract() {
        assert_eq!(SR_BUSY, 1);
        assert_eq!(SR_TX_NOT_FULL, 2);
        assert_eq!(SR_TX_EMPTY, 4);
    }

    #[test]
    fn tx_empty_irq_uses_only_designware_bit0() {
        assert_eq!(IRQ_TXEI, 1);
    }

    #[test]
    fn interrupt_source_offsets_are_adjacent_after_mask() {
        assert_eq!(ISR, IMR + 4);
        assert_eq!(RISR, ISR + 4);
    }
}

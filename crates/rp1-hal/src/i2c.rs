use crate::addr::I2C1_BASE;
use crate::gpio::{ConfiguredPin, Function, Pin, configure_i2c_pin};
use crate::mmio::Reg;

const IC_CON: usize = 0x00;
const IC_TAR: usize = 0x04;
const IC_DATA_CMD: usize = 0x10;
const IC_SS_SCL_HCNT: usize = 0x14;
const IC_SS_SCL_LCNT: usize = 0x18;
const IC_INTR_MASK: usize = 0x30;
const IC_RAW_INTR_STAT: usize = 0x34;
const IC_RX_TL: usize = 0x38;
const IC_TX_TL: usize = 0x3c;
const IC_CLR_INTR: usize = 0x40;
const IC_CLR_TX_ABRT: usize = 0x54;
const IC_CLR_STOP_DET: usize = 0x60;
const IC_ENABLE: usize = 0x6c;
const IC_STATUS: usize = 0x70;
const IC_TXFLR: usize = 0x74;
const IC_SDA_HOLD: usize = 0x7c;
const IC_TX_ABRT_SOURCE: usize = 0x80;
const IC_ENABLE_STATUS: usize = 0x9c;
const IC_COMP_PARAM_1: usize = 0xf4;
const IC_COMP_TYPE: usize = 0xfc;

const IC_COMP_TYPE_VALUE: u32 = 0x4457_0140;
const IC_CON_MASTER_STD_RESTART: u32 = (1 << 0) | (1 << 1) | (1 << 5) | (1 << 6);
const IC_DATA_CMD_STOP: u32 = 1 << 9;
const IC_INTR_TX_ABRT: u32 = 1 << 6;
const IC_INTR_STOP_DET: u32 = 1 << 9;
const IC_ENABLE_ENABLE: u32 = 1 << 0;

// Linux caps clk_sys at 200 MHz. These counts therefore keep SCL at or below
// 100 kHz for every supported clk_sys rate, using RP1's 65/100 ns DT timings.
const IC_SS_HCNT_200MHZ: u32 = 781;
const IC_SS_LCNT_200MHZ: u32 = 1_186;
const IC_SDA_HOLD_200MHZ: u32 = IC_SS_LCNT_200MHZ / 2;

const CONTROL_POLL_LIMIT: u32 = 100_000;
const TRANSFER_POLL_LIMIT: u32 = 5_000_000;

pub struct I2c1 {
    _private: (),
}

pub struct I2c1Host {
    _sda: ConfiguredPin<2, Function<3>>,
    _scl: ConfiguredPin<3, Function<3>>,
    tx_fifo_depth: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum I2c1Error {
    ComponentType(u32),
    FifoTooShallow(u16),
    DisableTimeout,
    EnableTimeout,
    InvalidAddress(u8),
    EmptyPayload,
    PayloadTooLong { len: usize, fifo_depth: u16 },
    TxFifoTimeout,
    TxAbort(u32),
    StopTimeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct I2c1Snapshot {
    pub component_type: u32,
    pub component_parameter: u32,
    pub control: u32,
    pub target: u32,
    pub enable_status: u32,
    pub status: u32,
    pub raw_interrupt_status: u32,
    pub abort_source: u32,
    pub tx_fifo_depth: u16,
    pub bytes_queued: u16,
}

impl I2c1 {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    pub fn into_host_100khz(self, sda: Pin<2>, scl: Pin<3>) -> Result<I2c1Host, I2c1Error> {
        let component_type = reg(IC_COMP_TYPE).read();
        if component_type != IC_COMP_TYPE_VALUE {
            return Err(I2c1Error::ComponentType(component_type));
        }

        let component_parameter = reg(IC_COMP_PARAM_1).read();
        let tx_fifo_depth = tx_fifo_depth(component_parameter);
        if tx_fifo_depth < 2 {
            return Err(I2c1Error::FifoTooShallow(tx_fifo_depth));
        }

        disable()?;
        reg(IC_INTR_MASK).write(0);
        let _ = reg(IC_CLR_INTR).read();
        reg(IC_SS_SCL_HCNT).write(IC_SS_HCNT_200MHZ);
        reg(IC_SS_SCL_LCNT).write(IC_SS_LCNT_200MHZ);
        reg(IC_SDA_HOLD).write(IC_SDA_HOLD_200MHZ);
        reg(IC_TX_TL).write(0);
        reg(IC_RX_TL).write(0);
        reg(IC_CON).write(IC_CON_MASTER_STD_RESTART);

        let sda = sda.into_function::<3>();
        let scl = scl.into_function::<3>();
        configure_i2c_pin::<2>();
        configure_i2c_pin::<3>();

        Ok(I2c1Host {
            _sda: sda,
            _scl: scl,
            tx_fifo_depth,
        })
    }
}

impl I2c1Host {
    pub fn write(&mut self, address: u8, payload: &[u8]) -> Result<I2c1Snapshot, I2c1Error> {
        if address > 0x7f {
            return Err(I2c1Error::InvalidAddress(address));
        }
        if payload.is_empty() {
            return Err(I2c1Error::EmptyPayload);
        }
        if payload.len() > usize::from(self.tx_fifo_depth) {
            return Err(I2c1Error::PayloadTooLong {
                len: payload.len(),
                fifo_depth: self.tx_fifo_depth,
            });
        }

        disable()?;
        reg(IC_TAR).write(u32::from(address));
        reg(IC_INTR_MASK).write(0);
        let _ = reg(IC_CLR_INTR).read();
        reg(IC_ENABLE).write(IC_ENABLE_ENABLE);
        if !poll_until(
            || reg(IC_ENABLE).read() & IC_ENABLE_ENABLE != 0,
            CONTROL_POLL_LIMIT,
        ) {
            return Err(I2c1Error::EnableTimeout);
        }

        for (index, byte) in payload.iter().copied().enumerate() {
            if !poll_until(
                || reg(IC_TXFLR).read() < u32::from(self.tx_fifo_depth),
                CONTROL_POLL_LIMIT,
            ) {
                return Err(I2c1Error::TxFifoTimeout);
            }
            let stop = if index + 1 == payload.len() {
                IC_DATA_CMD_STOP
            } else {
                0
            };
            reg(IC_DATA_CMD).write(u32::from(byte) | stop);
        }

        for _ in 0..TRANSFER_POLL_LIMIT {
            let raw = reg(IC_RAW_INTR_STAT).read();
            if raw & IC_INTR_TX_ABRT != 0 {
                let source = reg(IC_TX_ABRT_SOURCE).read();
                let _ = reg(IC_CLR_TX_ABRT).read();
                return Err(I2c1Error::TxAbort(source));
            }
            if raw & IC_INTR_STOP_DET != 0 {
                let _ = reg(IC_CLR_STOP_DET).read();
                return Ok(self.snapshot(payload.len() as u16, 0));
            }
            core::hint::spin_loop();
        }

        Err(I2c1Error::StopTimeout)
    }

    pub fn snapshot(&self, bytes_queued: u16, abort_source: u32) -> I2c1Snapshot {
        I2c1Snapshot {
            component_type: reg(IC_COMP_TYPE).read(),
            component_parameter: reg(IC_COMP_PARAM_1).read(),
            control: reg(IC_CON).read(),
            target: reg(IC_TAR).read(),
            enable_status: reg(IC_ENABLE_STATUS).read(),
            status: reg(IC_STATUS).read(),
            raw_interrupt_status: reg(IC_RAW_INTR_STAT).read(),
            abort_source,
            tx_fifo_depth: self.tx_fifo_depth,
            bytes_queued,
        }
    }
}

fn disable() -> Result<(), I2c1Error> {
    reg(IC_ENABLE).write(0);
    if poll_until(
        || reg(IC_ENABLE_STATUS).read() & IC_ENABLE_ENABLE == 0,
        CONTROL_POLL_LIMIT,
    ) {
        Ok(())
    } else {
        Err(I2c1Error::DisableTimeout)
    }
}

fn poll_until(mut ready: impl FnMut() -> bool, limit: u32) -> bool {
    for _ in 0..limit {
        if ready() {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

const fn tx_fifo_depth(component_parameter: u32) -> u16 {
    (((component_parameter >> 16) & 0xff) + 1) as u16
}

#[inline(always)]
fn reg(offset: usize) -> Reg<u32> {
    unsafe { Reg::new(I2C1_BASE + offset) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_map_matches_rp1_designware_i2c1() {
        assert_eq!(I2C1_BASE, 0x4007_4000);
        assert_eq!(I2C1_BASE + IC_CON, 0x4007_4000);
        assert_eq!(I2C1_BASE + IC_DATA_CMD, 0x4007_4010);
        assert_eq!(I2C1_BASE + IC_ENABLE, 0x4007_406c);
        assert_eq!(I2C1_BASE + IC_COMP_TYPE, 0x4007_40fc);
    }

    #[test]
    fn standard_master_configuration_matches_linux_contract() {
        assert_eq!(IC_CON_MASTER_STD_RESTART, 0x63);
        assert_eq!(IC_SS_HCNT_200MHZ, 781);
        assert_eq!(IC_SS_LCNT_200MHZ, 1_186);
        assert_eq!(IC_SDA_HOLD_200MHZ, 593);
    }

    #[test]
    fn fifo_depth_decodes_component_parameter() {
        assert_eq!(tx_fifo_depth(31 << 16), 32);
        assert_eq!(tx_fifo_depth(1 << 16), 2);
    }

    #[test]
    fn terminal_command_sets_stop_only() {
        assert_eq!(u32::from(0x5a_u8) | IC_DATA_CMD_STOP, 0x25a);
    }
}

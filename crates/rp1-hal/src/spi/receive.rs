//! Bounded full-duplex RX service for the RP1 DesignWare SSI.
//!
//! Register semantics follow Raspberry Pi Linux spi-dw; this is not an RP1
//! local IRQ routing proof. No NVIC, clock, reset or peer GPIO is programmed.

use super::*;

const CTRLR0_DFS_8BIT_TX_RX_MODE0: u32 = 7 << 16;
const IRQ_TXOI: u32 = 1 << 1;
const IRQ_RXUI: u32 = 1 << 2;
const IRQ_RXOI: u32 = 1 << 3;
const IRQ_RXFI: u32 = 1 << 4;
const IRQ_ERRORS: u32 = IRQ_TXOI | IRQ_RXUI | IRQ_RXOI;
const IRQ_RX_MASK: u32 = IRQ_ERRORS | IRQ_RXFI;
const ICR: usize = 0x48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Spi0RxError {
    Setup(Spi0Error),
    LengthMismatch { tx: usize, rx: usize },
    NotPrepared,
    Incomplete { received: usize, expected: usize },
    InterruptFault(u32),
    UnexpectedSource(u32),
    InvalidFifoLevel { level: u32, remaining: usize },
    TransferTimeout,
    Cancelled,
    CleanupReadback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Spi0RxState {
    Prepared,
    Active,
    /// All bytes arrived; the final serial edge/CS completion is not yet proven.
    RxComplete,
    /// Foreground finish observed idle TX and verified local cleanup.
    Complete,
    Failed(Spi0RxError),
}

struct RxBuffer<'a> {
    data: &'a mut [u8],
    received: usize,
    fifo_depth: u16,
    state: Spi0RxState,
}

impl<'a> RxBuffer<'a> {
    fn new(data: &'a mut [u8], tx_len: usize, fifo_depth: u16) -> Result<Self, Spi0RxError> {
        if fifo_depth == 0 || fifo_depth > FIFO_DEPTH_LIMIT {
            return Err(Spi0RxError::Setup(Spi0Error::FifoDepthUnknown));
        }
        if tx_len == 0 {
            return Err(Spi0RxError::Setup(Spi0Error::EmptyPayload));
        }
        if tx_len != data.len() {
            return Err(Spi0RxError::LengthMismatch {
                tx: tx_len,
                rx: data.len(),
            });
        }
        if tx_len > usize::from(fifo_depth) {
            return Err(Spi0RxError::Setup(Spi0Error::PayloadTooLong {
                len: tx_len,
                fifo_depth,
            }));
        }
        Ok(Self {
            data,
            received: 0,
            fifo_depth,
            state: Spi0RxState::Prepared,
        })
    }

    fn start(&mut self) -> Result<(), Spi0RxError> {
        if self.state != Spi0RxState::Prepared {
            return Err(Spi0RxError::NotPrepared);
        }
        self.state = Spi0RxState::Active;
        Ok(())
    }

    fn fail(&mut self, error: Spi0RxError) {
        self.state = Spi0RxState::Failed(error);
    }

    // The production ISR and host tests use this same bounded drain logic.
    fn service(&mut self, masked_status: u32, level: u32, mut read: impl FnMut() -> u32) {
        if self.state != Spi0RxState::Active || masked_status == 0 {
            return;
        }
        if masked_status & IRQ_ERRORS != 0 {
            self.fail(Spi0RxError::InterruptFault(masked_status & IRQ_ERRORS));
            return;
        }
        if masked_status & !IRQ_RX_MASK != 0 {
            self.fail(Spi0RxError::UnexpectedSource(masked_status));
            return;
        }
        let remaining = self.data.len() - self.received;
        if level == 0 || level > u32::from(self.fifo_depth) || level as usize > remaining {
            self.fail(Spi0RxError::InvalidFifoLevel { level, remaining });
            return;
        }
        for _ in 0..level {
            self.data[self.received] = read() as u8;
            self.received += 1;
        }
        if self.received == self.data.len() {
            self.state = Spi0RxState::RxComplete;
        }
    }

    fn finish_error(&self) -> Option<Spi0RxError> {
        match self.state {
            Spi0RxState::RxComplete | Spi0RxState::Complete => None,
            Spi0RxState::Failed(error) => Some(error),
            _ => Some(Spi0RxError::Incomplete {
                received: self.received,
                expected: self.data.len(),
            }),
        }
    }
}

/// Owns SPI0 and the RX buffer until finish/abort/drop. The application must
/// arrange exclusive ISR access, keep the selected local NVIC route masked
/// through prepare/start, and disable that route before releasing this object.
/// No receive polling, IRQ discovery, NVIC pending clear or deadline scheduler
/// is provided. The caller must impose a bounded deadline and call abort on it.
/// Drop only performs best-effort mask/stop; use finish/abort for checked cleanup.
pub struct Spi0IrqTransfer<'a> {
    _host: &'a mut Spi0Host,
    rx: RxBuffer<'a>,
}

impl Spi0Host {
    /// Preload TX with CS inactive; call start only after arranging ISR ownership.
    ///
    /// Electrical MISO/peer timing and a known-good reset/clock baseline are
    /// preconditions. This keeps the established nominal 100 kHz divider.
    pub fn prepare_irq_transfer<'a>(
        &'a mut self,
        tx: &[u8],
        rx: &'a mut [u8],
    ) -> Result<Spi0IrqTransfer<'a>, Spi0RxError> {
        // ponytail: one FIFO per transaction; add IRQ TX refill only after this
        // bounded receive path and the actual local IRQ route are HW-proven.
        let rx = RxBuffer::new(rx, tx.len(), self.fifo_depth)?;
        let transfer = Spi0IrqTransfer { _host: self, rx };
        quiesce()?;
        reg(CTRLR0).write(CTRLR0_DFS_8BIT_TX_RX_MODE0);
        reg(BAUDR).write(BAUD_DIV_100KHZ_AT_200MHZ);
        reg(TXFTLR).write(0);
        reg(RXFTLR).write(0); // RXFI is a level source: any received byte.
        reg(DMACR).write(0);
        reg(SSIENR).write(SSI_ENABLE);
        if !poll_until(|| reg(SSIENR).read() & SSI_ENABLE != 0, CONTROL_POLL_LIMIT) {
            return Err(Spi0RxError::Setup(Spi0Error::EnableTimeout));
        }
        for byte in tx {
            if !poll_until(|| reg(SR).read() & SR_TX_NOT_FULL != 0, CONTROL_POLL_LIMIT) {
                return Err(Spi0RxError::Setup(Spi0Error::TxFifoTimeout));
            }
            reg(DR).write(u32::from(*byte));
        }
        let errors = reg(RISR).read() & IRQ_ERRORS;
        if errors != 0 {
            return Err(Spi0RxError::InterruptFault(errors));
        }
        Ok(transfer)
    }
}

impl Spi0IrqTransfer<'_> {
    pub fn state(&self) -> Spi0RxState {
        self.rx.state
    }

    pub fn received(&self) -> &[u8] {
        &self.rx.data[..self.rx.received]
    }

    pub fn start(&mut self) -> Result<(), Spi0RxError> {
        self.rx.start()?;
        reg(IMR).write(IRQ_RX_MASK);
        reg(SER).write(SER_CS0);
        Ok(())
    }

    /// Call from the verified local ISR, not from a polling loop. A zero ISR
    /// source never reads DR. Draining RXFLR clears level RXFI naturally; fault
    /// latches are cleared only during masked, disabled foreground cleanup.
    pub fn on_interrupt(&mut self) -> Spi0RxState {
        if self.rx.state == Spi0RxState::Active {
            let source = reg(ISR).read();
            let level = reg(RXFLR).read();
            self.rx.service(source, level, || reg(DR).read());
        }
        if self.rx.state != Spi0RxState::Active {
            reg(IMR).write(0);
            if matches!(self.rx.state, Spi0RxState::Failed(_)) {
                stop();
            }
        }
        io_barrier();
        self.rx.state
    }

    /// Foreground completion: never drains RX by polling. The bounded status
    /// wait prevents deasserting CS immediately on the last received byte.
    pub fn finish(&mut self) -> Result<(), Spi0RxError> {
        if self.rx.state == Spi0RxState::Complete {
            return Ok(());
        }
        let mut error = self.rx.finish_error();
        if error.is_none() {
            if !poll_until(
                || {
                    reg(SR).read() & (SR_BUSY | SR_TX_EMPTY) == SR_TX_EMPTY
                        && reg(TXFLR).read() == 0
                },
                TRANSFER_POLL_LIMIT,
            ) {
                error = Some(Spi0RxError::TransferTimeout);
            } else {
                let faults = reg(RISR).read() & IRQ_ERRORS;
                let level = reg(RXFLR).read();
                if faults != 0 {
                    error = Some(Spi0RxError::InterruptFault(faults));
                } else if level != 0 {
                    error = Some(Spi0RxError::InvalidFifoLevel {
                        level,
                        remaining: 0,
                    });
                }
            }
        }
        if let Err(cleanup) = quiesce() {
            error = Some(cleanup);
        }
        if let Some(error) = error {
            self.rx.fail(error);
            Err(error)
        } else {
            self.rx.state = Spi0RxState::Complete;
            Ok(())
        }
    }

    /// Cancel on the caller's deadline; preserves the received prefix.
    pub fn abort(&mut self) -> Result<(), Spi0RxError> {
        self.rx.fail(Spi0RxError::Cancelled);
        let result = quiesce();
        if let Err(error) = result {
            self.rx.fail(error);
        }
        result
    }
}

impl Drop for Spi0IrqTransfer<'_> {
    fn drop(&mut self) {
        reg(IMR).write(0);
        stop();
        io_barrier();
    }
}

fn io_barrier() {
    #[cfg(target_arch = "arm")]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

fn quiesce() -> Result<(), Spi0RxError> {
    reg(IMR).write(0);
    stop();
    disable().map_err(Spi0RxError::Setup)?;
    let _ = reg(ICR).read(); // DW SSI read-to-clear latched faults, not RXFI.
    if reg(IMR).read() != 0
        || reg(SER).read() != 0
        || reg(ISR).read() != 0
        || reg(RISR).read() & IRQ_ERRORS != 0
        || reg(TXFLR).read() != 0
        || reg(RXFLR).read() != 0
    {
        return Err(Spi0RxError::CleanupReadback);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_before_any_hardware_action() {
        let mut data = [0; 5];
        assert!(matches!(
            RxBuffer::new(&mut [], 0, 4),
            Err(Spi0RxError::Setup(Spi0Error::EmptyPayload))
        ));
        assert!(matches!(
            RxBuffer::new(&mut data, 4, 8),
            Err(Spi0RxError::LengthMismatch { tx: 4, rx: 5 })
        ));
        assert!(matches!(
            RxBuffer::new(&mut data, 5, 4),
            Err(Spi0RxError::Setup(Spi0Error::PayloadTooLong { .. }))
        ));
        for depth in [0, 257] {
            assert!(matches!(
                RxBuffer::new(&mut data, 5, depth),
                Err(Spi0RxError::Setup(Spi0Error::FifoDepthUnknown))
            ));
        }
    }

    #[test]
    fn drains_real_payload_in_partial_fifo_batches_then_ignores_replay() {
        let mut data = [0; 5];
        let mut rx = RxBuffer::new(&mut data, 5, 8).unwrap();
        rx.service(IRQ_RXFI, 1, || panic!("not started"));
        rx.start().unwrap();
        assert_eq!(rx.start(), Err(Spi0RxError::NotPrepared));
        rx.service(0, 3, || panic!("spurious IRQ must not read DR"));
        let mut payload = [0xa5, 0x00, 0xff, 0x42, 0x19].into_iter();
        rx.service(IRQ_RXFI, 2, || payload.next().unwrap());
        assert_eq!(rx.state, Spi0RxState::Active);
        assert_eq!(rx.received, 2);
        rx.service(IRQ_RXFI, 3, || payload.next().unwrap());
        assert_eq!(rx.data, &[0xa5, 0x00, 0xff, 0x42, 0x19]);
        assert_eq!(rx.state, Spi0RxState::RxComplete);
        assert_eq!(rx.finish_error(), None);
        rx.service(IRQ_RXFI, 1, || panic!("terminal replay must not read DR"));
        assert_eq!(rx.state, Spi0RxState::RxComplete);
    }

    #[test]
    fn single_byte_and_full_fifo_boundaries() {
        for len in [1, 256] {
            let mut data = [0; 256];
            let mut rx = RxBuffer::new(&mut data[..len], len, 256).unwrap();
            rx.start().unwrap();
            let mut reads = 0;
            rx.service(IRQ_RXFI, len as u32, || {
                reads += 1;
                reads
            });
            assert_eq!(reads as usize, len);
            assert_eq!(rx.received, len);
            assert_eq!(rx.data[len - 1], len as u8);
            assert_eq!(rx.state, Spi0RxState::RxComplete);
        }
    }

    #[test]
    fn invalid_levels_never_read_or_overwrite_buffer() {
        for level in [0, 5, 9, u32::MAX] {
            let mut data = [0x55; 4];
            let mut rx = RxBuffer::new(&mut data, 4, 8).unwrap();
            rx.start().unwrap();
            rx.service(IRQ_RXFI, level, || panic!("invalid level"));
            assert_eq!(
                rx.state,
                Spi0RxState::Failed(Spi0RxError::InvalidFifoLevel {
                    level,
                    remaining: 4
                })
            );
            assert_eq!(rx.data, &[0x55; 4]);
            rx.service(IRQ_RXFI, 1, || panic!("failed transfer replay"));
            assert_eq!(rx.received, 0);
        }
    }

    #[test]
    fn faults_win_over_rx_and_keep_completed_prefix() {
        for fault in [IRQ_TXOI, IRQ_RXUI, IRQ_RXOI, IRQ_ERRORS] {
            let mut data = [0; 2];
            let mut rx = RxBuffer::new(&mut data, 2, 8).unwrap();
            rx.start().unwrap();
            rx.service(IRQ_RXFI, 1, || 0x42);
            rx.service(IRQ_RXFI | fault, 1, || panic!("fault must prevent DR read"));
            assert_eq!(rx.finish_error(), Some(Spi0RxError::InterruptFault(fault)));
            assert_eq!(rx.received, 1);
            assert_eq!(rx.data, &[0x42, 0]);
        }
    }

    #[test]
    fn unexpected_sources_and_partial_completion_fail_closed() {
        let mut data = [0; 2];
        let mut rx = RxBuffer::new(&mut data, 2, 8).unwrap();
        rx.start().unwrap();
        rx.service(IRQ_RXFI, 1, || 0x13);
        assert_eq!(
            rx.finish_error(),
            Some(Spi0RxError::Incomplete {
                received: 1,
                expected: 2
            })
        );
        rx.service(IRQ_TXEI, 0, || panic!("unexpected source"));
        assert_eq!(
            rx.state,
            Spi0RxState::Failed(Spi0RxError::UnexpectedSource(IRQ_TXEI))
        );
    }

    #[test]
    fn rearm_uses_new_state_not_stale_count_or_fault() {
        let mut data = [0; 1];
        {
            let mut rx = RxBuffer::new(&mut data, 1, 8).unwrap();
            rx.start().unwrap();
            rx.fail(Spi0RxError::Cancelled);
            rx.service(IRQ_RXFI, 1, || panic!("cancelled"));
        }
        let mut rx = RxBuffer::new(&mut data, 1, 8).unwrap();
        assert_eq!(rx.received, 0);
        rx.start().unwrap();
        rx.service(IRQ_RXFI, 1, || 0x99);
        assert_eq!(rx.data, &[0x99]);
        assert_eq!(rx.state, Spi0RxState::RxComplete);
    }
}

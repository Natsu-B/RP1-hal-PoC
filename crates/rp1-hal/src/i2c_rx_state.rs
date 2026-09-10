//! Pure, single-owner I2C RX accounting; NOT an IRQ/MMIO implementation.
//!
//! The adapter must serialize every call with its proc0 timeout/IRQ path. FIFO
//! levels must be fresh, full-width hardware observations. Prepare an operation,
//! perform exactly that guarded hardware access, then record its actual outcome
//! before preparing another operation. Tokens do not perform or prove an access.
//! Observe fatal/abort before any access and again at the end of IRQ service.
//! Hardware IRQs have no generation tag: pending/active/quiet cleanup is still
//! required before assigning the next generation to a newly delivered IRQ.

pub const MAX_LEN: usize = 33;
pub const FATAL_CAUSES: u32 = 0x4b;
pub const STOP_DET: u32 = 0x200;
pub const OWNED_CAUSES: u32 = FATAL_CAUSES | 4 | STOP_DET;
const READ_COMMAND: u32 = 0x100;
const COMMAND_STOP: u32 = 0x200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Fatal { causes: u32, abort_source: u32 },
    InvalidTxLevel(u32),
    InvalidRxLevel(u32),
    EmptyPop,
    UncommittedIo,
    Mismatch,
    PrematureStop,
    Timeout,
    Adapter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Idle,
    Active,
    Complete,
    Failed(Error),
    TimedOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejected {
    InvalidDepth,
    InvalidLength,
    NotClean,
    GenerationExhausted,
    StaleGeneration,
    Inactive,
    PendingIo,
    WrongOperation,
    RequestFailed(Error),
    CleanupFailed(CleanupError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupError {
    NotDisabled,
    NotMasked,
    FifoNotEmpty,
    CausesNotClear,
    IrqNotQuiescent,
    QuietNotObserved,
}

/// Caller evidence collected AFTER bounded disable, any failure-only discard,
/// selective observed-cause acknowledgement, and owned NVIC pending cleanup.
/// `quiet_conditions_held` attests these conditions held THROUGHOUT the measured
/// interval, not merely at its endpoint. This struct cannot establish that fact.
#[derive(Clone, Copy, Debug)]
pub struct CleanupEvidence {
    pub enable_status: u32,
    pub interrupt_mask: u32,
    pub rx_level: u32,
    pub tx_level: u32,
    pub raw_status: u32,
    pub masked_status: u32,
    pub abort_source: u32,
    pub owned_irq_enabled: bool,
    pub owned_irq_pending: bool,
    pub owned_irq_active: bool,
    pub irq_count_before: u32,
    pub irq_count_after: u32,
    pub quiet_conditions_held: bool,
    pub quiet_elapsed_ticks: u32,
    pub quiet_required_ticks: u32,
}

impl CleanupEvidence {
    fn validate(self) -> Result<(), CleanupError> {
        if self.enable_status != 0 {
            Err(CleanupError::NotDisabled)
        } else if self.interrupt_mask != 0 {
            Err(CleanupError::NotMasked)
        } else if self.rx_level != 0 || self.tx_level != 0 {
            Err(CleanupError::FifoNotEmpty)
        } else if self.raw_status & OWNED_CAUSES != 0
            || self.masked_status != 0
            || self.abort_source != 0
        {
            Err(CleanupError::CausesNotClear)
        } else if self.owned_irq_enabled || self.owned_irq_pending || self.owned_irq_active {
            Err(CleanupError::IrqNotQuiescent)
        } else if !self.quiet_conditions_held
            || self.quiet_required_ticks == 0
            || self.quiet_elapsed_ticks < self.quiet_required_ticks
            || self.irq_count_before != self.irq_count_after
        {
            Err(CleanupError::QuietNotObserved)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mismatch {
    pub index: u8,
    pub expected: u8,
    pub actual: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Pending {
    None,
    Read,
    Pop,
}

/// One non-cloneable permission; the caller must actually write `word()` before
/// `record_issued`. Never substitute this token for hardware write evidence.
#[derive(Debug)]
pub struct ReadCommand {
    generation: u32,
    index: u8,
    stop: bool,
}

impl ReadCommand {
    pub fn word(&self) -> u32 {
        READ_COMMAND | if self.stop { COMMAND_STOP } else { 0 }
    }
}

/// One non-cloneable permission to TRY one guarded FIFO pop. The adapter must
/// recheck RXFLR before the actual pop; a failed guard is recorded as `None`.
#[derive(Debug)]
pub struct PopPermit {
    generation: u32,
    index: u8,
}

// ponytail: fixed 33-byte read-only requests; extend only with a real larger or
// write-read use case. No heap, DMA, executor, callback, or MMIO abstraction.
pub struct RxState {
    rx_depth: u16,
    tx_depth: u16,
    generation: u32,
    state: State,
    clean: bool,
    cleanup_error: Option<CleanupError>,
    requested: u8,
    issued: u8,
    drained: u8,
    stop_seen: bool,
    pending: Pending,
    expected: [u8; MAX_LEN],
    check_expected: bool,
    bytes: [u8; MAX_LEN],
    first_mismatch: Option<Mismatch>,
}

impl RxState {
    /// Depths are caller-provided observations of the existing 8-bit encoded
    /// depth fields (+1); construction itself provides no clean-hardware claim.
    pub fn new(rx_depth: u16, tx_depth: u16) -> Result<Self, Rejected> {
        if !(1..=256).contains(&rx_depth) || !(1..=256).contains(&tx_depth) {
            return Err(Rejected::InvalidDepth);
        }
        Ok(Self {
            rx_depth,
            tx_depth,
            generation: 0,
            state: State::Idle,
            clean: false,
            cleanup_error: None,
            requested: 0,
            issued: 0,
            drained: 0,
            stop_seen: false,
            pending: Pending::None,
            expected: [0; MAX_LEN],
            check_expected: false,
            bytes: [0; MAX_LEN],
            first_mismatch: None,
        })
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn primary_error(&self) -> Option<Error> {
        match self.state {
            State::Failed(error) => Some(error),
            State::TimedOut => Some(Error::Timeout),
            _ => None,
        }
    }
    pub fn counts(&self) -> (u8, u8, u8) {
        (self.requested, self.issued, self.drained)
    }
    pub fn stop_seen(&self) -> bool {
        self.stop_seen
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.drained)]
    }
    pub fn first_mismatch(&self) -> Option<Mismatch> {
        self.first_mismatch
    }
    pub fn cleanup_error(&self) -> Option<CleanupError> {
        self.cleanup_error
    }
    pub fn is_clean(&self) -> bool {
        self.clean
    }

    /// A later successful cleanup permits reuse but preserves the first cleanup
    /// failure for this generation. Neither cleanup outcome changes its primary.
    pub fn record_cleanup(
        &mut self,
        generation: u32,
        evidence: CleanupEvidence,
    ) -> Result<(), Rejected> {
        self.check_generation(generation)?;
        if self.state == State::Active {
            return Err(Rejected::PendingIo);
        }
        let result = evidence.validate();
        self.clean = result.is_ok();
        if let Err(error) = result {
            self.cleanup_error.get_or_insert(error);
            return Err(Rejected::CleanupFailed(error));
        }
        Ok(())
    }

    /// The evidence retained by record_cleanup must still describe the hardware;
    /// do not enable the peripheral or modify its FIFO between cleanup and arm.
    pub fn arm(&mut self, expected: &[u8]) -> Result<u32, Rejected> {
        self.arm_inner(expected.len(), Some(expected))
    }

    /// Receive real bytes without a predicted payload. Geometry, command/RX
    /// credit, fatal/STOP priority and clean-generation prerequisites still hold.
    pub fn arm_read(&mut self, length: usize) -> Result<u32, Rejected> {
        self.arm_inner(length, None)
    }

    fn arm_inner(&mut self, length: usize, expected: Option<&[u8]>) -> Result<u32, Rejected> {
        if length == 0 || length > MAX_LEN {
            return Err(Rejected::InvalidLength);
        }
        if !self.clean || self.state == State::Active {
            return Err(Rejected::NotClean);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(Rejected::GenerationExhausted)?;
        self.generation = generation;
        self.state = State::Active;
        self.clean = false;
        self.cleanup_error = None;
        self.requested = length as u8;
        self.issued = 0;
        self.drained = 0;
        self.stop_seen = false;
        self.pending = Pending::None;
        self.expected = [0; MAX_LEN];
        self.check_expected = expected.is_some();
        if let Some(expected) = expected { self.expected[..length].copy_from_slice(expected); }
        self.bytes = [0; MAX_LEN];
        self.first_mismatch = None;
        Ok(generation)
    }

    pub fn prepare_read(
        &mut self,
        generation: u32,
        tx_level: u32,
    ) -> Result<Option<ReadCommand>, Rejected> {
        self.ready_for_io(generation)?;
        if tx_level > u32::from(self.tx_depth) {
            return self.fail(Error::InvalidTxLevel(tx_level));
        }
        if self.issued == self.requested
            || tx_level == u32::from(self.tx_depth)
            || u16::from(self.issued - self.drained) == self.rx_depth
        {
            return Ok(None);
        }
        self.pending = Pending::Read;
        Ok(Some(ReadCommand {
            generation,
            index: self.issued,
            stop: self.issued + 1 == self.requested,
        }))
    }

    /// Call only AFTER the single command write, in the same serialized section.
    pub fn record_issued(&mut self, command: ReadCommand) -> Result<(), Rejected> {
        self.active(command.generation)?;
        if self.pending != Pending::Read || command.index != self.issued {
            return Err(Rejected::WrongOperation);
        }
        self.issued += 1;
        self.pending = Pending::None;
        Ok(())
    }

    /// A zero level returns no permission, so the adapter must perform no read.
    /// Reject full-width corrupt/extra levels BEFORE touching DATA_CMD.
    pub fn prepare_pop(
        &mut self,
        generation: u32,
        rx_level: u32,
    ) -> Result<Option<PopPermit>, Rejected> {
        self.ready_for_io(generation)?;
        if rx_level > u32::from(self.rx_depth)
            || rx_level > u32::from(self.issued - self.drained)
            || rx_level > u32::from(self.requested - self.drained)
        {
            return self.fail(Error::InvalidRxLevel(rx_level));
        }
        if rx_level == 0 {
            return Ok(None);
        }
        self.pending = Pending::Pop;
        Ok(Some(PopPermit {
            generation,
            index: self.drained,
        }))
    }

    /// Record the actual guarded pop result, never a default or synthetic byte.
    /// Does NOT complete: the final fatal/STOP snapshot must still be observed.
    pub fn record_pop(&mut self, permit: PopPermit, actual: Option<u8>) -> Result<(), Rejected> {
        self.active(permit.generation)?;
        if self.pending != Pending::Pop || permit.index != self.drained {
            return Err(Rejected::WrongOperation);
        }
        let Some(actual) = actual else {
            return self.fail(Error::EmptyPop);
        };
        let index = usize::from(self.drained);
        self.bytes[index] = actual;
        if self.check_expected && actual != self.expected[index] && self.first_mismatch.is_none() {
            self.first_mismatch = Some(Mismatch {
                index: self.drained,
                expected: self.expected[index],
                actual,
            });
        }
        self.drained += 1;
        self.pending = Pending::None;
        Ok(())
    }

    /// Apply an entry or post-service hardware snapshot. `causes` is raw|masked,
    /// with abort_source captured BEFORE clear. STOP is latched, not acknowledged
    /// here. Fatal beats last-byte completion AND a recorded payload mismatch.
    pub fn observe(
        &mut self,
        generation: u32,
        causes: u32,
        abort_source: u32,
    ) -> Result<State, Rejected> {
        self.active(generation)?;
        self.stop_seen |= causes & STOP_DET != 0;
        let error = if causes & FATAL_CAUSES != 0 || abort_source != 0 {
            Some(Error::Fatal {
                causes: causes & FATAL_CAUSES,
                abort_source,
            })
        } else if self.pending != Pending::None {
            Some(Error::UncommittedIo)
        } else if self.first_mismatch.is_some() {
            Some(Error::Mismatch)
        } else if self.stop_seen && self.issued != self.requested {
            Some(Error::PrematureStop)
        } else {
            None
        };
        if let Some(error) = error {
            self.state = State::Failed(error);
            self.pending = Pending::None;
        } else if self.stop_seen && self.drained == self.requested {
            self.state = State::Complete;
        }
        Ok(self.state)
    }

    /// End an active request for an adapter fault; retain its detailed reason in
    /// adapter evidence. Ok means the failure was recorded, not receive success.
    /// Pending tokens are invalidated; this performs no hardware cleanup.
    pub fn fail_adapter(&mut self, generation: u32) -> Result<(), Rejected> {
        self.active(generation)?;
        self.state = State::Failed(Error::Adapter);
        self.pending = Pending::None;
        Ok(())
    }

    /// Caller supplies an already-validated deadline/iteration-limit event.
    /// This module neither reads a clock nor implements a wait/timeout policy.
    pub fn timeout(&mut self, generation: u32) -> Result<(), Rejected> {
        self.active(generation)?;
        self.state = State::TimedOut;
        self.pending = Pending::None;
        Ok(())
    }

    fn check_generation(&self, generation: u32) -> Result<(), Rejected> {
        if generation != self.generation {
            Err(Rejected::StaleGeneration)
        } else {
            Ok(())
        }
    }
    fn active(&self, generation: u32) -> Result<(), Rejected> {
        self.check_generation(generation)?;
        if self.state == State::Active {
            Ok(())
        } else {
            Err(Rejected::Inactive)
        }
    }
    fn ready_for_io(&self, generation: u32) -> Result<(), Rejected> {
        self.active(generation)?;
        if self.pending == Pending::None {
            Ok(())
        } else {
            Err(Rejected::PendingIo)
        }
    }
    fn fail<T>(&mut self, error: Error) -> Result<T, Rejected> {
        self.state = State::Failed(error);
        self.pending = Pending::None;
        Err(Rejected::RequestFailed(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean() -> CleanupEvidence {
        CleanupEvidence {
            enable_status: 0,
            interrupt_mask: 0,
            rx_level: 0,
            tx_level: 0,
            raw_status: 0,
            masked_status: 0,
            abort_source: 0,
            owned_irq_enabled: false,
            owned_irq_pending: false,
            owned_irq_active: false,
            irq_count_before: 7,
            irq_count_after: 7,
            quiet_conditions_held: true,
            quiet_elapsed_ticks: 4_000,
            quiet_required_ticks: 4_000,
        }
    }
    fn armed(expected: &[u8], rx: u16, tx: u16) -> (RxState, u32) {
        let mut s = RxState::new(rx, tx).unwrap();
        assert_eq!(s.record_cleanup(0, clean()), Ok(()));
        let g = s.arm(expected).unwrap();
        (s, g)
    }
    fn issue(s: &mut RxState, g: u32, tx_level: u32) -> u32 {
        let command = s.prepare_read(g, tx_level).unwrap().unwrap();
        let word = command.word();
        s.record_issued(command).unwrap(); // simulated successful hardware write
        word
    }
    fn pop(s: &mut RxState, g: u32, level: u32, byte: u8) {
        let permit = s.prepare_pop(g, level).unwrap().unwrap();
        s.record_pop(permit, Some(byte)).unwrap(); // simulated guarded physical pop
    }

    #[test]
    fn unpredictable_payload_retains_geometry_stop_and_cleanup_contracts() {
        let mut s = RxState::new(2, 2).unwrap();
        assert_eq!(s.arm_read(1), Err(Rejected::NotClean));
        s.record_cleanup(0, clean()).unwrap();
        for length in [0, MAX_LEN+1, usize::MAX] {
            assert_eq!(s.arm_read(length), Err(Rejected::InvalidLength));
        }
        for length in [1, 2, 31, 32, 33] {
            let g = s.arm_read(length).unwrap();
            for n in 0..length {
                assert_eq!(issue(&mut s,g,0), 0x100 | if n+1==length {0x200} else {0});
                pop(&mut s,g,1,(n as u8).wrapping_mul(13)^0xa7);
            }
            assert_eq!(s.observe(g,0,0), Ok(State::Active));
            assert_eq!(s.observe(g,STOP_DET,0), Ok(State::Complete));
            assert_eq!(s.first_mismatch(),None);
            for (n,byte) in s.bytes().iter().enumerate() { assert_eq!(*byte,(n as u8).wrapping_mul(13)^0xa7); }
            s.record_cleanup(g,clean()).unwrap();
        }
        // Explicit expected-payload mode still rejects a mismatch after reuse.
        let g=s.arm(&[0]).unwrap();issue(&mut s,g,0);pop(&mut s,g,1,0xff);
        assert_eq!(s.observe(g,STOP_DET,0),Ok(State::Failed(Error::Mismatch)));
    }

    #[test]
    fn boundary_lengths_credits_tx_capacity_stop_orders_and_rearm() {
        for rx in [1, 2, 31, 32] {
            for tx in [1, 2, 31, 32] {
                let mut s = RxState::new(rx, tx).unwrap();
                for len in [1, 2, 31, 32, 33] {
                    for stop_first in [false, true] {
                        let expected: [u8; MAX_LEN] = core::array::from_fn(|i| {
                            (i as u8).wrapping_mul(7) ^ s.generation() as u8
                        });
                        assert_eq!(s.record_cleanup(s.generation(), clean()), Ok(()));
                        let previous = s.generation();
                        let g = s.arm(&expected[..len]).unwrap();
                        assert_eq!(g, previous + 1);
                        let mut command_count = 0;
                        let mut stop_commands = 0;
                        while s.drained < s.requested {
                            // Simulated TX accepts at most depth commands before
                            // the bus consumes that batch and produces RX bytes.
                            let mut tx_level = 0;
                            while let Some(command) = s.prepare_read(g, tx_level).unwrap() {
                                stop_commands += usize::from(command.word() == 0x300);
                                command_count += 1;
                                assert_eq!(command.word() == 0x300, command_count == len);
                                s.record_issued(command).unwrap();
                                tx_level += 1;
                                assert!(tx_level <= u32::from(tx));
                                assert!(u16::from(s.issued - s.drained) <= rx);
                            }
                            assert!(s.issued > s.drained);
                            let terminal_batch = s.issued == s.requested;
                            if stop_first && terminal_batch {
                                assert_eq!(s.observe(g, STOP_DET, 0), Ok(State::Active));
                            }
                            while s.drained < s.issued {
                                let byte = expected[usize::from(s.drained)];
                                let level = u32::from(s.issued - s.drained);
                                pop(&mut s, g, level, byte);
                                assert_eq!(s.state(), State::Active);
                            }
                            let stop = if terminal_batch && !stop_first {
                                STOP_DET
                            } else {
                                0
                            };
                            s.observe(g, stop, 0).unwrap();
                        }
                        assert_eq!(command_count, len);
                        assert_eq!(stop_commands, 1);
                        assert_eq!(s.counts(), (len as u8, len as u8, len as u8));
                        assert_eq!(s.state(), State::Complete);
                        assert_eq!(s.bytes(), &expected[..len]);
                        assert!(s.stop_seen());
                        assert_eq!(s.first_mismatch(), None);
                        assert_eq!(s.arm(&[1]), Err(Rejected::NotClean));
                    }
                }
            }
        }
    }

    #[test]
    fn validation_empty_pop_and_full_width_levels() {
        for (rx, tx) in [(0, 32), (32, 0), (257, 32), (32, 257)] {
            assert!(matches!(RxState::new(rx, tx), Err(Rejected::InvalidDepth)));
        }
        let mut s = RxState::new(32, 32).unwrap();
        assert_eq!(s.arm(&[1]), Err(Rejected::NotClean));
        assert_eq!(s.arm(&[]), Err(Rejected::InvalidLength));
        assert_eq!(s.arm(&[0; 34]), Err(Rejected::InvalidLength));
        s.record_cleanup(0, clean()).unwrap();
        let g = s.arm(&[1]).unwrap();
        assert!(s.prepare_pop(g, 0).unwrap().is_none());
        assert_eq!(s.drained, 0);
        assert!(s.prepare_read(g, 32).unwrap().is_none());
        issue(&mut s, g, 0);
        let permit = s.prepare_pop(g, 1).unwrap().unwrap();
        assert_eq!(
            s.record_pop(permit, None),
            Err(Rejected::RequestFailed(Error::EmptyPop))
        );
        assert!(s.bytes().is_empty());
        for bad in [2, 31, 32, 33, 256, u32::MAX] {
            let (mut s, g) = armed(&[1], 32, 32);
            issue(&mut s, g, 0);
            assert!(
                matches!(s.prepare_pop(g, bad), Err(Rejected::RequestFailed(Error::InvalidRxLevel(v))) if v == bad)
            );
            assert!(s.bytes().is_empty());
        }
        let (mut s, g) = armed(&[0; 33], 32, 32);
        for _ in 0..32 {
            issue(&mut s, g, 0);
        }
        assert!(matches!(
            s.prepare_pop(g, 33),
            Err(Rejected::RequestFailed(Error::InvalidRxLevel(33)))
        ));
        for bad in [33, 256, u32::MAX] {
            let (mut s, g) = armed(&[1], 32, 32);
            assert!(
                matches!(s.prepare_read(g, bad), Err(Rejected::RequestFailed(Error::InvalidTxLevel(v))) if v == bad)
            );
            assert_eq!(s.issued, 0);
        }
        let (mut s, g) = armed(&[1], 32, 32);
        assert!(matches!(
            s.prepare_pop(g, 1),
            Err(Rejected::RequestFailed(Error::InvalidRxLevel(1)))
        ));
    }

    #[test]
    fn exact_completion_requires_stop_and_final_error_check() {
        let (mut s, g) = armed(&[0xa5], 32, 32);
        issue(&mut s, g, 0);
        pop(&mut s, g, 1, 0xa5);
        assert_eq!(s.observe(g, 4, 0), Ok(State::Active));
        assert_eq!(s.observe(g, STOP_DET, 0), Ok(State::Complete));
        let (mut s, g) = armed(&[1, 2], 32, 32);
        issue(&mut s, g, 0);
        assert_eq!(
            s.observe(g, STOP_DET, 0),
            Ok(State::Failed(Error::PrematureStop))
        );
        for fatal in [1, 2, 8, 64] {
            for mismatch in [false, true] {
                let (mut s, g) = armed(&[0xa5], 32, 32);
                issue(&mut s, g, 0);
                s.observe(g, STOP_DET, 0).unwrap();
                pop(&mut s, g, 1, if mismatch { 0x5a } else { 0xa5 });
                assert_eq!(
                    s.observe(g, STOP_DET | fatal, 0),
                    Ok(State::Failed(Error::Fatal {
                        causes: fatal,
                        abort_source: 0
                    }))
                );
                assert_eq!(s.drained, 1);
            }
        }
        let (mut s, g) = armed(&[1], 32, 32);
        issue(&mut s, g, 0);
        assert_eq!(
            s.observe(g, STOP_DET, 0x1234),
            Ok(State::Failed(Error::Fatal {
                causes: 0,
                abort_source: 0x1234
            }))
        );
        assert!(s.bytes().is_empty());
        assert!(matches!(s.prepare_pop(g, 1), Err(Rejected::Inactive)));
    }

    #[test]
    fn first_mismatch_and_payload_evidence_survive_failure_cleanup() {
        let (mut s, g) = armed(&[1, 2, 3], 32, 32);
        for _ in 0..3 {
            issue(&mut s, g, 0);
        }
        for (level, byte) in [(3, 1), (2, 8), (1, 9)] {
            pop(&mut s, g, level, byte);
        }
        let mismatch = Some(Mismatch {
            index: 1,
            expected: 2,
            actual: 8,
        });
        assert_eq!(s.first_mismatch(), mismatch);
        assert_eq!(
            s.observe(g, STOP_DET, 0),
            Ok(State::Failed(Error::Mismatch))
        );
        assert_eq!(s.bytes(), &[1, 8, 9]);
        s.record_cleanup(g, clean()).unwrap();
        assert_eq!(s.state(), State::Failed(Error::Mismatch));
        assert_eq!(s.first_mismatch(), mismatch);
    }

    #[test]
    fn terminal_late_events_and_stale_generation_cannot_resurrect() {
        for terminal in [
            State::Complete,
            State::TimedOut,
            State::Failed(Error::Mismatch),
        ] {
            let (mut s, g) = armed(&[1], 32, 32);
            issue(&mut s, g, 0);
            if terminal == State::TimedOut {
                s.timeout(g).unwrap();
            } else {
                pop(
                    &mut s,
                    g,
                    1,
                    if terminal == State::Complete { 1 } else { 2 },
                );
                s.observe(g, STOP_DET, 0).unwrap();
            }
            let counts = s.counts();
            assert_eq!(
                s.observe(g, STOP_DET | FATAL_CAUSES, 1),
                Err(Rejected::Inactive)
            );
            assert_eq!(s.timeout(g), Err(Rejected::Inactive));
            assert!(matches!(s.prepare_read(g, 0), Err(Rejected::Inactive)));
            assert!(matches!(s.prepare_pop(g, 1), Err(Rejected::Inactive)));
            assert_eq!(s.state(), terminal);
            assert_eq!(s.counts(), counts);
            assert_eq!(s.arm(&[2]), Err(Rejected::NotClean));
            s.record_cleanup(g, clean()).unwrap();
            let next = s.arm(&[2]).unwrap();
            assert_eq!(next, g + 1);
            assert_eq!(s.observe(g, STOP_DET, 0), Err(Rejected::StaleGeneration));
            assert_eq!(s.timeout(g), Err(Rejected::StaleGeneration));
            assert!(matches!(
                s.prepare_read(g, 0),
                Err(Rejected::StaleGeneration)
            ));
            assert!(matches!(
                s.prepare_pop(g, 1),
                Err(Rejected::StaleGeneration)
            ));
            assert_eq!(s.record_cleanup(g, clean()), Err(Rejected::StaleGeneration));
            assert_eq!(s.state(), State::Active);
            assert_eq!(s.counts(), (1, 0, 0));
            assert!(!s.stop_seen());
            assert!(s.bytes().is_empty());
            assert_eq!(s.first_mismatch(), None);
        }
    }

    #[test]
    fn failed_cleanup_blocks_rearm_without_overwriting_primary() {
        for field in 0..16 {
            let (mut s, g) = armed(&[1], 32, 32);
            s.timeout(g).unwrap();
            let mut e = clean();
            match field {
                0 => e.enable_status = 1,
                1 => e.interrupt_mask = 1,
                2 => e.rx_level = 1,
                3 => e.tx_level = 1,
                4 => e.raw_status = STOP_DET,
                5 => e.masked_status = 1,
                6 => e.abort_source = 1,
                7 => e.owned_irq_enabled = true,
                8 => e.owned_irq_pending = true,
                9 => e.owned_irq_active = true,
                10 => e.irq_count_after += 1,
                11 => e.quiet_conditions_held = false,
                12 => e.quiet_elapsed_ticks -= 1,
                13 => e.quiet_required_ticks = 0,
                14 => e.raw_status = 4,
                15 => e.raw_status = FATAL_CAUSES,
                _ => unreachable!(),
            }
            assert!(matches!(
                s.record_cleanup(g, e),
                Err(Rejected::CleanupFailed(_))
            ));
            let first_error = s.cleanup_error();
            assert!(first_error.is_some());
            assert_eq!(s.state(), State::TimedOut);
            assert_eq!(s.primary_error(), Some(Error::Timeout));
            assert_eq!(s.arm(&[2]), Err(Rejected::NotClean));
            s.record_cleanup(g, clean()).unwrap();
            assert_eq!(s.cleanup_error(), first_error);
            assert_eq!(s.state(), State::TimedOut);
            assert_eq!(s.arm(&[2]), Ok(g + 1));
            assert_eq!(s.cleanup_error(), None);
            assert_eq!(s.record_cleanup(g + 1, clean()), Err(Rejected::PendingIo));
        }
    }

    #[test]
    fn operations_are_single_use_and_cannot_cross_timeout_or_generation() {
        let (mut s, g) = armed(&[1], 32, 32);
        let command = s.prepare_read(g, 0).unwrap().unwrap();
        assert_eq!(s.issued, 0);
        assert!(matches!(s.prepare_read(g, 0), Err(Rejected::PendingIo)));
        assert!(matches!(s.prepare_pop(g, 0), Err(Rejected::PendingIo)));
        s.timeout(g).unwrap();
        s.record_cleanup(g, clean()).unwrap();
        let next = s.arm(&[1]).unwrap();
        assert_eq!(s.record_issued(command), Err(Rejected::StaleGeneration));
        issue(&mut s, next, 0);
        let permit = s.prepare_pop(next, 1).unwrap().unwrap();
        assert!(matches!(s.prepare_pop(next, 1), Err(Rejected::PendingIo)));
        s.timeout(next).unwrap();
        assert_eq!(s.record_pop(permit, Some(1)), Err(Rejected::Inactive));
        assert!(s.bytes().is_empty());
        s.record_cleanup(next, clean()).unwrap();
        s.generation = u32::MAX; // exercise the production checked_add boundary
        assert_eq!(s.arm(&[1]), Err(Rejected::GenerationExhausted));
        assert_eq!(s.generation(), u32::MAX);
        assert_eq!(s.state(), State::TimedOut);
        assert!(s.is_clean());
    }

    #[test]
    fn cleanup_failure_is_not_receive_success_or_a_primary_rewrite() {
        let (mut s, g) = armed(&[1], 32, 32);
        issue(&mut s, g, 0);
        pop(&mut s, g, 1, 1);
        assert_eq!(s.observe(g, STOP_DET, 0), Ok(State::Complete));
        let mut dirty = clean();
        dirty.rx_level = 1;
        assert_eq!(
            s.record_cleanup(g, dirty),
            Err(Rejected::CleanupFailed(CleanupError::FifoNotEmpty))
        );
        assert_eq!(s.state(), State::Complete); // protocol only, not clean/HW PASS
        assert_eq!(s.primary_error(), None);
        assert_eq!(s.cleanup_error(), Some(CleanupError::FifoNotEmpty));
        assert_eq!(s.arm(&[2]), Err(Rejected::NotClean));

        let (mut s, g) = armed(&[1], 32, 32);
        let uncommitted = s.prepare_read(g, 0).unwrap().unwrap();
        assert_eq!(s.observe(g, 0, 0), Ok(State::Failed(Error::UncommittedIo)));
        assert_eq!(s.record_issued(uncommitted), Err(Rejected::Inactive));
        assert_eq!(s.counts(), (1, 0, 0));
    }

    #[test]
    fn adapter_fault_invalidates_pending_tokens_and_recovers_without_rewriting_failure() {
        for pending in [Pending::None, Pending::Read, Pending::Pop] {
            for reject_after_rearm in [false, true] {
                let (mut s, g) = armed(&[0xa5, 0x5a], 32, 32);
                issue(&mut s, g, 0);
                pop(&mut s, g, 1, 0xa5);
                let (command, permit) = match pending {
                    Pending::None => (None, None),
                    Pending::Read => (s.prepare_read(g, 0).unwrap(), None),
                    Pending::Pop => {
                        issue(&mut s, g, 0);
                        s.observe(g, STOP_DET, 0).unwrap();
                        (None, s.prepare_pop(g, 1).unwrap())
                    }
                };
                let before = (s.counts(), s.bytes, s.stop_seen(), s.first_mismatch());
                assert_eq!(s.fail_adapter(g), Ok(()));
                assert_eq!(s.state(), State::Failed(Error::Adapter));
                assert_eq!(s.primary_error(), Some(Error::Adapter));
                assert_eq!(s.pending, Pending::None);
                assert_eq!(
                    (s.counts(), s.bytes, s.stop_seen(), s.first_mismatch()),
                    before
                );
                assert_eq!(s.observe(g, STOP_DET, 0), Err(Rejected::Inactive));
                assert_eq!(s.timeout(g), Err(Rejected::Inactive));
                assert!(!s.is_clean());
                assert_eq!(s.arm(&[0x3c]), Err(Rejected::NotClean));

                let mut dirty = clean();
                dirty.enable_status = 1;
                assert_eq!(
                    s.record_cleanup(g, dirty),
                    Err(Rejected::CleanupFailed(CleanupError::NotDisabled))
                );
                assert_eq!(s.arm(&[0x3c]), Err(Rejected::NotClean));
                s.record_cleanup(g, clean()).unwrap();
                assert!(s.is_clean());
                assert_eq!(s.state(), State::Failed(Error::Adapter));
                assert_eq!(s.primary_error(), Some(Error::Adapter));
                assert_eq!(s.cleanup_error(), Some(CleanupError::NotDisabled));
                assert_eq!(s.bytes(), &[0xa5]);

                if reject_after_rearm {
                    assert_eq!(s.arm(&[0x3c]), Ok(g + 1));
                }
                let rejection = if reject_after_rearm {
                    Rejected::StaleGeneration
                } else {
                    Rejected::Inactive
                };
                if let Some(command) = command {
                    assert_eq!(s.record_issued(command), Err(rejection));
                }
                if let Some(permit) = permit {
                    assert_eq!(s.record_pop(permit, Some(0x5a)), Err(rejection));
                }
                if !reject_after_rearm {
                    assert_eq!(s.counts(), before.0);
                    assert_eq!(s.bytes(), &[0xa5]);
                    assert_eq!(s.arm(&[0x3c]), Ok(g + 1));
                }
                assert_eq!(s.counts(), (1, 0, 0));
                assert!(s.bytes().is_empty());
                assert!(!s.stop_seen());
                assert_eq!(s.first_mismatch(), None);
                assert_eq!(s.primary_error(), None);
                assert_eq!(s.cleanup_error(), None);
                assert_eq!(issue(&mut s, g + 1, 0), 0x300);
                pop(&mut s, g + 1, 1, 0x3c);
                assert_eq!(s.observe(g + 1, STOP_DET, 0), Ok(State::Complete));
                assert_eq!(s.bytes(), &[0x3c]);
                assert_eq!(s.counts(), (1, 1, 1));
            }
        }
    }

    #[test]
    fn adapter_fault_rejects_wrong_generation_and_inactive_without_any_mutation() {
        // Every field, including pending tokens and hidden buffer tails.
        let snapshot = |s: &RxState| {
            (
                (
                    s.rx_depth,
                    s.tx_depth,
                    s.generation,
                    s.state,
                    s.clean,
                    s.cleanup_error,
                ),
                (
                    s.requested,
                    s.issued,
                    s.drained,
                    s.stop_seen,
                    s.pending,
                    s.expected,
                    s.bytes,
                    s.first_mismatch,
                ),
            )
        };
        for case in 0..9 {
            let mut s = RxState::new(32, 32).unwrap();
            s.record_cleanup(0, clean()).unwrap();
            if case != 0 {
                let g = s.arm(&[0xa5]).unwrap();
                match case {
                    1 => {}
                    2 => {
                        s.prepare_read(g, 0).unwrap().unwrap();
                    }
                    3 => {
                        issue(&mut s, g, 0);
                        s.prepare_pop(g, 1).unwrap().unwrap();
                    }
                    4 | 5 => {
                        issue(&mut s, g, 0);
                        pop(&mut s, g, 1, if case == 4 { 0xa5 } else { 0x5a });
                        s.observe(g, STOP_DET, 0).unwrap();
                    }
                    6 => s.timeout(g).unwrap(),
                    7 => s.fail_adapter(g).unwrap(),
                    8 => {
                        s.observe(g, STOP_DET | FATAL_CAUSES, 0x1234).unwrap();
                    }
                    _ => unreachable!(),
                }
                if s.state() != State::Active {
                    let mut dirty = clean();
                    dirty.interrupt_mask = 1;
                    assert_eq!(
                        s.record_cleanup(g, dirty),
                        Err(Rejected::CleanupFailed(CleanupError::NotMasked))
                    );
                    s.record_cleanup(g, clean()).unwrap();
                }
            }
            let g = s.generation();
            let before = snapshot(&s);
            for wrong in [g.wrapping_sub(1), g + 1, u32::MAX] {
                assert_eq!(s.fail_adapter(wrong), Err(Rejected::StaleGeneration));
                assert_eq!(snapshot(&s), before);
            }
            if s.state() == State::Active {
                assert_eq!(s.fail_adapter(g), Ok(()));
                assert_eq!(s.state(), State::Failed(Error::Adapter));
                assert_eq!(s.pending, Pending::None);
            } else {
                assert_eq!(s.fail_adapter(g), Err(Rejected::Inactive));
                assert_eq!(snapshot(&s), before);
            }
        }
    }
}

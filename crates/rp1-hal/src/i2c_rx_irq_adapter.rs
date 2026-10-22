//! Bounded service on the existing I2c1Host and exact RX engine, not a lifecycle.
//!
//! Caller owns one proc0 host/engine, constructed from the same observed depths.
//! It establishes real clean/quiet evidence, arms the engine and peripheral with
//! arm_rx_irq_preserving_causes, and enables IRQ8 BEFORE the initial command.
//! Caller serializes the ENTIRE call against the same host/engine's owned IRQ,
//! including every prepare/access/record. Under an RTOS, mask that owned IRQ
//! and keep one task owner; do not replace global PRIMASK/VTOR or other IRQ state.
//! service_irq is ISR-only: actual IPSR/route verification,
//! NVIC gating, deadlines, total entry/no-progress limits and terminal cleanup
//! remain caller obligations. Neither function re-arms, enables IRQs, or cleans.
//! Terminal/late snapshots are NOT acknowledged: cleanup must first retain its
//! pre-disable and post-disable/pre-ack evidence. Active STOP is selectively acked.
//! A stale-generation call masks the peripheral even if another generation is
//! ACTIVE: this is fail-closed caller-required recovery, never successful service.
use crate::i2c::{self, I2c1Host, I2c1Read1Snapshot};
use crate::i2c_rx_state::{FATAL_CAUSES, OWNED_CAUSES, Rejected, RxState, State, STOP_DET};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterFault {
    Control,
    ObservedGeometry,
}

/// Fixed first/error/final evidence, not the future two-request snapshot ABI.
/// Retain this before the next call. Fatal evidence outranks payload success;
/// late fatal cannot rewrite an already terminal engine and remains independent.
#[derive(Debug)]
pub struct ServiceEvidence {
    pub entry: I2c1Read1Snapshot,
    pub last: I2c1Read1Snapshot,
    pub first_stop: Option<I2c1Read1Snapshot>,
    pub first_fatal: Option<I2c1Read1Snapshot>,
    pub post_pop: Option<I2c1Read1Snapshot>,
    pub last_word: Option<u32>,
    pub rx_depth: u32,
    pub tx_depth: u32,
    pub queued: u16,
    pub pop_attempts: u16,
    pub popped: u16,
    pub acknowledged: u32,
    pub rejected: Option<Rejected>,
    pub fault: Option<AdapterFault>,
}

impl ServiceEvidence {
    fn new() -> Self {
        let entry = i2c::i2c1_read1_snapshot();
        let mut result = Self {
            entry, last: entry, first_stop: None, first_fatal: None,
            post_pop: None, last_word: None, rx_depth: 0, tx_depth: 0,
            queued: 0, pop_attempts: 0, popped: 0, acknowledged: 0,
            rejected: None, fault: None,
        };
        result.save(entry);
        result
    }

    fn save(&mut self, snapshot: I2c1Read1Snapshot) {
        let causes = snapshot.irq.raw_interrupt_status | snapshot.irq.masked_interrupt_status;
        if causes & STOP_DET != 0 { self.first_stop.get_or_insert(snapshot); }
        if causes & FATAL_CAUSES != 0 || snapshot.irq.abort_source != 0 {
            self.first_fatal.get_or_insert(snapshot);
        }
        self.last = snapshot;
    }

    fn observe(&mut self, engine: &mut RxState, generation: u32,
               snapshot: I2c1Read1Snapshot) -> bool {
        self.save(snapshot); // Source/abort retained before any selective ack.
        let causes = snapshot.irq.raw_interrupt_status | snapshot.irq.masked_interrupt_status;
        // Fatal keeps engine priority. Otherwise reject control faults BEFORE
        // observe could irreversibly complete on the last byte/STOP join.
        if engine.state() == State::Active && engine.generation() == generation
            && causes & FATAL_CAUSES == 0 && snapshot.irq.abort_source == 0
            && (snapshot.irq.interrupt_mask != OWNED_CAUSES
                || snapshot.irq.enable_status != 1 || snapshot.rx_threshold != 0) {
            self.fail(engine, generation, AdapterFault::Control);
            return false;
        }
        match engine.observe(generation, causes, snapshot.irq.abort_source) {
            Ok(State::Active) => true,
            Ok(_) => false,
            Err(error) => { self.rejected.get_or_insert(error); false }
        }
    }

    fn fail(&mut self, engine: &mut RxState, generation: u32, fault: AdapterFault) {
        self.fault.get_or_insert(fault);
        if let Err(error) = engine.fail_adapter(generation) {
            self.rejected.get_or_insert(error);
        }
    }

    fn begin(&mut self, host: &I2c1Host, engine: &mut RxState, generation: u32) -> bool {
        if !self.observe(engine, generation, self.entry) { return false; }
        let hardware = host.snapshot(0, 0);
        self.rx_depth = u32::from(i2c::i2c1_rx_fifo_depth(hardware.component_parameter));
        self.tx_depth = u32::from(hardware.tx_fifo_depth);
        if !(1..=256).contains(&self.tx_depth) {
            self.fail(engine, generation, AdapterFault::ObservedGeometry);
            return false;
        }
        true
    }

    fn finish(&mut self, engine: &RxState) {
        if engine.state() != State::Active || self.rejected.is_some() || self.fault.is_some() {
            i2c::i2c1_mask_read1_irq();
        } else {
            // Exact saved, observed causes only. RX_FULL is not a clear register.
            self.acknowledged = i2c::i2c1_ack_read1_irq(self.last);
        }
    }
}

fn queue(host: &mut I2c1Host, engine: &mut RxState, generation: u32,
         evidence: &mut ServiceEvidence) {
    for _ in 0..evidence.tx_depth {
        let snapshot = i2c::i2c1_read1_snapshot();
        if !evidence.observe(engine, generation, snapshot) { return; }
        let (requested, issued, drained) = engine.counts();
        let outstanding = u32::from(issued - drained);
        if snapshot.tx_level > evidence.tx_depth || outstanding > evidence.rx_depth
            || snapshot.rx_level > evidence.rx_depth.min(outstanding)
                .min(u32::from(requested - drained)) {
            evidence.fail(engine, generation, AdapterFault::ObservedGeometry);
            return;
        }
        if snapshot.tx_level == evidence.tx_depth || outstanding == evidence.rx_depth { break; }
        match engine.prepare_read(generation, snapshot.tx_level) {
            Ok(Some(command)) => {
                host.issue_read_command(&command);
                let recorded = engine.record_issued(command); // No intervening observe/access.
                evidence.queued += 1; // Actual submissions, not bus ACKs.
                if let Err(error) = recorded {
                    evidence.rejected.get_or_insert(error);
                    return;
                }
            }
            Ok(None) => break,
            Err(error) => { evidence.rejected.get_or_insert(error); return; }
        }
    }
    evidence.observe(engine, generation, i2c::i2c1_read1_snapshot());
}

/// Foreground prime queues bounded READ commands and NEVER pops DATA_CMD.
pub fn prime(host: &mut I2c1Host, engine: &mut RxState, generation: u32) -> ServiceEvidence {
    let mut evidence = ServiceEvidence::new();
    if evidence.begin(host, engine, generation) {
        queue(host, engine, generation, &mut evidence);
    }
    evidence.finish(engine);
    evidence
}

/// Call ONLY from the validated owned ISR under the documented serialization.
/// Each entry has independent observed RX-depth pop and TX-depth queue bounds.
pub fn service_irq(host: &mut I2c1Host, engine: &mut RxState, generation: u32) -> ServiceEvidence {
    let mut evidence = ServiceEvidence::new();
    if evidence.begin(host, engine, generation) {
        for _ in 0..evidence.rx_depth {
            let snapshot = i2c::i2c1_read1_snapshot();
            if !evidence.observe(engine, generation, snapshot) { break; }
            let (requested, issued, drained) = engine.counts();
            let max_level = evidence.rx_depth.min(u32::from(issued - drained))
                .min(u32::from(requested - drained));
            if snapshot.rx_level > evidence.rx_depth {
                evidence.fail(engine, generation, AdapterFault::ObservedGeometry);
                break;
            }
            match engine.prepare_pop(generation, snapshot.rx_level) {
                Ok(Some(permit)) => {
                    let actual = host.pop_rx_one(max_level);
                    let recorded = engine.record_pop(permit, actual.map(|word| word as u8));
                    evidence.pop_attempts += 1;
                    if let Some(word) = actual {
                        evidence.popped += 1;
                        evidence.last_word = Some(word);
                    }
                    // Even failed guard/record retains real post-attempt causes.
                    let post = i2c::i2c1_read1_snapshot();
                    evidence.post_pop = Some(post);
                    let active = evidence.observe(engine, generation, post);
                    if let Err(error) = recorded {
                        evidence.rejected = Some(error);
                        break;
                    }
                    if !active { break; }
                }
                Ok(None) => break,
                Err(error) => { evidence.rejected.get_or_insert(error); break; }
            }
        }
        if engine.state() == State::Active && evidence.rejected.is_none() && evidence.fault.is_none() {
            queue(host, engine, generation, &mut evidence);
        }
    }
    evidence.finish(engine);
    evidence
}

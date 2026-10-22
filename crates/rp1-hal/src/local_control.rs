//! Single-owner control state in RP1 local memory. No DDR, allocation or MMIO.
//! The application applies its resource's safe output whenever `step` is inactive.
use rp1_abi::amp::{Command, CommandGate, Error, Generation, RECORD_BYTES, VERSION};

pub const SET_PARAMETERS: u32 = 8;
pub const STOP: u32 = 9;
pub const PAYLOAD_MAGIC: u32 = u32::from_le_bytes(*b"CTL1");

/// CTL1 payload: magic, version, parameter count (4 for SET, 0 for STOP),
/// flags=0, four application-defined words, then eight reserved zero words.
/// The application must validate the meanings/ranges of all four words before
/// accepting a message. This type grants no peripheral or address capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parameters(pub [u32; 4]);

/// A checked local copy, suitable for an SPSC queue. It grants no MMIO access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlUpdate {
    generation: Generation,
    sequence: u64,
    apply: u64,
    until: u64,
    parameters: Parameters,
    stop: bool,
}

impl ControlUpdate {
    pub fn decode(packet: &[u8], generation: Generation, now: u64,
        validate: impl FnOnce(Parameters) -> Result<(), Error>) -> Result<Self, Error> {
        if packet.len() != 2 * RECORD_BYTES { return Err(Error::Length); }
        let command = Command::decode(packet)?;
        let w = core::array::from_fn::<_, 16, _>(|i| u32::from_le_bytes(
            packet[RECORD_BYTES+i*4..RECORD_BYTES+i*4+4].try_into().unwrap()));
        let stop = command.0[12] == STOP;
        if command.0[12] != SET_PARAMETERS && !stop { return Err(Error::Owner); }
        if w[0] != PAYLOAD_MAGIC || w[1] != VERSION { return Err(Error::Version); }
        if w[2] != if stop { 0 } else { 4 } { return Err(Error::Length); }
        if w[3] != 0 || w[8..].iter().any(|x| *x != 0) { return Err(Error::Flags); }
        let parameters = Parameters(core::array::from_fn(|i| w[4+i]));
        if stop {
            if parameters.0 != [0;4] { return Err(Error::Flags); }
            if command.apply_time() > now { return Err(Error::Time); }
        } else { validate(parameters)?; }
        CommandGate::new(generation)?.accept(command, RECORD_BYTES, now)?;
        Ok(Self {generation,sequence:command.sequence(),apply:command.apply_time(),until:command.valid_until(),parameters,stop})
    }
    pub const fn sequence(self) -> u64 { self.sequence }
    fn command(self) -> Command {
        let generation = self.generation;
        Command([VERSION,64,generation.boot as u32,(generation.boot>>32) as u32,
            generation.session as u32,(generation.session>>32) as u32,self.sequence as u32,(self.sequence>>32) as u32,
            self.apply as u32,(self.apply>>32) as u32,self.until as u32,(self.until>>32) as u32,
            if self.stop {STOP} else {SET_PARAMETERS},64,0,0])
    }
}

#[derive(Clone, Copy)]
struct Target {
    sequence: u64,
    apply: u64,
    until: u64,
    parameters: Parameters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleStamp {
    pub boot: u64,
    pub sequence: u64,
    pub acquired_us: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inactive {
    NoCommand,
    CommandExpired,
    SampleGeneration,
    SampleSequence,
    SampleTime,
    SampleStale,
    Retired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Active {
        sequence: u64,
        parameters: Parameters,
    },
    Inactive(Inactive),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub accepted: u64,
    pub applied: u64,
    pub rejected: u64,
    pub queue_full: u64,
    pub command_expired: u64,
    pub sample_sequence_gaps: u64,
    pub sample_duplicate: u64,
    pub sample_reordered: u64,
    pub sample_rejected: u64,
    pub failsafe_entries: u64,
}

pub struct LocalControl {
    generation: Generation,
    gate: CommandGate,
    current: Option<Target>,
    next: Option<Target>,
    last_sample: u64,
    last_time: Option<u64>,
    max_sample_age: u64,
    retired: bool,
    active: bool,
    counters: Counters,
}

impl LocalControl {
    /// Construct once for the admitted cold boot/session. Recreating this value
    /// is not a restart/reconnect contract. A retired instance never reopens.
    pub fn new(generation: Generation, max_sample_age_us: u64) -> Result<Self, Error> {
        if max_sample_age_us == 0 {
            return Err(Error::Time);
        }
        Ok(Self {
            generation,
            gate: CommandGate::new(generation)?,
            current: None,
            next: None,
            last_sample: 0,
            last_time: None,
            max_sample_age: max_sample_age_us,
            retired: false,
            active: false,
            counters: Counters::default(),
        })
    }
    pub fn counters(&self) -> Counters {
        self.counters
    }
    pub fn last_sequence(&self) -> u64 {
        self.gate.last_sequence()
    }
    pub fn retire(&mut self) {
        self.retired = true;
        self.current = None;
        self.next = None;
    }

    fn observe_time(&mut self, now: u64) -> bool {
        if self.last_time.is_some_and(|t| now < t) {
            self.retire();
            return false;
        }
        self.last_time = Some(now);
        true
    }

    /// Copy an entire validated parameter update; never partially modify current
    /// parameters. One pending target: drop-newest with an explicit Full result.
    /// STOP is immediate, clears both targets and does not wait for the queue.
    /// `validate` is the application's range/schema check, performed before commit.
    pub fn offer(
        &mut self,
        packet: &[u8],
        now: u64,
        validate: impl FnOnce(Parameters) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let result = self.offer_checked(packet, now, validate);
        if result.is_err() {
            self.counters.rejected = self.counters.rejected.saturating_add(1);
        }
        result
    }

    fn offer_checked(
        &mut self,
        packet: &[u8],
        now: u64,
        validate: impl FnOnce(Parameters) -> Result<(), Error>,
    ) -> Result<(), Error> {
        if self.retired {
            return Err(Error::Generation);
        }
        if !self.observe_time(now) {
            return Err(Error::Time);
        }
        let update = ControlUpdate::decode(packet, self.generation, now, validate)?;
        self.commit(update, now)
    }

    /// Recheck expiry/sequence when the realtime owner dequeues; queue acceptance
    /// is not application. The original bound generation stays with this owner.
    pub fn offer_update(&mut self, update: ControlUpdate, now: u64) -> Result<(), Error> {
        let result = if self.retired { Err(Error::Generation) }
            else if !self.observe_time(now) { Err(Error::Time) }
            else { self.commit(update, now) };
        if result.is_err() { self.counters.rejected = self.counters.rejected.saturating_add(1); }
        result
    }

    fn commit(&mut self, update: ControlUpdate, now: u64) -> Result<(), Error> {
        let command = update.command();
        let mut gate = self.gate;
        gate.accept(command, RECORD_BYTES, now)?;
        if !update.stop && self.next.is_some() {
            self.counters.queue_full = self.counters.queue_full.saturating_add(1);
            return Err(Error::Full);
        }
        // Every header and application check has passed. No fallible operation follows.
        self.gate = gate;
        self.counters.accepted = self.counters.accepted.saturating_add(1);
        if update.stop {
            self.current = None;
            self.next = None;
        } else {
            self.next = Some(Target {
                sequence: command.sequence(),
                apply: command.apply_time(),
                until: command.valid_until(),
                parameters: update.parameters,
            });
        }
        Ok(())
    }

    fn inactive(&mut self, reason: Inactive) -> Decision {
        if self.active {
            self.counters.failsafe_entries = self.counters.failsafe_entries.saturating_add(1);
        }
        self.active = false;
        Decision::Inactive(reason)
    }

    /// Called by the local realtime owner, independent of Linux collection and
    /// DDR progress. Timestamps refer to RP1 acquisition, not Linux arrival.
    pub fn step(&mut self, now: u64, sample: SampleStamp) -> Decision {
        if !self.observe_time(now) || self.retired {
            return self.inactive(Inactive::Retired);
        }
        if let Some(next) = self.next {
            if now >= next.until {
                self.next = None;
                self.counters.command_expired = self.counters.command_expired.saturating_add(1);
            } else if now >= next.apply {
                self.current = self.next.take();
                self.counters.applied = self.counters.applied.saturating_add(1);
            }
        }
        let expired = self.current.is_some_and(|c| now >= c.until);
        if expired {
            self.current = None;
            self.counters.command_expired = self.counters.command_expired.saturating_add(1);
        }
        let reason = if sample.boot != self.generation.boot {
            Some(Inactive::SampleGeneration)
        } else if sample.sequence == 0 || sample.sequence <= self.last_sample {
            if sample.sequence == self.last_sample && sample.sequence != 0 {
                self.counters.sample_duplicate = self.counters.sample_duplicate.saturating_add(1);
            } else {
                self.counters.sample_reordered = self.counters.sample_reordered.saturating_add(1);
            }
            Some(Inactive::SampleSequence)
        } else if sample.acquired_us > now {
            Some(Inactive::SampleTime)
        } else if now - sample.acquired_us > self.max_sample_age {
            Some(Inactive::SampleStale)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.counters.sample_rejected = self.counters.sample_rejected.saturating_add(1);
            return self.inactive(reason);
        }
        if self.last_sample != 0 {
            self.counters.sample_sequence_gaps = self
                .counters
                .sample_sequence_gaps
                .saturating_add(sample.sequence - self.last_sample - 1);
        }
        self.last_sample = sample.sequence;
        if expired {
            return self.inactive(Inactive::CommandExpired);
        }
        if let Some(c) = self.current {
            self.active = true;
            Decision::Active {
                sequence: c.sequence,
                parameters: c.parameters,
            }
        } else {
            self.inactive(Inactive::NoCommand)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn controller() -> LocalControl {
        LocalControl::new(Generation::new(17, 29).unwrap(), 200).unwrap()
    }
    fn packet(
        sequence: u64,
        apply: u64,
        until: u64,
        parameters: [u32; 4],
        stop: bool,
    ) -> [u8; 128] {
        let mut w = [0u32; 32];
        w[0] = VERSION;
        w[1] = 64;
        w[2] = 17;
        w[4] = 29;
        w[6] = sequence as u32;
        w[7] = (sequence >> 32) as u32;
        w[8] = apply as u32;
        w[9] = (apply >> 32) as u32;
        w[10] = until as u32;
        w[11] = (until >> 32) as u32;
        w[12] = if stop { STOP } else { SET_PARAMETERS };
        w[13] = 64;
        w[16] = PAYLOAD_MAGIC;
        w[17] = VERSION;
        w[18] = if stop { 0 } else { 4 };
        w[20..24].copy_from_slice(&parameters);
        let mut b = [0; 128];
        for (dst, v) in b.chunks_exact_mut(4).zip(w) {
            dst.copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    fn sample(sequence: u64, now: u64) -> SampleStamp {
        SampleStamp {
            boot: 17,
            sequence,
            acquired_us: now,
        }
    }
    #[test]
    fn queued_updates_recheck_generation_expiry_sequence_and_all_parameters() {
        let generation=Generation{boot:17,session:29};
        let update=ControlUpdate::decode(&packet(1,300,1000,[1,2,3,4],false),generation,100,|_|Ok(())).unwrap();
        assert!(core::mem::size_of::<ControlUpdate>()<=64);
        let mut q=crate::local_queue::LocalQueue::<ControlUpdate,1>::new();
        let (mut p,mut r)=q.split();p.push(update).unwrap();
        assert_eq!(p.push(update),Err(Error::Full));assert_eq!(p.drops(),1);
        let mut c=controller();c.offer_update(r.pop().unwrap(),200).unwrap();
        assert_eq!(c.step(200,sample(1,200)),Decision::Inactive(Inactive::NoCommand));
        active(&mut c,300,2,[1,2,3,4],1);
        assert_eq!(c.offer_update(update,300),Err(Error::Sequence));
        assert_eq!(controller().offer_update(update,1000),Err(Error::Expired));
        let mut other=LocalControl::new(Generation{boot:18,session:29},200).unwrap();
        assert_eq!(other.offer_update(update,200),Err(Error::Generation));
        let mut other=LocalControl::new(Generation{boot:17,session:30},200).unwrap();
        assert_eq!(other.offer_update(update,200),Err(Error::Generation));
        c.retire();assert_eq!(c.offer_update(update,400),Err(Error::Generation));
    }
    fn active(c: &mut LocalControl, now: u64, n: u64, p: [u32; 4], sequence: u64) {
        assert_eq!(
            c.step(now, sample(n, now)),
            Decision::Active {
                sequence,
                parameters: Parameters(p)
            }
        );
    }
    #[test]
    fn future_atomic_update_and_immediate_stop() {
        let mut c = controller();
        let a = [1, 2, 3, 4];
        let b = [5, 6, 7, 8];
        c.offer(&packet(1, 100, 300, a, false), 0, |_| Ok(()))
            .unwrap();
        assert_eq!(
            c.step(99, sample(1, 99)),
            Decision::Inactive(Inactive::NoCommand)
        );
        active(&mut c, 100, 2, a, 1);
        c.offer(&packet(2, 200, 400, b, false), 101, |_| Ok(()))
            .unwrap();
        active(&mut c, 199, 3, a, 1);
        active(&mut c, 200, 4, b, 2);
        c.offer(&packet(3, 210, 500, a, false), 201, |_| Ok(()))
            .unwrap();
        c.offer(&packet(4, 202, 500, [0; 4], true), 202, |_| panic!())
            .unwrap();
        assert_eq!(
            c.step(211, sample(5, 211)),
            Decision::Inactive(Inactive::NoCommand)
        );
        assert_eq!(c.counters().applied, 2);
    }
    #[test]
    fn full_is_explicit_no_overwrite_or_sequence_consumption() {
        let mut c = controller();
        c.offer(&packet(1, 100, 1000, [1; 4], false), 0, |_| Ok(()))
            .unwrap();
        assert_eq!(
            c.offer(&packet(2, 100, 1000, [2; 4], false), 1, |_| Ok(())),
            Err(Error::Full)
        );
        assert_eq!(c.last_sequence(), 1);
        assert_eq!(c.counters().queue_full, 1);
        active(&mut c, 100, 1, [1; 4], 1);
        c.offer(&packet(2, 101, 1000, [2; 4], false), 101, |_| Ok(()))
            .unwrap();
        active(&mut c, 101, 2, [2; 4], 2);
    }
    #[test]
    fn rejected_header_payload_or_application_never_changes_parameters() {
        for (word, value) in [
            (0usize, 2u32),
            (1, 63),
            (2, 18),
            (4, 30),
            (13, 63),
            (14, 1),
            (15, 1),
            (16, 0),
            (17, 2),
            (18, 3),
            (19, 1),
            (24, 1),
            (12, 55),
        ] {
            let mut c = controller();
            c.offer(&packet(1, 0, 1000, [1; 4], false), 0, |_| Ok(()))
                .unwrap();
            active(&mut c, 0, 1, [1; 4], 1);
            let mut bad = packet(2, 1, 1000, [2; 4], false);
            bad[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
            assert!(c.offer(&bad, 1, |_| Ok(())).is_err());
            assert_eq!(c.last_sequence(), 1);
            active(&mut c, 2, 2, [1; 4], 1);
        }
        let mut c = controller();
        assert_eq!(
            c.offer(&packet(1, 0, 1000, [1; 4], false), 0, |_| Err(
                Error::Bounds
            )),
            Err(Error::Bounds)
        );
        assert_eq!(c.last_sequence(), 0);
        for n in [0, 63, 64, 127] {
            assert_eq!(c.offer(&[0; 128][..n], 0, |_| Ok(())), Err(Error::Length));
        }
    }
    #[test]
    fn duplicate_reorder_expiry_and_retirement() {
        let mut c = controller();
        let p = packet(3, 0, 100, [1; 4], false);
        c.offer(&p, 0, |_| Ok(())).unwrap();
        active(&mut c, 0, 1, [1; 4], 3);
        for seq in [3, 2, 0] {
            assert_eq!(
                c.offer(&packet(seq, 1, 100, [2; 4], false), 1, |_| Ok(())),
                Err(Error::Sequence)
            );
        }
        assert_eq!(
            c.step(100, sample(2, 100)),
            Decision::Inactive(Inactive::CommandExpired)
        );
        assert_eq!(c.counters().command_expired, 1);
        assert_eq!(
            c.offer(&packet(4, 99, 100, [2; 4], false), 100, |_| Ok(())),
            Err(Error::Expired)
        );
        c.retire();
        assert_eq!(
            c.offer(&packet(4, 100, 200, [2; 4], false), 100, |_| Ok(())),
            Err(Error::Generation)
        );
        assert_eq!(
            c.step(101, sample(3, 101)),
            Decision::Inactive(Inactive::Retired)
        );
    }
    #[test]
    fn sensor_integrity_and_age_force_safe_output() {
        let mut c = controller();
        c.offer(&packet(1, 0, 10000, [1; 4], false), 0, |_| Ok(()))
            .unwrap();
        active(&mut c, 10, 1, [1; 4], 1);
        assert_eq!(
            c.step(11, sample(1, 11)),
            Decision::Inactive(Inactive::SampleSequence)
        );
        active(&mut c, 12, 4, [1; 4], 1);
        assert_eq!(c.counters().sample_sequence_gaps, 2);
        let mut s = sample(5, 13);
        s.boot = 18;
        assert_eq!(
            c.step(13, s),
            Decision::Inactive(Inactive::SampleGeneration)
        );
        assert_eq!(
            c.step(14, sample(5, 15)),
            Decision::Inactive(Inactive::SampleTime)
        );
        assert_eq!(
            c.step(215, sample(5, 14)),
            Decision::Inactive(Inactive::SampleStale)
        );
        active(&mut c, 216, 5, [1; 4], 1);
        assert_eq!(c.counters().sample_duplicate, 1);
    }
    #[test]
    fn clock_regression_permanently_retires_and_expired_pending_never_applies() {
        let mut c = controller();
        c.offer(&packet(1, 100, 200, [1; 4], false), 1, |_| Ok(()))
            .unwrap();
        assert_eq!(
            c.step(200, sample(1, 200)),
            Decision::Inactive(Inactive::NoCommand)
        );
        assert_eq!(c.counters().applied, 0);
        assert_eq!(
            c.step(199, sample(2, 199)),
            Decision::Inactive(Inactive::Retired)
        );
        assert_eq!(
            c.offer(&packet(2, 201, 300, [2; 4], false), 201, |_| Ok(())),
            Err(Error::Generation)
        );
    }
    #[test]
    fn expired_command_does_not_discard_an_acquired_sample() {
        let mut c = controller();
        c.offer(&packet(1, 0, 100, [1; 4], false), 0, |_| Ok(()))
            .unwrap();
        active(&mut c, 99, 1, [1; 4], 1);
        assert_eq!(
            c.step(100, sample(2, 100)),
            Decision::Inactive(Inactive::CommandExpired)
        );
        assert_eq!(
            c.step(101, sample(3, 101)),
            Decision::Inactive(Inactive::NoCommand)
        );
        c.offer(&packet(2, 102, 200, [2; 4], false), 102, |_| Ok(()))
            .unwrap();
        active(&mut c, 102, 4, [2; 4], 2);
        assert_eq!(c.counters().sample_sequence_gaps, 0);
    }
    #[test]
    fn stalled_trace_collector_does_not_block_local_decisions_or_expiry() {
        // Simulated timestamps exercise backpressure, not hardware 5 kHz timing.
        let mut c = controller();
        c.offer(&packet(1, 0, 2_000_200, [1; 4], false), 0, |_| Ok(()))
            .unwrap();
        let mut queue = crate::local_queue::LocalQueue::<[u64; 2], 8>::new();
        let (mut producer, mut consumer) = queue.split();
        for seq in 1..=10_000 {
            let now = seq * 200;
            active(&mut c, now, seq, [1; 4], 1);
            assert_eq!(
                producer.push([seq, now]),
                if seq <= 8 { Ok(()) } else { Err(Error::Full) }
            );
        }
        assert_eq!(producer.drops(), 9_992);
        assert_eq!(c.counters().sample_sequence_gaps, 0);
        assert_eq!(c.counters().failsafe_entries, 0);
        assert_eq!(
            c.step(2_000_200, sample(10_001, 2_000_200)),
            Decision::Inactive(Inactive::CommandExpired)
        );
        assert_eq!(c.counters().failsafe_entries, 1);
        for seq in 1..=8 {
            assert_eq!(consumer.pop(), Some([seq, seq * 200]));
        }
        assert_eq!(consumer.pop(), None);
    }
}

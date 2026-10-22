//! C0-owned OBS1 collector. C1 only uses its local-SRAM Producer; it never calls poll.
use super::*;
use crate::local_queue::Consumer;
use rp1_abi::{
    amp::VERSION,
    observation::{BYTES, Observation},
};

pub const COPY_WORDS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Idle,
    Blocked,
    Copying,
    Ready(Descriptor),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counters {
    pub published: u64,
    pub faults: u64,
    pub source_drop_newest: u32,
}

#[cfg(all(test, target_os = "linux", target_pointer_width = "64"))]
pub(super) mod tests {
    extern crate std;
    use super::*;
    use crate::{
        local_control::{Decision, LocalControl, SampleStamp},
        local_queue::LocalQueue,
    };
    use core::ffi::{c_int, c_void};
    const BASE: usize = 0x81000000;
    const G: Generation = Generation {
        boot: 17,
        session: 29,
    };
    unsafe extern "C" {
        fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
    }
    pub(crate) struct ProtectedTx;
    impl ProtectedTx {
        pub(crate) fn new() -> Self {
            assert_eq!(
                unsafe { mprotect((BASE + HALF_BYTES) as *mut c_void, HALF_BYTES, 0) },
                0
            );
            Self
        }
    }
    impl Drop for ProtectedTx {
        fn drop(&mut self) {
            assert_eq!(
                unsafe { mprotect((BASE + HALF_BYTES) as *mut c_void, HALF_BYTES, 3) },
                0
            );
        }
    }
    fn record(sequence: u64) -> Observation {
        let time = 100 + sequence * 200;
        Observation {
            generation: G,
            sequence,
            edge_us: 0,
            irq_us: time,
            transfer_us: time,
            calculation_us: time,
            actuator_us: time,
            sample_timestamp_rp1: time,
            time_sync_generation: 0,
            time_sync_uncertainty_us: u32::MAX,
            flags: 0,
        }
    }
    fn packet(kind: u32, sequence: u32, d: Descriptor) -> [u8; 128] {
        let command = [
            1, 64, 17, 0, 29, 0, sequence, 0, 0, 0, 100000, 0, kind, 64, 0, 0,
        ];
        core::array::from_fn(|i| {
            (if i < 64 {
                command[i / 4]
            } else {
                d.0[(i - 64) / 4]
            })
            .to_le_bytes()[i % 4]
        })
    }
    fn payload() -> [u8; BYTES] {
        let words = core::array::from_fn::<_, { BYTES / 4 }, _>(|i| unsafe {
            ((BASE + HALF_BYTES + i * 4) as *const u32).read_volatile()
        });
        core::array::from_fn(|i| words[i / 4].to_le_bytes()[i % 4])
    }
    fn collect<const N: usize>(c: &mut Collector<'_, N>, sample: Observation) -> Descriptor {
        let words = sample.encode_words().unwrap();
        for i in 0..BYTES / 4 {
            unsafe { ((BASE + HALF_BYTES + i * 4) as *mut u32).write_volatile(0xcccccccc) };
        }
        let mut ready = None;
        for pass in 0..BYTES / 4 / COPY_WORDS {
            let progress = c.poll().unwrap();
            if pass == 5 {
                let Progress::Ready(d) = progress else {
                    panic!("final chunk not published")
                };
                ready = Some(d);
            } else {
                assert_eq!(progress, Progress::Copying);
                assert_eq!(c.ready(), None);
            }
            for (i, expected) in words.iter().enumerate() {
                assert_eq!(
                    unsafe { ((BASE + HALF_BYTES + i * 4) as *const u32).read_volatile() },
                    if i < (pass + 1) * COPY_WORDS {
                        *expected
                    } else {
                        0xcccccccc
                    }
                );
            }
        }
        let d = ready.unwrap();
        assert_eq!(c.ready(), Some(d));
        assert_eq!(Observation::decode(&payload()).unwrap(), sample);
        assert_eq!(d.0[10], BYTES as u32);
        assert_eq!(
            d.0[11] as u64 | ((d.0[12] as u64) << 32),
            sample.sample_timestamp_rp1
        );
        assert_eq!(
            d.0[13..15],
            [sample.time_sync_generation, sample.time_sync_uncertainty_us]
        );
        d
    }
    // Called under bulk::tests' sole anonymous mapping; never touches /dev/mem.
    pub(crate) fn exercise() {
        let mut queue = LocalQueue::<Observation, 2>::new();
        let (mut producer, consumer) = queue.split();
        let mut c = Collector::new(
            unsafe { ReservedDdrWindow::from_verified_region0() },
            G,
            consumer,
        )
        .unwrap();
        {
            let _guard = ProtectedTx::new();
            assert_eq!(c.poll(), Ok(Progress::Idle));
        }
        producer.push(record(1)).unwrap();
        producer.push(record(2)).unwrap();
        let ready = collect(&mut c, record(1));
        let first_bytes = payload();
        let mut local = LocalControl::new(G, 200).unwrap();
        {
            let _guard = ProtectedTx::new();
            // Linux never releases its buffer: C1 still performs100000 local steps/pushes.
            for n in 0..100000u64 {
                assert_eq!(c.poll(), Ok(Progress::Blocked));
                assert_eq!(c.ready(), Some(ready));
                assert!(matches!(
                    local.step(
                        (n + 1) * 200,
                        SampleStamp {
                            boot: G.boot,
                            sequence: n + 1,
                            acquired_us: (n + 1) * 200
                        }
                    ),
                    Decision::Inactive(_)
                ));
                assert_eq!(
                    producer.push(record(n + 3)),
                    if n == 0 { Ok(()) } else { Err(AbiError::Full) }
                );
            }
            assert_eq!(producer.drops(), 99999);
            // RX remains independently usable while the TX half is inaccessible.
            unsafe { (BASE as *mut u32).write_volatile(0x12345678) };
            let rx = Descriptor([1, 2, 17, 0, 29, 0, 1, 0, 1, 0, 4, 50, 0, 0, u32::MAX, 0]);
            c.control(&packet(RX_READY, 1, rx), 100).unwrap();
            let mut word = [0];
            c.read_rx(&mut word).unwrap();
            assert_eq!(word, [0x12345678]);
            assert_eq!(c.release_rx().unwrap().0[1], State::Free as u32);
            let mut free = ready;
            free.0[1] = State::Free as u32;
            let mut stale = free;
            stale.0[4] = 30;
            assert_eq!(
                c.control(&packet(TX_FREE, 2, stale), 100),
                Err(AbiError::Generation.into())
            );
            assert_eq!(c.ready(), Some(ready));
            assert_eq!(c.poll(), Ok(Progress::Blocked));
            c.control(&packet(TX_FREE, 2, free), 100).unwrap();
            assert_eq!(c.ready(), None);
        }
        assert_eq!(payload(), first_bytes);
        collect(&mut c, record(2));
        let second_bytes = payload();
        assert_eq!(
            c.counters(),
            Counters {
                published: 2,
                faults: 0,
                source_drop_newest: 99999
            }
        );
        if let Some(path) = option_env!("RP1_COLLECTOR_FIXTURE") {
            let mut wire = std::vec::Vec::from(first_bytes);
            wire.extend_from_slice(&second_bytes);
            std::fs::write(path, wire).unwrap();
        }
        {
            let _guard = ProtectedTx::new();
            c.retire();
            assert_eq!(c.poll(), Err(Error::Retired));
            assert_eq!(c.ready(), None);
            assert_eq!(
                c.control(&packet(RX_READY, 3, Descriptor([0; 16])), 100),
                Err(Error::Retired)
            );
            assert_eq!(c.read_rx(&mut [0]), Err(Error::Retired));
            assert_eq!(c.release_rx(), Err(Error::Retired));
            producer.push(record(4)).unwrap();
        }
        drop(c);
        drop(producer);
        let (_, mut left) = queue.split();
        assert_eq!(left.pop(), Some(record(3)));
        assert_eq!(left.pop(), Some(record(4)));

        for (mut bad, error) in [
            (record(1), AbiError::Generation),
            (record(1), AbiError::Flags),
            (record(1), AbiError::Time),
            (record(0), AbiError::Sequence),
        ] {
            match error {
                AbiError::Generation => bad.generation.session += 1,
                AbiError::Flags => bad.flags = 16,
                AbiError::Time => bad.calculation_us = 0,
                _ => {}
            }
            let mut queue = LocalQueue::<Observation, 2>::new();
            let (mut p, q) = queue.split();
            p.push(bad).unwrap();
            let mut c = Collector::new(unsafe { ReservedDdrWindow::from_verified_region0() }, G, q)
                .unwrap();
            let _guard = ProtectedTx::new();
            assert_eq!(c.poll(), Err(error.into()));
            assert_eq!(c.counters().faults, 1);
            assert_eq!(c.poll(), Err(Error::Retired));
            assert_eq!(c.counters().published, 0);
            assert_eq!(c.counters().faults, 1);
        }
        for (mut bad, error) in [(record(4), AbiError::Sequence), (record(6), AbiError::Time)] {
            if error == AbiError::Time {
                bad.sample_timestamp_rp1 = 1;
            }
            let mut queue = LocalQueue::<Observation, 2>::new();
            let (mut p, q) = queue.split();
            p.push(record(5)).unwrap();
            let mut c = Collector::new(unsafe { ReservedDdrWindow::from_verified_region0() }, G, q)
                .unwrap();
            let mut free = collect(&mut c, record(5));
            free.0[1] = State::Free as u32;
            c.control(&packet(TX_FREE, 1, free), 100).unwrap();
            p.push(bad).unwrap();
            let _guard = ProtectedTx::new();
            assert_eq!(c.poll(), Err(error.into()));
            assert_eq!(c.counters().faults, 1);
        }
    }
}

/// Owns the unique DDR window and sole queue consumer. One 96-byte OBS1 record
/// per buffer; Linux echoes its exact descriptor as FREE before reuse. Keep this
/// object/queue in RP1 local SRAM. Bounds work, not PCIe transaction latency.
/// Transport loss must call retire; no rebind/reset bypasses peer-stop admission.
pub struct Collector<'a, const N: usize> {
    pub(super) pump: Duplex,
    consumer: Option<Consumer<'a, Observation, N>>,
    words: [u32; BYTES / 4],
    copying: bool,
    last_sequence: u64,
    last_timestamp: u64,
    published: u64,
    faults: u64,
}

impl<'a, const N: usize> Collector<'a, N> {
    pub fn new(
        window: ReservedDdrWindow,
        generation: Generation,
        consumer: Consumer<'a, Observation, N>,
    ) -> Result<Self, Error> {
        Ok(Self::from_duplex(Duplex::new(window, generation)?, consumer))
    }

    // Preserve the admitted window, command gate and RX ledger when the sole
    // endpoint transfers ownership; never creates a second DDR writer.
    pub(super) fn from_duplex(pump: Duplex, consumer: Consumer<'a, Observation, N>) -> Self {
        let mut collector = Self::commissioning(pump);
        collector.consumer = Some(consumer);
        collector
    }
    pub(super) fn commissioning(pump: Duplex) -> Self {
        Self {
            pump,
            consumer: None,
            words: [0; BYTES / 4],
            copying: false,
            last_sequence: 0,
            last_timestamp: 0,
            published: 0,
            faults: 0,
        }
    }
    pub(super) fn attached(&self) -> bool { self.consumer.is_some() }
    // The endpoint validates START before invoking the one-shot C1 handoff.
    pub(super) fn attach(&mut self, consumer: Consumer<'a, Observation, N>) {
        assert!(!self.attached());
        self.consumer = Some(consumer);
    }

    fn fault(&mut self, error: Error) -> Result<Progress, Error> {
        self.faults = self.faults.saturating_add(1);
        self.retire();
        Err(error)
    }

    /// Hold the queue head until READY, with four encoded words/pass. Final
    /// publication performs one non-posted read then releases the queue slot.
    /// A held READY buffer never dequeues or touches DDR.
    #[cfg_attr(feature="amp-observation",inline(never))]
    #[cfg_attr(feature="amp-observation",unsafe(link_section=".amp_observation_text"))]
    pub fn poll(&mut self) -> Result<Progress, Error> {
        self.pump.active()?;
        if !self.copying {
            if self.pump.tx.slot.state() != State::Free {
                return Ok(Progress::Blocked);
            }
            let Some(consumer)=self.consumer.as_mut() else {return Ok(Progress::Idle);};
            let Some(record)=consumer.front() else {return Ok(Progress::Idle);};
            if record.generation != self.pump.generation {
                return self.fault(AbiError::Generation.into());
            }
            if record.sequence <= self.last_sequence {
                return self.fault(AbiError::Sequence.into());
            }
            if record.sample_timestamp_rp1 < self.last_timestamp {
                return self.fault(AbiError::Time.into());
            }
            if let Err(e) = record.encode_words_into(&mut self.words) {
                return self.fault(e.into());
            }
            let g = self.pump.generation;
            let d = Descriptor([
                VERSION,
                State::Writing as u32,
                g.boot as u32,
                (g.boot >> 32) as u32,
                g.session as u32,
                (g.session >> 32) as u32,
                record.sequence as u32,
                (record.sequence >> 32) as u32,
                Actor::Rp1 as u32,
                HALF_BYTES as u32,
                BYTES as u32,
                record.sample_timestamp_rp1 as u32,
                (record.sample_timestamp_rp1 >> 32) as u32,
                record.time_sync_generation,
                record.time_sync_uncertainty_us,
                0,
            ]);
            if let Err(e) = self.pump.begin_tx(d) {
                return self.fault(e);
            }
            self.copying = true;
            self.last_sequence = record.sequence;
            self.last_timestamp = record.sample_timestamp_rp1;
        }
        let first = self.pump.tx.cursor / 4;
        if first>=BYTES/4 {return self.fault(AbiError::Bounds.into());}
        let end = (first + COPY_WORDS).min(BYTES/4);
        if let Err(e) = self.pump.write_tx(&self.words[first..end]) {
            return self.fault(e);
        }
        if end != BYTES/4 {
            return Ok(Progress::Copying);
        }
        let ready = match self.pump.publish_tx() {
            Ok(d) => d,
            Err(e) => return self.fault(e),
        };
        if !self.consumer.as_mut().unwrap().discard_front() {return self.fault(AbiError::Owner.into());}
        self.copying = false;
        self.published = self.published.saturating_add(1);
        Ok(Progress::Ready(ready))
    }

    /// Retry this notification on RPMsg pressure; querying it never re-executes a copy.
    pub fn ready(&self) -> Option<Descriptor> {
        if self.pump.retired || self.pump.tx.slot.state() != State::Ready {
            return None;
        }
        let mut d = self.pump.tx.descriptor?;
        d.0[1] = State::Ready as u32;
        Some(d)
    }
    pub fn control(&mut self, packet: &[u8], now: u64) -> Result<(), Error> {
        self.pump.control(packet, now)
    }
    pub fn read_rx(&mut self, dst: &mut [u32]) -> Result<(), Error> {
        self.pump.read_rx(dst)
    }
    pub fn release_rx(&mut self) -> Result<Descriptor, Error> {
        self.pump.release_rx()
    }
    pub fn retire(&mut self) {
        self.pump.retire();
    }
    pub fn counters(&self) -> Counters {
        Counters {
            published: self.published,
            faults: self.faults,
            source_drop_newest: self.consumer.as_ref().map_or(0,Consumer::drops),
        }
    }
}

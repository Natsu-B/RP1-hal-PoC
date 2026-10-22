//! Single-task, generation-bound duplex DDR pump. Descriptors travel over the
//! control transport; payloads stay in the dedicated 64 KiB DDR reservation.
//! Linux produces [0, 32 KiB); RP1 produces [32 KiB, 64 KiB). Each direction
//! has one buffer. No buffer is reused until its consumer explicitly releases
//! it. `owner` in a descriptor is the immutable producer, not a bus lock.
//!
//! This is a transport/application API, not a periodic control task. Each copy
//! is at most 256 bytes; that bounds work, NOT PCIe transaction latency. Keep
//! the real-time task's state and short queue in local SRAM and service this
//! pump separately. A full direction rejects new data without touching DDR;
//! its saturating counter counts rejected begin attempts (drop-newest).
use rp1_abi::amp::{Actor, Command, CommandGate, Descriptor, Error as AbiError, Generation, Slot, State, Window};
use crate::pcie_outbound::{self, ReservedDdrWindow};

pub const HALF_BYTES: usize = pcie_outbound::SIZE / 2;
pub const CHUNK_WORDS: usize = 64;
pub const RX_READY: u32 = 1;
pub const TX_FREE: u32 = 2;
pub mod observation;

#[cfg(any(test, feature = "amp-bulk"))]
#[path = "bulk_transport.rs"]
mod transport;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Abi(AbiError), Ddr(pcie_outbound::Error), Incomplete, Retired }
impl From<AbiError> for Error { fn from(e: AbiError) -> Self { Self::Abi(e) } }
impl From<pcie_outbound::Error> for Error { fn from(e: pcie_outbound::Error) -> Self { Self::Ddr(e) } }

struct Direction { slot: Slot, descriptor: Option<Descriptor>, cursor: usize }
impl Direction {
    fn new(generation: Generation, producer: Actor) -> Result<Self, Error> {
        Ok(Self { slot: Slot::new(generation, producer)?, descriptor: None, cursor: 0 })
    }
    fn chunk(&self, words: usize, state: State) -> Result<usize, Error> {
        if self.slot.state() != state { return Err(AbiError::Owner.into()); }
        if words == 0 || words > CHUNK_WORDS { return Err(AbiError::Length.into()); }
        let d = self.descriptor.ok_or(AbiError::Owner)?;
        if self.cursor + words * 4 > d.0[10] as usize { return Err(AbiError::Bounds.into()); }
        Ok(d.0[9] as usize + self.cursor)
    }
    fn complete(&self, state: State) -> Result<Descriptor, Error> {
        if self.slot.state() != state { return Err(AbiError::Owner.into()); }
        let d = self.descriptor.ok_or(AbiError::Owner)?;
        if self.cursor != d.0[10] as usize { return Err(Error::Incomplete); }
        Ok(d)
    }
}

/// Owning this value excludes the legacy DDR test RPC and all other region-0
/// users/configuration writers. The sole transport task owns it for the boot.
/// Construction performs no MMIO and does not establish Linux's reservation.
/// The input window's unsafe admission contract must already hold.
///
/// On transport loss call `retire`: all later operations reject without MMIO.
/// There is intentionally no reset/rebind method. Establishing a new session
/// requires a separately proved peer-stop contract, not just changing a nonce.
pub struct Duplex {
    window: ReservedDdrWindow,
    generation: Generation,
    control: CommandGate,
    rx: Direction,
    tx: Direction,
    retired: bool,
}
impl Duplex {
    pub fn new(window: ReservedDdrWindow, generation: Generation) -> Result<Self, Error> {
        Ok(Self { window, generation, control: CommandGate::new(generation)?, rx: Direction::new(generation, Actor::Linux)?,
            tx: Direction::new(generation, Actor::Rp1)?, retired: false })
    }
    fn active(&self) -> Result<(), Error> {
        if self.retired { Err(Error::Retired) } else { Ok(()) }
    }
    fn validate(&self, d: Descriptor, producer: Actor, state: State) -> Result<(), Error> {
        self.active()?;
        d.validate(self.generation, Window::new(0, pcie_outbound::SIZE as u64)?, producer, state)?;
        let offset = if producer == Actor::Linux { 0 } else { HALF_BYTES };
        if d.0[9] as usize != offset || d.0[10] as usize > HALF_BYTES { return Err(AbiError::Bounds.into()); }
        Ok(())
    }
    fn sequence(d: Descriptor) -> u64 { u64::from(d.0[6]) | (u64::from(d.0[7]) << 32) }

    /// RPMsg payload: Command (64 bytes) followed by Descriptor (64 bytes).
    /// Kinds RX_READY/TX_FREE notify ownership changes, never carry bulk data.
    /// Notifications are immediate: future apply_time is rejected. `now` must
    /// come from the RP1 monotonic clock, never a value supplied by Linux.
    /// A rejected notification does not consume its command sequence.
    pub fn control(&mut self, packet: &[u8], now: u64) -> Result<(), Error> {
        self.active()?;
        let command = Command::decode(packet)?;
        if packet.len() != 128 { return Err(AbiError::Length.into()); }
        if command.apply_time() > now { return Err(AbiError::Time.into()); }
        let descriptor = Descriptor::decode(&packet[64..])?;
        let mut next = self.control;
        next.accept(command, 64, now)?;
        match command.0[12] {
            RX_READY => self.accept_rx(descriptor)?,
            TX_FREE => self.acknowledge_tx(descriptor)?,
            _ => return Err(AbiError::Flags.into()),
        }
        self.control = next;
        Ok(())
    }

    /// Linux must complete payload writes before notifying RP1 and must not
    /// touch that payload again until it receives the FREE descriptor.
    pub fn accept_rx(&mut self, d: Descriptor) -> Result<(), Error> {
        self.validate(d, Actor::Linux, State::Ready)?;
        let seq = Self::sequence(d);
        self.rx.slot.begin(Actor::Linux, self.generation, seq)?;
        self.rx.slot.transition(Actor::Linux, self.generation, seq, State::Ready)?;
        self.rx.slot.transition(Actor::Rp1, self.generation, seq, State::Reading)?;
        self.rx.descriptor = Some(d); self.rx.cursor = 0;
        Ok(())
    }
    pub fn read_rx(&mut self, dst: &mut [u32]) -> Result<(), Error> {
        self.active()?;
        let offset = self.rx.chunk(dst.len(), State::Reading)?;
        pcie_outbound::barrier();
        self.window.read_words(offset, dst)?;
        self.rx.cursor += dst.len() * 4;
        Ok(())
    }
    /// Return this descriptor over the control transport. Failed notification
    /// must be retried without letting Linux reuse the buffer early.
    pub fn release_rx(&mut self) -> Result<Descriptor, Error> {
        self.active()?;
        let mut d = self.rx.complete(State::Reading)?;
        pcie_outbound::barrier();
        self.rx.slot.transition(Actor::Rp1, self.generation, Self::sequence(d), State::Free)?;
        d.0[1] = State::Free as u32;
        Ok(d)
    }
    /// Descriptor must name RP1's fixed half, WRITING, and a fresh sequence.
    /// Timestamp and time-sync fields describe acquisition, not Linux arrival.
    pub fn begin_tx(&mut self, d: Descriptor) -> Result<(), Error> {
        self.validate(d, Actor::Rp1, State::Writing)?;
        self.tx.slot.begin(Actor::Rp1, self.generation, Self::sequence(d))?;
        self.tx.descriptor = Some(d); self.tx.cursor = 0;
        Ok(())
    }
    pub fn write_tx(&mut self, src: &[u32]) -> Result<(), Error> {
        self.active()?;
        let offset = self.tx.chunk(src.len(), State::Writing)?;
        self.window.write_words(offset, src)?;
        self.tx.cursor += src.len() * 4;
        Ok(())
    }
    /// Completes posted writes by a non-posted read in RP1's OWN half before
    /// returning READY. Retain/retry this descriptor if RPMsg is congested.
    pub fn publish_tx(&mut self) -> Result<Descriptor, Error> {
        self.active()?;
        let mut d = self.tx.complete(State::Writing)?;
        self.window.complete_posted_write_at(HALF_BYTES)?;
        self.tx.slot.transition(Actor::Rp1, self.generation, Self::sequence(d), State::Ready)?;
        d.0[1] = State::Ready as u32;
        Ok(d)
    }
    /// Linux echoes the exact descriptor with only state changed to FREE,
    /// after it has finished reading. A stale/wrong acknowledgement cannot
    /// free the current buffer or perform any DDR transaction.
    pub fn acknowledge_tx(&mut self, mut ack: Descriptor) -> Result<(), Error> {
        self.validate(ack, Actor::Rp1, State::Free)?;
        ack.0[1] = State::Writing as u32;
        if Some(ack) != self.tx.descriptor { return Err(AbiError::Sequence.into()); }
        let seq = Self::sequence(ack);
        self.tx.slot.transition(Actor::Linux, self.generation, seq, State::Reading)?;
        self.tx.slot.transition(Actor::Linux, self.generation, seq, State::Free)?;
        Ok(())
    }
    pub fn retire(&mut self) { self.retired = true; }
    // Only an explicit, generation-validated live-peer RETIRE may drain a prefix.
    // Transport loss uses retire() without issuing a read to a missing root.
    fn quiesce_for_peer(&mut self) -> Result<(), Error> {
        self.retire();
        if self.tx.slot.state() == State::Writing && self.tx.cursor != 0 {
            self.window.complete_posted_write_at(HALF_BYTES)?;
        }
        Ok(())
    }
    pub fn drops(&self) -> (u64, u64) { (self.rx.slot.drops(), self.tx.slot.drops()) }
}

#[cfg(all(test, target_os = "linux", target_pointer_width = "64"))]
mod tests {
    use super::*;
    use core::ffi::{c_int, c_void};
    unsafe extern "C" {
        fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
        fn munmap(addr: *mut c_void, len: usize) -> c_int;
    }
    const BASE: usize = 0x81000000;
    const G: Generation = Generation { boot: 17, session: 29 };
    fn descriptor(producer: Actor, sequence: u64, length: usize) -> Descriptor {
        Descriptor([1, if producer == Actor::Rp1 {1} else {2}, 17,0,29,0,
            sequence as u32,(sequence >> 32) as u32,producer as u32,
            if producer == Actor::Rp1 {HALF_BYTES as u32} else {0},length as u32,123,0,0,u32::MAX,0])
    }
    fn packet(kind: u32, sequence: u32, d: Descriptor) -> [u8;128] {
        let c = [1,64,17,0,29,0,sequence,0,0,0,200,0,kind,64,0,0];
        core::array::from_fn(|i| {
            let word = if i < 64 { c[i/4] } else { d.0[(i-64)/4] };
            word.to_le_bytes()[i%4]
        })
    }
    // One Linux process-local anonymous mapping, never /dev/mem. NOREPLACE
    // prevents clobbering an existing mapping. Drop unmaps even on assertion.
    struct Mapping;
    impl Drop for Mapping { fn drop(&mut self) { unsafe { munmap(BASE as *mut c_void, pcie_outbound::SIZE); } } }

    #[test]
    fn duplex_real_copy_stall_invalid_ack_and_retirement() {
        unsafe {
            assert_eq!(mmap(BASE as *mut c_void, pcie_outbound::SIZE, 3,
                0x100000 | 0x20 | 2, -1, 0) as usize, BASE);
        }
        let _mapping = Mapping;
        let mut pump = Duplex::new(unsafe { ReservedDdrWindow::from_verified_region0() }, G).unwrap();
        // Full 64 KiB, both directions interleaved at 256 bytes per call.
        // These are host RAM copies; they do not prove hardware coherence.
        for i in 0..HALF_BYTES/4 { unsafe { ((BASE+i*4) as *mut u32).write_volatile(i as u32); } }
        let notification = packet(RX_READY, 1, descriptor(Actor::Linux, 1, HALF_BYTES));
        assert!(pump.control(&notification[..127], 100).is_err());
        assert!(pump.control(&notification, 200).is_err());
        assert_eq!(pump.control.last_sequence(), 0);
        pump.control(&notification, 100).unwrap();
        assert_eq!(pump.control(&notification, 100), Err(Error::Abi(AbiError::Sequence)));
        pump.begin_tx(descriptor(Actor::Rp1, 1, HALF_BYTES)).unwrap();
        assert_eq!(pump.publish_tx(), Err(Error::Incomplete));
        assert_eq!(pump.release_rx(), Err(Error::Incomplete));
        assert!(pump.write_tx(&[0;CHUNK_WORDS+1]).is_err());
        let mut chunk = [0;CHUNK_WORDS];
        for i in (0..HALF_BYTES/4).step_by(CHUNK_WORDS) {
            pump.read_rx(&mut chunk).unwrap();
            for (j, v) in chunk.iter_mut().enumerate() { assert_eq!(*v, (i+j) as u32); *v ^= 0xa55aa55a; }
            pump.write_tx(&chunk).unwrap();
        }
        assert!(pump.read_rx(&mut [0]).is_err());
        let free = pump.release_rx().unwrap(); assert_eq!(free.0[1], 0);
        let ready = pump.publish_tx().unwrap(); assert_eq!(ready.0[1], 2);
        for i in 0..HALF_BYTES/4 {
            assert_eq!(unsafe { ((BASE+HALF_BYTES+i*4) as *const u32).read_volatile() }, (i as u32)^0xa55aa55a);
        }
        // Linux stalls forever: repeated producer attempts cannot write or
        // silently overwrite unread data. The opposite direction still works.
        for n in 2..102 { assert_eq!(pump.begin_tx(descriptor(Actor::Rp1,n,4)), Err(Error::Abi(AbiError::Full))); }
        assert_eq!(pump.drops(), (0,100));
        assert!(pump.write_tx(&[0]).is_err());
        pump.accept_rx(descriptor(Actor::Linux,2,4)).unwrap();
        pump.read_rx(&mut [0]).unwrap(); pump.release_rx().unwrap();
        let mut ack = ready; ack.0[1] = 0;
        for (index, value) in [(2,18),(4,30),(6,0),(9,0),(10,4),(11,124),(15,1)] {
            let mut bad=ack; bad.0[index]=value; assert!(pump.acknowledge_tx(bad).is_err());
        }
        // Invalid common header or payload cannot release the slot or consume
        // the next control sequence, including when the descriptor is valid.
        let control_ack = packet(TX_FREE, 2, ack);
        for (word,value) in [(0,2u32),(2,18),(4,30),(6,1),(8,101),(10,100),(12,99),(13,60),(14,1),(15,1),(16+10,4)] {
            let mut bad = control_ack;
            bad[word*4..word*4+4].copy_from_slice(&value.to_le_bytes());
            assert!(pump.control(&bad,100).is_err());
            assert_eq!(pump.control.last_sequence(), 1);
        }
        pump.control(&control_ack,100).unwrap();
        assert!(pump.acknowledge_tx(ack).is_err());
        assert!(pump.begin_tx(descriptor(Actor::Rp1,1,4)).is_err());
        pump.begin_tx(descriptor(Actor::Rp1,2,4)).unwrap();
        pump.retire();
        assert_eq!(pump.write_tx(&[0]), Err(Error::Retired));
        assert_eq!(pump.publish_tx(), Err(Error::Retired));
        assert_eq!(pump.read_rx(&mut [0]), Err(Error::Retired));
        assert_eq!(pump.release_rx(), Err(Error::Retired));
        assert_eq!(pump.acknowledge_tx(ack), Err(Error::Retired));
        assert_eq!(pump.accept_rx(descriptor(Actor::Linux,3,4)), Err(Error::Retired));
        assert_eq!(pump.begin_tx(descriptor(Actor::Rp1,3,4)), Err(Error::Retired));
        assert_eq!(unsafe { ((BASE+HALF_BYTES) as *const u32).read_volatile() }, 0xa55aa55a);
        super::transport::tests::exercise();
        super::transport::tests::exercise_observations();
        super::observation::tests::exercise();
        // Empty/held buffers need no completion transaction; a partial prefix
        // is drained without publishing READY or accepting later writes.
        let mut p=Duplex::new(unsafe {ReservedDdrWindow::from_verified_region0()},G).unwrap();
        p.begin_tx(descriptor(Actor::Rp1,1,32)).unwrap();
        {let _guard=super::observation::tests::ProtectedTx::new();p.quiesce_for_peer().unwrap();}
        let mut p=Duplex::new(unsafe {ReservedDdrWindow::from_verified_region0()},G).unwrap();
        p.begin_tx(descriptor(Actor::Rp1,1,32)).unwrap();p.write_tx(&[0x12345678;4]).unwrap();
        p.quiesce_for_peer().unwrap();assert_eq!(p.tx.slot.state(),State::Writing);
        assert_eq!(p.write_tx(&[0]),Err(Error::Retired));assert_eq!(p.publish_tx(),Err(Error::Retired));
        {let _guard=super::observation::tests::ProtectedTx::new();p.retire();}
    }
}

//! AMP v1: allocation-free validation, not a cache or synchronization primitive.
//! Wire records are little-endian u32 words; do not cast untrusted bytes to Rust
//! enums. The transport must complete writes before publishing, and acquire the
//! publication before copying a record. One local actor owns each Slot/Gate.

pub const VERSION: u32 = 1;
pub const RECORD_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Alignment, Overflow, Bounds, Width, Empty, Version, Length, Flags,
    Generation, Sequence, Expired, Time, Owner, Full,
}

/// Metadata only: constructing this value never grants access to physical RAM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window { base: u64, size: u64 }
impl Window {
    pub fn new(base: u64, size: u64) -> Result<Self, Error> {
        if size == 0 { return Err(Error::Empty); }
        if base & 3 != 0 || size & 3 != 0 { return Err(Error::Alignment); }
        base.checked_add(size).ok_or(Error::Overflow)?;
        Ok(Self { base, size })
    }
    pub const fn base(self) -> u64 { self.base }
    pub const fn size(self) -> u64 { self.size }
    pub fn check(self, offset: u64, length: u64, width: u32) -> Result<u64, Error> {
        // Only the aligned volatile u32 path has an RP1 hardware contract.
        if width != 4 { return Err(Error::Width); }
        if offset & 3 != 0 || length & 3 != 0 { return Err(Error::Alignment); }
        let end = offset.checked_add(length).ok_or(Error::Overflow)?;
        if offset > self.size || end > self.size { return Err(Error::Bounds); }
        self.base.checked_add(offset).ok_or(Error::Overflow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation { pub boot: u64, pub session: u64 }
impl Generation {
    pub fn new(boot: u64, session: u64) -> Result<Self, Error> {
        if boot == 0 || session == 0 { return Err(Error::Generation); }
        Ok(Self { boot, session })
    }
    fn matches(self, other: Self) -> Result<(), Error> {
        Self::new(self.boot, self.session)?;
        if self != other { return Err(Error::Generation); }
        Ok(())
    }
}

/// Header words: version, byte length, boot[lo,hi], session[lo,hi],
/// sequence[lo,hi], apply_time[lo,hi], valid_until[lo,hi], type,
/// payload_bytes, flags, reserved. Times are RP1 monotonic microseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command(pub [u32; 16]);
fn pair(w: &[u32], i: usize) -> u64 { u64::from(w[i]) | (u64::from(w[i+1]) << 32) }
fn decode_words(bytes: &[u8]) -> Result<[u32;16], Error> {
    if bytes.len() != RECORD_BYTES { return Err(Error::Length); }
    Ok(core::array::from_fn(|i| u32::from_le_bytes(bytes[i*4..i*4+4].try_into().unwrap())))
}
impl Command {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < RECORD_BYTES { return Err(Error::Length); }
        let w = decode_words(&bytes[..RECORD_BYTES])?;
        if usize::try_from(w[13]).ok().and_then(|n| n.checked_add(RECORD_BYTES)) != Some(bytes.len()) {
            return Err(Error::Length);
        }
        Ok(Self(w))
    }
    pub fn generation(self) -> Generation { Generation { boot: pair(&self.0,2), session: pair(&self.0,4) } }
    pub fn sequence(self) -> u64 { pair(&self.0,6) }
    pub fn apply_time(self) -> u64 { pair(&self.0,8) }
    pub fn valid_until(self) -> u64 { pair(&self.0,10) }
}

/// Validate a complete message before atomically replacing a local command.
/// The caller keeps current/next command and failsafe in RP1 local memory.
/// Sequence wrap requires a NEW session; no half-range/wrapping comparisons.
#[derive(Clone, Copy)]
pub struct CommandGate { generation: Generation, last: u64 }
impl CommandGate {
    pub fn new(generation: Generation) -> Result<Self, Error> {
        Generation::new(generation.boot, generation.session)?;
        Ok(Self { generation, last: 0 })
    }
    pub fn accept(&mut self, c: Command, payload_bytes: usize, now: u64) -> Result<(), Error> {
        if c.0[0] != VERSION || c.0[1] != RECORD_BYTES as u32 { return Err(Error::Version); }
        if usize::try_from(c.0[13]).ok() != Some(payload_bytes) { return Err(Error::Length); }
        if c.0[14] != 0 || c.0[15] != 0 { return Err(Error::Flags); }
        self.generation.matches(c.generation())?;
        if c.sequence() <= self.last { return Err(Error::Sequence); }
        if c.apply_time() > c.valid_until() { return Err(Error::Time); }
        if now >= c.valid_until() { return Err(Error::Expired); }
        // Message-type/payload schema validation must precede this call. This
        // gate only advances after EVERY common-header check has succeeded.
        self.last = c.sequence();
        Ok(())
    }
    pub const fn last_sequence(&self) -> u64 { self.last }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Actor { Linux = 1, Rp1 = 2 }
impl Actor { fn peer(self) -> Self { match self { Self::Linux => Self::Rp1, Self::Rp1 => Self::Linux } } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum State { Free = 0, Writing = 1, Ready = 2, Reading = 3 }

/// 64-byte wire descriptor: version, state, boot lo/hi, session lo/hi,
/// sequence lo/hi, owner, offset, length, timestamp_rp1 lo/hi,
/// time_sync_generation, time_sync_uncertainty_us, flags.
/// A per-direction ring uses one producer, one consumer and disjoint payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Descriptor(pub [u32;16]);
impl Descriptor {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> { Ok(Self(decode_words(bytes)?)) }
    pub fn validate(self, generation: Generation, window: Window, owner: Actor, state: State) -> Result<(), Error> {
        let w = self.0;
        if w[0] != VERSION { return Err(Error::Version); }
        generation.matches(Generation { boot: pair(&w,2), session: pair(&w,4) })?;
        if w[1] != state as u32 || w[8] != owner as u32 { return Err(Error::Owner); }
        if pair(&w,6) == 0 { return Err(Error::Sequence); }
        if w[10] == 0 { return Err(Error::Empty); }
        if w[15] != 0 { return Err(Error::Flags); }
        if w[13] == 0 && w[14] != u32::MAX { return Err(Error::Time); }
        window.check(u64::from(w[9]), u64::from(w[10]), 4)?;
        Ok(())
    }
}

/// Local ownership ledger. Never keep this control object in Linux DDR.
/// Full policy is drop-newest: no payload access/overwrite, increment drops.
/// It is not itself a multi-core lock or a remotely writable descriptor.
pub struct Slot {
    generation: Generation, producer: Actor, state: State,
    sequence: u64, drops: u64,
}
impl Slot {
    pub fn new(generation: Generation, producer: Actor) -> Result<Self, Error> {
        Generation::new(generation.boot, generation.session)?;
        Ok(Self { generation, producer, state: State::Free, sequence: 0, drops: 0 })
    }
    pub fn begin(&mut self, actor: Actor, generation: Generation, sequence: u64) -> Result<(), Error> {
        self.generation.matches(generation)?;
        if actor != self.producer { return Err(Error::Owner); }
        if sequence <= self.sequence { return Err(Error::Sequence); }
        if self.state != State::Free {
            self.drops = self.drops.saturating_add(1);
            return Err(Error::Full);
        }
        self.sequence = sequence;
        self.state = State::Writing;
        Ok(())
    }
    pub fn transition(&mut self, actor: Actor, generation: Generation, sequence: u64, next: State) -> Result<(), Error> {
        self.generation.matches(generation)?;
        if sequence != self.sequence || sequence == 0 { return Err(Error::Sequence); }
        let allowed = match (self.state, next) {
            (State::Writing, State::Ready) => actor == self.producer,
            (State::Ready, State::Reading) | (State::Reading, State::Free) => actor == self.producer.peer(),
            _ => false,
        };
        if !allowed { return Err(Error::Owner); }
        self.state = next;
        Ok(())
    }
    pub const fn state(&self) -> State { self.state }
    pub const fn drops(&self) -> u64 { self.drops }
}

#[cfg(test)]
mod tests {
    use super::*;
    const G: Generation = Generation { boot: 7, session: 3 };
    #[test]
    fn full_range_and_capacity_are_metadata_only() {
        for size in [65536, 1048576, 4194304] {
            let w = Window::new(0x21000000, size).unwrap();
            for offset in (0..size).step_by(4) { assert_eq!(w.check(offset,4,4),Ok(w.base()+offset)); }
            for page in [4096,16384] { assert!(w.check(page-4,8,4).is_ok()); }
            assert_eq!(w.check(size-4,8,4),Err(Error::Bounds));
            assert_eq!(w.check(size,4,4),Err(Error::Bounds));
            assert_eq!(w.check(2,4,4),Err(Error::Alignment));
            assert_eq!(w.check(0,6,4),Err(Error::Alignment));
            assert_eq!(w.check(0,4,8),Err(Error::Width));
            assert_eq!(w.check(u64::MAX-3,8,4),Err(Error::Overflow));
        }
        assert_eq!(Window::new(u64::MAX-3,8),Err(Error::Overflow));
        assert_eq!(Window::new(0,0),Err(Error::Empty));
        assert_eq!(Window::new(2,4),Err(Error::Alignment));
    }
    fn command(seq: u32) -> Command { Command([1,64,7,0,3,0,seq,0,100,0,200,0,1,4,0,0]) }
    #[test]
    fn command_rejections_do_not_advance_atomic_update() {
        let mut gate = CommandGate::new(G).unwrap();
        gate.accept(command(1),4,100).unwrap();
        for (index,value,error) in [(0,2,Error::Version),(1,63,Error::Version),(2,8,Error::Generation),
            (4,4,Error::Generation),(6,1,Error::Sequence),(13,8,Error::Length),
            (14,1,Error::Flags),(15,1,Error::Flags),(8,201,Error::Time),(10,100,Error::Expired)] {
            let mut c=command(2);c.0[index]=value;
            assert_eq!(gate.accept(c,4,100),Err(error));assert_eq!(gate.last_sequence(),1);
        }
        gate.accept(command(3),4,100).unwrap();
        assert_eq!(gate.accept(command(2),4,100),Err(Error::Sequence));
        let bytes: [u8;64] = core::array::from_fn(|i| command(3).0[i/4].to_le_bytes()[i%4]);
        assert_eq!(Command::decode(&bytes),Err(Error::Length));
        let mut packet=[0u8;68];packet[..64].copy_from_slice(&bytes);
        assert_eq!(Command::decode(&packet),Ok(command(3)));
    }
    #[test]
    fn ownership_stall_drop_and_restart_both_directions() {
        for producer in [Actor::Linux,Actor::Rp1] {
            let mut slot=Slot::new(G,producer).unwrap();
            slot.begin(producer,G,1).unwrap();
            assert_eq!(slot.begin(producer,G,2),Err(Error::Full));
            assert_eq!(slot.drops(),1);
            assert_eq!(slot.transition(producer.peer(),G,1,State::Ready),Err(Error::Owner));
            slot.transition(producer,G,1,State::Ready).unwrap();
            slot.transition(producer.peer(),G,1,State::Reading).unwrap();
            slot.transition(producer.peer(),G,1,State::Free).unwrap();
            assert_eq!(slot.begin(producer,G,1),Err(Error::Sequence));
            let new=Generation { boot:8,session:4 };
            assert_eq!(slot.begin(producer,new,2),Err(Error::Generation));
            let mut fresh=Slot::new(new,producer).unwrap();
            assert_eq!(fresh.begin(producer,G,2),Err(Error::Generation));
            fresh.begin(producer,new,1).unwrap();
        }
    }
    #[test]
    fn descriptors_fail_closed() {
        let w=Window::new(0x21000000,65536).unwrap();
        let d=Descriptor([1,2,7,0,3,0,1,0,1,65532,4,99,0,2,10,0]);
        d.validate(G,w,Actor::Linux,State::Ready).unwrap();
        for (i,v) in [(0,2),(1,99),(2,8),(4,4),(6,0),(8,2),(9,65536),(10,8),(15,1)] {
            let mut bad=d;bad.0[i]=v;assert!(bad.validate(G,w,Actor::Linux,State::Ready).is_err());
        }
    }
}

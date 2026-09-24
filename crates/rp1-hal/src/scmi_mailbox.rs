//! SCMI one-channel shared-memory publication, separate from IRQ routing.
//! IRQ57 is NOT configured/proven by this module. The runtime must admit its
//! source->pending->vector route before calling service_irq from that handler.
use crate::clock_profile_generated::MAILBOX_CHANNEL;
use crate::scmi_clock::{ClockHardware, Error, Response, Server};

pub const WORDS: usize = 64;
const STATUS: usize = 1;
const FLAGS: usize = 4;
const LENGTH: usize = 5;
const HEADER: usize = 6;
const PAYLOAD: usize = 7;
const FREE: u32 = 1;
const MASK: u32 = 1 << MAILBOX_CHANNEL;

/// Board adapter; publication barriers must include compiler AND bus ordering.
pub trait MailboxIo {
    fn events(&mut self) -> u32;
    fn ack_request(&mut self, mask: u32);
    fn read(&mut self, word: usize) -> u32;
    fn write(&mut self, word: usize, value: u32);
    fn barrier(&mut self);
    fn notify_host(&mut self, mask: u32);
}

#[derive(Default)]
pub struct Counters {
    pub entries: u32,
    pub requests: u32,
    pub responses: u32,
    pub notifications: u32,
    pub malformed: u32,
}

/// Call only with Linux/channel ownership quiesced (including warm restart).
pub fn initialize(io: &mut impl MailboxIo) {
    for i in 0..WORDS { io.write(i, 0); }
    io.barrier();
    io.write(STATUS, FREE);
    io.barrier();
}

/// At most one request and four argument words per handler invocation. No
/// allocation, waits, locks, arbitrary memory operations or clock polling.
pub fn service_irq(server: &mut Server, hw: &mut impl ClockHardware,
                   io: &mut impl MailboxIo, count: &mut Counters) -> bool {
    count.entries = count.entries.wrapping_add(1);
    if io.events() & MASK == 0 { return false; }
    io.ack_request(MASK); // Never ACK unrelated mailbox channels.
    io.barrier();
    if io.read(STATUS) & FREE != 0 { return false; }
    count.requests = count.requests.wrapping_add(1);
    let length = io.read(LENGTH) as usize;
    let header = io.read(HEADER);
    let flags = io.read(FLAGS);
    let mut args = [0; 4];
    let response = if !(4..=20).contains(&length) || length % 4 != 0 || flags & !1 != 0 {
        count.malformed = count.malformed.wrapping_add(1);
        Response { words: [Error::Protocol as i32 as u32, 0, 0, 0, 0, 0, 0, 0], len: 1 }
    } else {
        let n = (length - 4) / 4;
        for (i, value) in args[..n].iter_mut().enumerate() { *value = io.read(PAYLOAD + i); }
        server.request(header, &args[..n], hw)
    };
    for (i, value) in response.words[..response.len].iter().enumerate() {
        io.write(PAYLOAD + i, *value);
    }
    // Header/token remains unchanged. Length includes header + status + data.
    io.write(LENGTH, (4 + response.len * 4) as u32);
    io.barrier();
    io.write(STATUS, FREE);
    io.barrier();
    count.responses = count.responses.wrapping_add(1);
    if flags & 1 != 0 {
        io.notify_host(MASK);
        io.barrier();
        count.notifications = count.notifications.wrapping_add(1);
    }
    true
}

/// Local volatile adapter. Does not change any IRQ routing/priority, clocks,
/// reset, BAR or iATU state. Events SET/CLR aliases follow rp1-mailbox.c.
#[cfg(target_arch = "arm")]
pub struct Rp1Mailbox { shared: *mut u32 }

#[cfg(target_arch = "arm")]
impl Rp1Mailbox {
    /// # Safety
    /// shared must point to this image's exclusive, 256-byte linker reservation,
    /// matched to the final DTB. There must be one proc0 handler owner and no
    /// second firmware writer. Linux must obey the SCMI ownership protocol.
    pub unsafe fn new(shared: *mut u32) -> Option<Self> {
        let start = shared as usize;
        if start & 3 != 0 || start < 0x2000_0000 || start.checked_add(256)? > 0x2000_e000 {
            return None;
        }
        Some(Self { shared })
    }
}

#[cfg(target_arch = "arm")]
impl MailboxIo for Rp1Mailbox {
    fn events(&mut self) -> u32 { unsafe { core::ptr::read_volatile(0x4000_8008 as *const u32) } }
    fn ack_request(&mut self, mask: u32) {
        unsafe { core::ptr::write_volatile(0x4000_b008 as *mut u32, mask & MASK); }
    }
    fn read(&mut self, word: usize) -> u32 {
        assert!(word < WORDS);
        unsafe { core::ptr::read_volatile(self.shared.add(word)) }
    }
    fn write(&mut self, word: usize, value: u32) {
        assert!(word < WORDS);
        unsafe { core::ptr::write_volatile(self.shared.add(word), value); }
    }
    fn barrier(&mut self) {
        // No nomem: also acts as a compiler memory barrier.
        unsafe { core::arch::asm!("dsb sy", options(nostack, preserves_flags)); }
    }
    fn notify_host(&mut self, mask: u32) {
        unsafe { core::ptr::write_volatile(0x4000_a00c as *mut u32, mask & MASK); }
    }
}

#[cfg(all(feature = "scmi-clock", target_arch = "arm"))]
#[repr(C, align(64))]
struct Shared([u32; WORDS]);

#[cfg(all(feature = "scmi-clock", target_arch = "arm"))]
#[used]
#[unsafe(link_section = ".scmi_shmem")]
static mut SHARED: Shared = Shared([0; WORDS]);

/// Address only; initialize explicitly before advertising the channel to Linux.
#[cfg(all(feature = "scmi-clock", target_arch = "arm"))]
pub fn shared_address() -> *mut u32 { core::ptr::addr_of_mut!(SHARED).cast::<u32>() }

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::clock_profile_generated::CLOCKS;
    use crate::scmi_clock::PhysicalState;
    struct Fake { words: [u32; WORDS], events: u32, log: std::vec::Vec<(usize, u32)> }
    impl MailboxIo for Fake {
        fn events(&mut self) -> u32 { self.events }
        fn ack_request(&mut self, mask: u32) { self.events &= !mask; self.log.push((100, mask)); }
        fn read(&mut self, i: usize) -> u32 { self.words[i] }
        fn write(&mut self, i: usize, v: u32) { self.words[i] = v; self.log.push((i, v)); }
        fn barrier(&mut self) { self.log.push((101, 0)); }
        fn notify_host(&mut self, mask: u32) { self.log.push((102, mask)); }
    }
    struct Hw;
    impl ClockHardware for Hw {
        fn read(&mut self, _: u32) -> Result<PhysicalState, Error> { Ok(PhysicalState { rate_hz: 100_000_000, enabled: true }) }
        fn enable(&mut self, _: u32, _: bool) -> Result<(), Error> { panic!("read-only test must not write") }
    }
    fn fake() -> Fake {
        let mut f = Fake { words: [0; WORDS], events: MASK | 1, log: std::vec::Vec::new() };
        f.words[FLAGS] = 1; f.words[LENGTH] = 8;
        f.words[HEADER] = (0x3ff << 18) | (0x14 << 10) | 6;
        f
    }
    #[test] fn irq_response_publication_order_and_token() {
        let mut f = fake(); let header = f.words[HEADER];
        let mut s = Server::new(CLOCKS, false).unwrap(); let mut c = Counters::default();
        assert!(service_irq(&mut s, &mut Hw, &mut f, &mut c));
        assert_eq!(f.events, 1);
        assert_eq!(f.words[HEADER], header);
        assert_eq!(f.words[PAYLOAD..PAYLOAD+3], [0, 100_000_000, 0]);
        assert_eq!(f.words[LENGTH], 16);
        assert_eq!(&f.log[f.log.len()-6..], &[(LENGTH,16),(101,0),(STATUS,FREE),(101,0),(102,MASK),(101,0)]);
        assert_eq!(c.notifications, 1);
    }
    #[test] fn free_or_unrelated_event_does_not_replay() {
        let mut f = fake(); f.words[STATUS] = FREE;
        let mut s = Server::new(CLOCKS, false).unwrap(); let mut c = Counters::default();
        assert!(!service_irq(&mut s, &mut Hw, &mut f, &mut c));
        assert!(!service_irq(&mut s, &mut Hw, &mut f, &mut c));
        assert_eq!(c.responses, 0); assert_eq!(f.events, 1);
    }
    #[test] fn malformed_length_is_bounded_and_reported() {
        for len in [0, 3, 5, 24, 232, u32::MAX] {
            let mut f = fake(); f.words[LENGTH] = len;
            let mut s = Server::new(CLOCKS, false).unwrap(); let mut c = Counters::default();
            assert!(service_irq(&mut s, &mut Hw, &mut f, &mut c));
            assert_eq!(f.words[PAYLOAD] as i32, -10);
            assert_eq!(c.malformed, 1);
        }
    }
    #[test] fn no_intr_flag_does_not_claim_irq_completion() {
        let mut f = fake(); f.words[FLAGS] = 0;
        let mut s = Server::new(CLOCKS, false).unwrap(); let mut c = Counters::default();
        assert!(service_irq(&mut s, &mut Hw, &mut f, &mut c));
        assert_eq!(c.responses, 1); assert_eq!(c.notifications, 0);
    }
    #[test] fn init_publishes_free_last() {
        let mut f = fake(); initialize(&mut f);
        assert_eq!(&f.log[f.log.len()-3..], &[(101,0),(STATUS,FREE),(101,0)]);
        assert_eq!(f.words[FLAGS], 0);
    }
}

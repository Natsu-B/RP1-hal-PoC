//! Bounded SCMI Base/Clock 2.0 server. One proc0 owner, no shared atomics.
//!
//! The backend must report physical state, NOT merely the desired profile.
//! No clock/PLL address is writable through this protocol. Mutation permission
//! is explicit and disabled during initial Base/RATE_GET transport commissioning.
use crate::clock_profile_generated::ClockPolicy;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    NotSupported = -1,
    InvalidParameters = -2,
    Denied = -3,
    NotFound = -4,
    Hardware = -9,
    Protocol = -10,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalState {
    pub rate_hz: u64,
    pub enabled: bool,
}

pub trait ClockHardware {
    fn read(&mut self, rp1_id: u32) -> Result<PhysicalState, Error>;
    /// Only known typed clock operations may implement this. On error, no
    /// logical vote is committed. Backend must handle/record partial failures.
    fn enable(&mut self, rp1_id: u32, enabled: bool) -> Result<(), Error>;
}

/// Response payload includes status; transport adds the unchanged SCMI header.
#[derive(Debug)]
pub struct Response {
    pub words: [u32; 8],
    pub len: usize,
}

impl Response {
    fn success(data: &[u32]) -> Self {
        let mut r = Self { words: [0; 8], len: data.len() + 1 };
        r.words[1..r.len].copy_from_slice(data);
        r
    }
    fn error(error: Error) -> Self {
        Self { words: [error as i32 as u32, 0, 0, 0, 0, 0, 0, 0], len: 1 }
    }
    fn name(prefix: &[u32], text: &[u8], suffix: &[u32]) -> Self {
        let mut data = [0; 7];
        data[..prefix.len()].copy_from_slice(prefix);
        for (i, b) in text.iter().take(15).enumerate() {
            data[prefix.len() + i / 4] |= (*b as u32) << ((i % 4) * 8);
        }
        let end = prefix.len() + 4;
        data[end..end + suffix.len()].copy_from_slice(suffix);
        Self::success(&data[..end + suffix.len()])
    }
}

pub struct Server<'a> {
    clocks: &'a [ClockPolicy],
    linux_votes: u32,
    allow_changes: bool,
    pub config_requests: u32,
}

impl<'a> Server<'a> {
    pub fn new(clocks: &'a [ClockPolicy], allow_changes: bool) -> Result<Self, Error> {
        let mut ids = 0u32;
        let mut count = 0;
        for c in clocks {
            if let Some(id) = c.scmi_id {
                if id >= 32 || ids & (1 << id) != 0 || c.rate_hz == 0 {
                    return Err(Error::InvalidParameters);
                }
                ids |= 1 << id;
                count += 1;
            }
        }
        if count > 0 && ids != u32::MAX >> (32 - count) {
            return Err(Error::InvalidParameters);
        }
        Ok(Self { clocks, linux_votes: 0, allow_changes, config_requests: 0 })
    }

    pub fn linux_votes(&self) -> u32 { self.linux_votes }

    fn clock(&self, id: u32) -> Result<&ClockPolicy, Error> {
        self.clocks.iter().find(|c| c.scmi_id == Some(id)).ok_or(Error::NotFound)
    }

    fn state(c: &ClockPolicy, hw: &mut impl ClockHardware) -> Result<PhysicalState, Error> {
        let state = hw.read(c.rp1_id)?;
        if state.rate_hz != c.rate_hz as u64 || ((c.boot_required || c.rp1_required) && !state.enabled) {
            return Err(Error::Hardware);
        }
        Ok(state)
    }

    /// Header token/protocol are preserved by the transport. Exact request
    /// lengths and all reserved flag bits are checked; unknown IDs fail closed.
    pub fn request(&mut self, header: u32, args: &[u32], hw: &mut impl ClockHardware) -> Response {
        match self.dispatch(header, args, hw) {
            Ok(r) => r,
            Err(e) => Response::error(e),
        }
    }

    fn dispatch(&mut self, header: u32, a: &[u32], hw: &mut impl ClockHardware) -> Result<Response, Error> {
        if header & 0xf000_0300 != 0 { return Err(Error::Protocol); }
        let protocol = (header >> 10) & 0xff;
        let message = header & 0xff;
        let required = match (protocol, message) {
            (0x10, 0 | 1 | 3 | 4 | 5) | (0x14, 0 | 1) => 0,
            (0x10, 2 | 6 | 7) | (0x14, 2 | 3 | 6) => 1,
            (0x14, 4 | 7) => 2,
            (0x14, 5) => 4,
            _ => return Err(Error::NotSupported),
        };
        if a.len() != required { return Err(Error::InvalidParameters); }
        let r = match (protocol, message) {
            (_, 0) => Response::success(&[0x0002_0000]),
            (0x10, 1) => Response::success(&[0x0201]), // two agents, one non-Base protocol
            (0x10, 2) => {
                if a[0] > 7 { return Err(Error::NotSupported); }
                Response::success(&[0])
            }
            (0x10, 3) => Response::name(&[], b"RP1-PoC", &[]),
            (0x10, 4) => Response::name(&[], b"Natsu-B", &[]),
            (0x10, 5) => Response::success(&[1]),
            (0x10, 6) => match a[0] {
                0 => Response::success(&[1, 0x14]),
                1 => Response::success(&[0]),
                _ => return Err(Error::InvalidParameters),
            },
            (0x10, 7) => match a[0] {
                0 => Response::name(&[0], b"RP1-FW", &[]),
                1 | 0xffff_ffff => Response::name(&[1], b"OSPM", &[]),
                _ => return Err(Error::NotFound),
            },
            (0x14, 1) => Response::success(&[self.clocks.iter().filter(|c| c.scmi_id.is_some()).count() as u32]),
            (0x14, 2) => {
                if a[0] > 7 || (!self.allow_changes && matches!(a[0], 5 | 7)) {
                    return Err(Error::NotSupported);
                }
                Response::success(&[0])
            }
            (0x14, 3) => {
                let c = self.clock(a[0])?;
                let s = Self::state(c, hw)?;
                Response::name(&[u32::from(s.enabled)], c.name.as_bytes(), &[0])
            }
            (0x14, 4) => {
                let c = self.clock(a[0])?;
                match a[1] {
                    0 => Response::success(&[1, c.rate_hz, 0]), // one discrete locked rate
                    1 => Response::success(&[0]),
                    _ => return Err(Error::InvalidParameters),
                }
            }
            (0x14, 5) => {
                if !self.allow_changes { return Err(Error::Denied); }
                // No async completion advertised. Rounding flags may be accepted
                // only for the exact supported rate; never silently round/change.
                if a[0] & !12 != 0 { return Err(Error::InvalidParameters); }
                let c = self.clock(a[1])?;
                if a[2] != c.rate_hz || a[3] != 0 { return Err(Error::Denied); }
                Self::state(c, hw)?;
                Response::success(&[])
            }
            (0x14, 6) => {
                let c = self.clock(a[0])?;
                let s = Self::state(c, hw)?;
                Response::success(&[s.rate_hz as u32, (s.rate_hz >> 32) as u32])
            }
            (0x14, 7) => {
                if !self.allow_changes { return Err(Error::Denied); }
                if a[1] > 1 { return Err(Error::InvalidParameters); }
                let c = *self.clock(a[0])?;
                let before = Self::state(&c, hw)?;
                let desired = c.boot_required || c.rp1_required || a[1] != 0;
                if before.enabled != desired { hw.enable(c.rp1_id, desired)?; }
                if Self::state(&c, hw)?.enabled != desired { return Err(Error::Hardware); }
                if a[1] == 0 { self.linux_votes &= !(1 << a[0]); }
                else { self.linux_votes |= 1 << a[0]; }
                self.config_requests = self.config_requests.wrapping_add(1);
                Response::success(&[])
            }
            _ => return Err(Error::NotSupported),
        };
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock_profile_generated::CLOCKS;
    struct Hw { state: PhysicalState, writes: u32, fail: bool }
    impl ClockHardware for Hw {
        fn read(&mut self, _: u32) -> Result<PhysicalState, Error> { Ok(self.state) }
        fn enable(&mut self, _: u32, en: bool) -> Result<(), Error> {
            self.writes += 1;
            if self.fail { return Err(Error::Hardware); }
            self.state.enabled = en;
            Ok(())
        }
    }
    fn hw() -> Hw { Hw { state: PhysicalState { rate_hz: 100_000_000, enabled: true }, writes: 0, fail: false } }
    fn req(s: &mut Server, h: &mut Hw, proto: u32, id: u32, args: &[u32]) -> Response {
        s.request((17 << 18) | (proto << 10) | id, args, h)
    }
    #[test] fn discovery_and_rate() {
        let (mut s, mut h) = (Server::new(CLOCKS, false).unwrap(), hw());
        for id in [0, 1, 3, 4, 5] { assert_eq!(req(&mut s, &mut h, 0x10, id, &[]).words[0], 0); }
        assert_eq!(req(&mut s, &mut h, 0x10, 6, &[0]).words[..3], [0, 1, 0x14]);
        assert_eq!(req(&mut s, &mut h, 0x10, 7, &[0xffff_ffff]).words[..2], [0, 1]);
        assert_eq!(req(&mut s, &mut h, 0x14, 1, &[]).words[..2], [0, 1]);
        assert_eq!(req(&mut s, &mut h, 0x14, 4, &[0, 0]).words[..4], [0, 1, 100_000_000, 0]);
        assert_eq!(req(&mut s, &mut h, 0x14, 6, &[0]).words[..3], [0, 100_000_000, 0]);
        assert_eq!(h.writes, 0);
    }
    #[test] fn readonly_and_unknown_fail_closed() {
        let (mut s, mut h) = (Server::new(CLOCKS, false).unwrap(), hw());
        assert_eq!(req(&mut s, &mut h, 0x14, 7, &[0, 0]).words[0] as i32, -3);
        assert_eq!(req(&mut s, &mut h, 0x14, 6, &[99]).words[0] as i32, -4);
        assert_eq!(req(&mut s, &mut h, 0x14, 2, &[0x106]).words[0] as i32, -1);
        assert_eq!(req(&mut s, &mut h, 0x16, 0, &[]).words[0] as i32, -1);
        assert_eq!(req(&mut s, &mut h, 0x14, 6, &[0, 0]).words[0] as i32, -2);
        assert_eq!(h.writes, 0);
    }
    #[test] fn linux_disable_cannot_remove_firmware_vote() {
        let (mut s, mut h) = (Server::new(CLOCKS, true).unwrap(), hw());
        for en in [1, 0] {
            assert_eq!(req(&mut s, &mut h, 0x14, 7, &[0, en]).words[0], 0);
            assert_eq!(s.linux_votes(), en);
            assert!(h.state.enabled);
        }
        assert_eq!(h.writes, 0);
        assert_eq!(s.config_requests, 2);
    }
    #[test] fn wrong_physical_rate_is_not_reported_as_profile_rate() {
        let (mut s, mut h) = (Server::new(CLOCKS, true).unwrap(), hw());
        h.state.rate_hz = 99_000_000;
        assert_eq!(req(&mut s, &mut h, 0x14, 6, &[0]).words[0] as i32, -9);
        assert_eq!(req(&mut s, &mut h, 0x14, 7, &[0, 1]).words[0] as i32, -9);
        assert_eq!(s.linux_votes(), 0);
    }
    #[test] fn locked_rate_and_flags() {
        let (mut s, mut h) = (Server::new(CLOCKS, true).unwrap(), hw());
        assert_eq!(req(&mut s, &mut h, 0x14, 5, &[0, 0, 100_000_000, 0]).words[0], 0);
        assert_eq!(req(&mut s, &mut h, 0x14, 5, &[8, 0, 100_000_000, 0]).words[0], 0);
        assert_eq!(req(&mut s, &mut h, 0x14, 5, &[4, 0, 100_000_000, 0]).words[0], 0);
        assert_eq!(req(&mut s, &mut h, 0x14, 5, &[0, 0, 50_000_000, 0]).words[0] as i32, -3);
        assert_eq!(req(&mut s, &mut h, 0x14, 5, &[1, 0, 100_000_000, 0]).words[0] as i32, -2);
        assert_eq!(req(&mut s, &mut h, 0x14, 7, &[0, 0x100]).words[0] as i32, -2);
        assert_eq!(h.writes, 0);
    }
    #[test] fn backend_error_keeps_logical_vote() {
        let mut policy = *CLOCKS.iter().find(|c| c.scmi_id == Some(0)).unwrap();
        policy.boot_required = false; policy.rp1_required = false;
        let table = [policy];
        let (mut s, mut h) = (Server::new(&table, true).unwrap(), hw());
        h.fail = true;
        assert_eq!(req(&mut s, &mut h, 0x14, 7, &[0, 0]).words[0] as i32, -9);
        assert_eq!(s.config_requests, 0);
        assert!(h.state.enabled);
    }
}

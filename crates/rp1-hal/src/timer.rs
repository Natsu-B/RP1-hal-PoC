use core::hint::spin_loop;

use crate::addr::{RAW_TIMER_HIGH, RAW_TIMER_LOW};

/// RP1 free-running 64-bit raw timer with a one-microsecond tick.
pub struct RawTimer {
    _private: (),
}

impl RawTimer {
    pub(crate) const unsafe fn new() -> Self {
        Self { _private: () }
    }

    #[inline(always)]
    pub fn now(&self) -> u64 {
        loop {
            let high_before = unsafe { core::ptr::read_volatile(RAW_TIMER_HIGH as *const u32) };
            let low = unsafe { core::ptr::read_volatile(RAW_TIMER_LOW as *const u32) };
            let high_after = unsafe { core::ptr::read_volatile(RAW_TIMER_HIGH as *const u32) };
            if high_before == high_after {
                return combine(high_before, low);
            }
        }
    }

    #[inline(always)]
    pub fn elapsed_since(&self, start: u64) -> u64 {
        elapsed_us(start, self.now())
    }

    pub fn delay_us(&self, delay_us: u64) {
        let start = self.now();
        while self.elapsed_since(start) < delay_us {
            spin_loop();
        }
    }
}

const fn combine(high: u32, low: u32) -> u64 {
    (high as u64) << 32 | low as u64
}

const fn elapsed_us(start: u64, end: u64) -> u64 {
    end.wrapping_sub(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_timer_register_contract_matches_recovered_map() {
        assert_eq!(RAW_TIMER_HIGH, 0x400a_c024);
        assert_eq!(RAW_TIMER_LOW, 0x400a_c028);
    }

    #[test]
    fn combines_stable_high_and_low_words() {
        assert_eq!(combine(0x0123_4567, 0x89ab_cdef), 0x0123_4567_89ab_cdef);
    }

    #[test]
    fn elapsed_time_wraps_like_the_hardware_counter() {
        assert_eq!(elapsed_us(u64::MAX - 4, 7), 12);
    }
}

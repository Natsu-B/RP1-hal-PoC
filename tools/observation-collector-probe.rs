#![no_std]
use rp1_hal::bulk::observation::{Collector, Progress};

#[unsafe(no_mangle)]
#[inline(never)]
pub fn ObservationCollectorPoll(
    collector: &mut Collector<'static, 8>,
    descriptor: &mut [u32; 16],
) -> u32 {
    match collector.poll() {
        Ok(Progress::Idle) => 0,
        Ok(Progress::Blocked) => 1,
        Ok(Progress::Copying) => 2,
        Ok(Progress::Ready(d)) => {
            *descriptor = d.0;
            3
        }
        Err(_) => 4,
    }
}

#[unsafe(no_mangle)]
pub static OBSERVATION_COLLECTOR_BYTES: usize = core::mem::size_of::<Collector<'static, 8>>();

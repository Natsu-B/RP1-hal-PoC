//! Compile-only Cortex-M3 probe. Never deploy this library as firmware.
#![no_std]
use rp1_abi::amp::Error;
use rp1_hal::{
    local_control::{Decision, LocalControl, SampleStamp},
    local_queue::LocalQueue,
};

#[unsafe(no_mangle)]
pub fn local_queue_codegen_probe(queue: &mut LocalQueue<[u32; 24], 4>, value: [u32; 24]) -> u32 {
    let (mut producer, mut consumer) = queue.split();
    let full = producer.push(value).is_err();
    let value = consumer.pop().map_or(0, |v| v[0]);
    value ^ producer.drops() ^ u32::from(full)
}

#[unsafe(no_mangle)]
pub fn local_control_codegen_probe(
    control: &mut LocalControl,
    packet: &[u8],
    now: u64,
    sample: SampleStamp,
) -> u32 {
    let accepted = control
        .offer(packet, now, |p| {
            if p.0.iter().all(|v| *v <= 1) {
                Ok(())
            } else {
                Err(Error::Bounds)
            }
        })
        .is_ok();
    match control.step(now, sample) {
        Decision::Active { parameters, .. } => parameters.0[0] ^ u32::from(accepted),
        Decision::Inactive(_) => 0,
    }
}

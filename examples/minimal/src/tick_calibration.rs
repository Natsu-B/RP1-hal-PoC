//! Exact bounded SysTick/raw-timer conversion without a generic u64 divider.
//! This changes arithmetic only, not the sampling interval or clock source.
pub fn processor_hz(cycles: u32, dt_us: u32) -> u32 {
    assert!(cycles <= 0x00ff_ffff && (10_000..=11_000).contains(&dt_us));
    // cycles*100 <= 1,677,721,500; remainder*10000 < 110,000,000.
    // floor(n*10000/d) = (n/d)*10000 + floor((n%d)*10000/d).
    // The final result is <= 1,677,721,500, so every operation fits u32.
    let scaled = cycles * 100;
    (scaled / dt_us) * 10_000 + (scaled % dt_us) * 10_000 / dt_us
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_old_u64_result_and_boundaries() {
        fn check(c: u32, d: u32) {
            assert_eq!(processor_hz(c, d), (u64::from(c) * 1_000_000 / u64::from(d)) as u32);
        }
        // Every possible 24-bit counter delta at both interval endpoints.
        for dt in [10_000, 11_000] {
            for cycles in 0..=0x00ff_ffff { check(cycles, dt); }
        }
        // Every admitted dt, all residue values near zero and the maximum.
        for dt in 10_000..=11_000 {
            for cycles in 0..=11_000 {
                check(cycles, dt);
                check(0x00ff_ffff - cycles, dt);
            }
        }
        for (cycles, dt) in [(0x0100_0000, 10_000), (1, 0), (1, 9999), (1, 11_001)] {
            assert!(std::panic::catch_unwind(|| processor_hz(cycles, dt)).is_err());
        }
        // Same five-sample rounded average, including maximal admitted sum.
        for high_samples in 0..=5 {
            let values = core::array::from_fn::<_, 5, _>(|i|
                if i < high_samples {500_000_000u32} else {1_000_000});
            let narrow: u32 = values.iter().sum();
            let wide: u64 = values.iter().map(|&x| u64::from(x)).sum();
            assert_eq!((narrow / 5 + 500) / 1000 * 1000,
                       ((wide / 5 + 500) / 1000 * 1000) as u32);
        }
    }
}

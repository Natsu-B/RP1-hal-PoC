//! Finite local-SRAM SPI receipt recorder. Not a sensor-to-actuator trace.
//! One proc0 task writes; IRQ only supplies its separately synchronized stamps.
//! Host readers may consume ONLY a frozen snapshot, never the running buffer.
pub const CAPACITY: usize = 16;
pub const HEADER_WORDS: usize = 8;
pub const RECORD_WORDS: usize = 8;
pub const WORDS: usize = HEADER_WORDS + CAPACITY * RECORD_WORDS;
pub const MAGIC: u32 = u32::from_le_bytes(*b"SPT1");

/// Little-endian u32 words: generation, start, first IRQ, last IRQ end,
/// checked completion, IRQ count, success(0/1), reserved(0).
/// Raw timer stamps wrap at 2^32 us; each request must finish within 2^31 us.
#[repr(C)]
pub struct Trace { words: [u32; WORDS] }
impl Trace {
    pub const fn new() -> Self {
        let mut words = [0; WORDS];
        // magic, version, state(1 running/2 frozen), total, stored, dropped,
        // last request generation, errors (bit0 sequence; bit1 saturated).
        words[0] = MAGIC; words[1] = 1; words[2] = 1;
        Self { words }
    }
    fn put(&mut self, index: usize, value: u32) {
        // Volatile publication survives optimization when only an external
        // observer reads SRAM. This does not synchronize a concurrent consumer.
        unsafe { self.words.as_mut_ptr().add(index).write_volatile(value); }
    }
    fn increment(&mut self, index: usize) {
        if let Some(n) = self.words[index].checked_add(1) { self.put(index, n); }
        else { self.put(7, self.words[7] | 2); }
    }
    /// Drop-newest when full. Never wait, allocate, access DDR or alter SPI state.
    /// Returns false after freeze without changing the frozen snapshot.
    pub fn push(&mut self, record: [u32; RECORD_WORDS]) -> bool {
        if self.words[2] != 1 { return false; }
        self.increment(3);
        if record[0] == 0 || self.words[6].checked_add(1) != Some(record[0]) {
            self.put(7, self.words[7] | 1);
            self.increment(5);
            return false;
        }
        self.put(6, record[0]);
        let count = self.words[4] as usize;
        if count == CAPACITY { self.increment(5); return false; }
        for (i, value) in record.into_iter().enumerate() {
            self.put(HEADER_WORDS + count * RECORD_WORDS + i, value);
        }
        self.put(4, count as u32 + 1);
        true
    }
    /// Caller first stops this finite producer and completes SRAM writes (DSB
    /// on RP1). Freeze is not a DMA/IRQ/peripheral stop primitive.
    pub fn freeze(&mut self) { self.put(2, 2); }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn collector_stall_drops_newest_without_overwrite() {
        let mut t = Trace::new();
        for n in 1..=CAPACITY as u32 {
            assert!(t.push([n, n*200, n*200+2, n*200+20, n*200+24, 1, 1, 0]));
        }
        let data = t.words;
        for n in CAPACITY as u32+1..=100_000 {
            assert!(!t.push([n, 0, 0, 0, 0, 0, 0, 0]));
        }
        assert_eq!(&t.words[HEADER_WORDS..], &data[HEADER_WORDS..]);
        assert_eq!(&t.words[3..8], &[100_000,16,99_984,100_000,0]);
        t.freeze(); let frozen = t.words;
        assert!(!t.push([100_001,0,0,0,0,0,0,0]));
        assert_eq!(t.words, frozen);
    }
    #[test]
    fn generations_and_counter_saturation_are_visible() {
        let mut t = Trace::new();
        assert!(!t.push([0; RECORD_WORDS]));
        assert!(t.push([1, 0xffff_fff0, 0xffff_fff2, 5, 8, 1, 1, 0]));
        assert!(!t.push([1,0,0,0,0,0,0,0]));
        assert_eq!(&t.words[3..8], &[3,1,2,1,1]);
        t.words[3] = u32::MAX;
        t.push([2,0,0,0,0,0,0,0]);
        assert_eq!(t.words[3], u32::MAX);
        assert_eq!(t.words[7], 3);
    }
    #[test]
    fn export_host_fixture_from_actual_recorder() {
        let mut t = Trace::new();
        for n in 1..=CAPACITY as u32+3 {
            let start = 0xffff_fff0u32.wrapping_add((n-1)*200);
            t.push([n,start,start.wrapping_add(2),start.wrapping_add(20),start.wrapping_add(24),1,1,0]);
        }
        t.freeze();
        if let Some(path) = std::env::var_os("RP1_SPI_TIMING_FIXTURE") {
            let bytes: std::vec::Vec<u8> = t.words.into_iter().flat_map(u32::to_le_bytes).collect();
            std::fs::write(path, bytes).unwrap();
        }
    }
}

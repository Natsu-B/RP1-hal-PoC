//! Single-producer/single-consumer queue for RP1 local SRAM, never Linux DDR.
//! Acquire/release loads and stores use no compare-exchange or exclusive monitor.
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicU32, Ordering},
};
use rp1_abi::amp::Error;

pub struct LocalQueue<T: Copy, const N: usize> {
    slots: [UnsafeCell<MaybeUninit<T>>; N],
    head: AtomicU32,
    tail: AtomicU32,
    drops: AtomicU32,
}
// split's exclusive borrow grants exactly one writer and one reader. The producer
// cannot reuse a slot until the consumer has copied it and released tail.
unsafe impl<T: Copy + Send, const N: usize> Sync for LocalQueue<T, N> {}

pub struct Producer<'a, T: Copy, const N: usize> {
    queue: &'a LocalQueue<T, N>,
}
pub struct Consumer<'a, T: Copy, const N: usize> {
    queue: &'a LocalQueue<T, N>,
}

impl<T: Copy, const N: usize> LocalQueue<T, N> {
    pub const fn new() -> Self {
        assert!(N > 0 && N.is_power_of_two() && N <= 0x4000_0000);
        Self {
            slots: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            drops: AtomicU32::new(0),
        }
    }
    /// Place the storage in RP1 local SRAM before splitting. Handles cannot be
    /// cloned; keep one on each admitted owner. No reset while either is alive.
    pub fn split(&mut self) -> (Producer<'_, T, N>, Consumer<'_, T, N>) {
        (Producer { queue: self }, Consumer { queue: self })
    }
}

impl<T: Copy, const N: usize> Producer<'_, T, N> {
    /// Bounded drop-newest: does not wait for DDR or overwrite an unread sample.
    pub fn push(&mut self, value: T) -> Result<(), Error> {
        self.push_with(|| value)
    }
    /// Construct only an admitted record. A full queue counts the drop without
    /// invoking `make`; the sole consumer can only free space during construction.
    /// The caller must keep construction and captured destructors bounded.
    pub fn push_with(&mut self, make: impl FnOnce() -> T) -> Result<(), Error> {
        let q = self.queue;
        let head = q.head.load(Ordering::Relaxed);
        let tail = q.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= N as u32 {
            q.drops.store(
                q.drops.load(Ordering::Relaxed).saturating_add(1),
                Ordering::Relaxed,
            );
            return Err(Error::Full);
        }
        unsafe {
            (*q.slots[head as usize & (N - 1)].get()).write(make());
        }
        q.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }
    pub fn drops(&self) -> u32 {
        self.queue.drops.load(Ordering::Relaxed)
    }
}

impl<T: Copy, const N: usize> Consumer<'_, T, N> {
    /// Borrow the published head without releasing its slot. The exclusive
    /// consumer borrow prevents discard/pop until this reference is no longer used.
    pub fn front(&mut self) -> Option<&T> {
        let q=self.queue;let tail=q.tail.load(Ordering::Relaxed);
        if tail==q.head.load(Ordering::Acquire) {return None;}
        Some(unsafe {(*q.slots[tail as usize&(N-1)].get()).assume_init_ref()})
    }
    /// Release only after the collector completed its copy/publication.
    pub fn discard_front(&mut self) -> bool {
        let q=self.queue;let tail=q.tail.load(Ordering::Relaxed);
        if tail==q.head.load(Ordering::Acquire) {return false;}
        q.tail.store(tail.wrapping_add(1),Ordering::Release);true
    }
    /// Copy into collector-owned local storage before releasing this queue slot.
    /// Empty leaves `out` unchanged; no stack-sized Option<T> is returned.
    #[cfg_attr(feature="amp-observation",inline(never))]
    #[cfg_attr(feature="amp-observation",unsafe(link_section=".amp_observation_text"))]
    pub fn pop_into(&mut self, out: &mut MaybeUninit<T>) -> bool {
        let q = self.queue;
        let tail = q.tail.load(Ordering::Relaxed);
        if tail == q.head.load(Ordering::Acquire) {
            return false;
        }
        unsafe {
            core::ptr::copy_nonoverlapping(
                (*q.slots[tail as usize & (N - 1)].get()).as_ptr(),
                out.as_mut_ptr(),
                1,
            );
        }
        q.tail.store(tail.wrapping_add(1), Ordering::Release);
        true
    }
    pub fn pop(&mut self) -> Option<T> {
        let q = self.queue;
        let tail = q.tail.load(Ordering::Relaxed);
        if tail == q.head.load(Ordering::Acquire) {
            return None;
        }
        let value = unsafe { (*q.slots[tail as usize & (N - 1)].get()).assume_init_read() };
        q.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }
    /// Saturating drop count; independent of the published slot's visibility.
    pub fn drops(&self) -> u32 {
        self.queue.drops.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    #[test]
    fn borrowed_front_is_not_reused_until_explicit_release() {
        let mut q=LocalQueue::<[u64;8],2>::new();
        q.head.store(u32::MAX,Ordering::Relaxed);q.tail.store(u32::MAX,Ordering::Relaxed);
        let (mut p,mut c)=q.split();
        assert!(c.front().is_none());assert!(!c.discard_front());
        p.push([1;8]).unwrap();p.push([2;8]).unwrap();
        {let head=c.front().unwrap();assert_eq!(p.push([3;8]),Err(Error::Full));assert_eq!(head,&[1;8]);}
        assert!(c.discard_front());p.push([3;8]).unwrap();
        for value in [2,3] {assert_eq!(c.front(),Some(&[value;8]));assert!(c.discard_front());}
        assert!(c.front().is_none());assert!(!c.discard_front());
    }
    #[test]
    fn pop_into_preserves_empty_output_and_copies_across_index_wrap() {
        let mut q = LocalQueue::<[u64; 8], 2>::new();
        q.head.store(u32::MAX, Ordering::Relaxed);
        q.tail.store(u32::MAX, Ordering::Relaxed);
        let (mut p, mut c) = q.split();
        let mut out = MaybeUninit::new([99; 8]);
        assert!(!c.pop_into(&mut out));
        assert_eq!(unsafe { out.assume_init_ref() }, &[99; 8]);
        p.push([1; 8]).unwrap();
        p.push([2; 8]).unwrap();
        assert!(c.pop_into(&mut out));
        assert_eq!(unsafe { out.assume_init_ref() }, &[1; 8]);
        p.push([3; 8]).unwrap();
        for expected in [2, 3] {
            assert!(c.pop_into(&mut out));
            assert_eq!(unsafe { out.assume_init_ref() }, &[expected; 8]);
        }
        assert!(!c.pop_into(&mut out));
        assert_eq!(unsafe { out.assume_init_ref() }, &[3; 8]);
    }
    #[test]
    fn full_queue_skips_construction_and_preserves_unread_records() {
        let mut q = LocalQueue::<[u64; 4], 2>::new();
        let (mut p, mut c) = q.split();
        let mut constructed = 0;
        for i in 1..=2 {
            p.push_with(|| {
                constructed += 1;
                [i; 4]
            })
            .unwrap();
        }
        for i in 3..=1002 {
            assert_eq!(
                p.push_with(|| {
                    constructed += 1;
                    [i; 4]
                }),
                Err(Error::Full)
            );
        }
        assert_eq!(constructed, 2);
        assert_eq!(p.drops(), 1000);
        assert_eq!(c.pop(), Some([1; 4]));
        p.push_with(|| {
            constructed += 1;
            [1003; 4]
        })
        .unwrap();
        assert_eq!(constructed, 3);
        assert_eq!(c.pop(), Some([2; 4]));
        assert_eq!(c.pop(), Some([1003; 4]));
        assert_eq!(c.pop(), None);
    }
    #[test]
    fn stopped_collector_preserves_old_data_and_reports_drop_newest() {
        let mut q = LocalQueue::<[u64; 4], 4>::new();
        let (mut p, mut c) = q.split();
        for i in 1..=4 {
            p.push([i; 4]).unwrap();
        }
        for i in 5..=1004 {
            assert_eq!(p.push([i; 4]), Err(Error::Full));
        }
        assert_eq!(c.drops(), 1000);
        for i in 1..=4 {
            assert_eq!(c.pop(), Some([i; 4]));
        }
        assert_eq!(c.pop(), None);
        p.push([1005; 4]).unwrap();
        assert_eq!(c.pop(), Some([1005; 4]));
    }
    #[test]
    fn index_wrap_and_saturating_drops() {
        let mut q = LocalQueue::<u32, 4>::new();
        q.head.store(u32::MAX - 1, Ordering::Relaxed);
        q.tail.store(u32::MAX - 1, Ordering::Relaxed);
        q.drops.store(u32::MAX, Ordering::Relaxed);
        let (mut p, mut c) = q.split();
        for i in 0..4 {
            p.push(i).unwrap();
        }
        assert_eq!(p.push(4), Err(Error::Full));
        assert_eq!(p.drops(), u32::MAX);
        for i in 0..4 {
            assert_eq!(c.pop(), Some(i));
        }
        p.push(5).unwrap();
        assert_eq!(c.pop(), Some(5));
    }
    #[test]
    fn concurrent_multiword_records_are_complete_and_in_order() {
        let mut q = LocalQueue::<[u64; 8], 16>::new();
        let (mut p, mut c) = q.split();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                for i in 1..=100_000 {
                    while p.push([i; 8]).is_err() {
                        std::thread::yield_now();
                    }
                }
            });
            scope.spawn(move || {
                let mut out = MaybeUninit::uninit();
                for i in 1..=100_000 {
                    loop {
                        if i % 3 == 0 && c.pop_into(&mut out) {
                            assert_eq!(unsafe { out.assume_init_ref() }, &[i; 8]);
                            break;
                        } else if i % 3 == 1 {
                            if let Some(value) = c.pop() {
                                assert_eq!(value, [i; 8]);
                                break;
                            }
                        } else if i % 3 == 2 {
                            if let Some(value)=c.front() {
                                assert_eq!(value,&[i;8]);
                                assert!(c.discard_front());
                                break;
                            }
                        }
                        std::thread::yield_now();
                    }
                }
            });
        });
    }
    #[test]
    #[should_panic]
    fn non_power_of_two_capacity_rejected() {
        let _ = LocalQueue::<u32, 3>::new();
    }
}

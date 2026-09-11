//! Fixed finite load, not maximum-throughput or a real sensor acquisition claim.
pub const REQUESTS: u32 = 384;
pub const CYCLE_TICKS: u32 = 10_000;
pub const SECOND_TICKS: u32 = 4500;
pub const REQUEST_TIMEOUT: u32 = 4500;
pub const SPI_WAIT_TICKS: u32 = CYCLE_TICKS-SECOND_TICKS+REQUEST_TIMEOUT+100;
pub fn release_offset(sequence: u32) -> u32 {
    assert!((1..=REQUESTS).contains(&sequence));
    (sequence-1)/2*CYCLE_TICKS + (sequence-1)%2*SECOND_TICKS
}
pub fn wire_id(sequence: u32) -> u32 {
    assert!((1..=REQUESTS).contains(&sequence));
    (sequence-1)%2+1
}
pub fn token(bytes: &mut [u8], sequence: u32) {
    assert!((1..=REQUESTS).contains(&sequence) && bytes.len()>=6);
    let end=bytes.len()-2;
    assert_eq!(&bytes[end..], b"\r\n");
    let mut value=sequence;
    for n in (end-4..end).rev() { bytes[n]=b'0'+(value%10) as u8;value/=10; }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn every_release_and_wire_token() {
        let mut previous=0;
        for s in 1..=REQUESTS {
            let offset=release_offset(s);
            assert!(s==1 || offset>previous);previous=offset;
            assert_eq!(wire_id(s),if s%2==1 {1} else {2});
            let mut t=*b"HOST2RP1 IRQ 0000\r\n";token(&mut t,s);
            let n=t[13..17].iter().fold(0u32,|a,b|a*10+u32::from(b-b'0'));
            assert_eq!(n,s);assert_eq!(&t[..13],b"HOST2RP1 IRQ ");
        }
        assert_eq!(release_offset(REQUESTS),1_914_500);
        assert!(SPI_WAIT_TICKS>=CYCLE_TICKS-SECOND_TICKS+REQUEST_TIMEOUT+100);
        let start=u32::MAX-3000;
        assert_eq!(start.wrapping_add(release_offset(3)).wrapping_sub(start),10_000);
    }
    #[test] #[should_panic] fn rejects_zero() { release_offset(0); }
    #[test] #[should_panic] fn rejects_excess() { wire_id(REQUESTS+1); }
}

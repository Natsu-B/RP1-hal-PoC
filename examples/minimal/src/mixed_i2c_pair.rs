//! IRP2 pair / optional bounded IRP3 stream, readiness before every UART token.
//! Static queues keep readiness/ACK separate from IRQ driver notification0.
use super::*;
const B:usize=I2C_BASE;
static mut READY:Option<U32Queue>=None;
static mut DONE:Option<U32Queue>=None;

pub unsafe fn prepare() { unsafe {
    ptr::addr_of_mut!(READY).write(Some(U32Queue::create(2,1).unwrap()));
    ptr::addr_of_mut!(DONE).write(Some(U32Queue::create(3,1).unwrap()));
} }

pub unsafe fn grant_and_wait(sequence:u32) { unsafe {
    // Validated host token grants this generation. Normal UART ACK follows
    // checked I2C completion, not just peer readiness or queue acceptance.
    assert!((1..=REQUESTS).contains(&sequence));
    let ready=ptr::addr_of!(READY).read().unwrap();
    let done=ptr::addr_of!(DONE).read().unwrap();
    assert!(ready.send(sequence,0).unwrap());
    assert!(done.receive(1000).unwrap()==Some(sequence));
} }

// PAIR_FRAME_BEGIN: compiled directly by the small host check.
fn frame_length(generation:u32)->Option<usize> {
    if !(1..=REQUESTS).contains(&generation) {return None;}
    Some(if generation&1==1 {2} else {31})
}
fn frame_byte(generation:u32,index:usize)->Option<u8> {
    let len=frame_length(generation)?;
    if index>=len {return None;}
    let frame=generation-1;
    Some(if cfg!(feature = "freertos-r2-mixed-i2c-stream") {
        match index {
            0=>(frame as u8)^0x31,
            1=>((frame>>8) as u8)^0x4e,
            _=>(0xb4+index as u32*0x1d+frame*7) as u8,
        }
    } else {(0x31u32+frame*0x83+index as u32*0x1d) as u8})
}
// PAIR_FRAME_END

pub unsafe extern "C" fn worker(_: *mut c_void) { unsafe {
    enter(B,if cfg!(feature = "freertos-r2-mixed-i2c-stream") {*b"ICMS"} else {*b"ICMP"},7);
    let driver=os::i2c1::Driver::new(ptr::addr_of_mut!(I2C).replace(None).unwrap());
    let ready=ptr::addr_of!(READY).read().unwrap();
    let done=ptr::addr_of!(DONE).read().unwrap();
    put(B+17,u32::MAX); put(B+31,0x2d00_0000|REQUESTS);
    let mut buffer=Buffer::<32>::new();
    for generation in 1..=REQUESTS {
        assert!(ready.receive(30_000).unwrap()==Some(generation));
        let length=frame_length(generation).unwrap();
        buffer.bytes.fill(0xc3); put(B+1,2);
        let started=raw_low(); if generation==1 {put(B+8,started);}
        let result=driver.receive(0x2d,&mut buffer.bytes[..length],50);
        put(B+9,raw_low()); buffer.canaries(B);
        let r=driver.last_receipt().unwrap();
        maxima(B,r.irq_entries,r.elapsed_us,r.irq_body_max_us,r.irq_end_to_task_us);
        put(B+10,r.generation);
        put(B+12,get(B+12)|r.first_fatal_causes); put(B+13,get(B+13)|r.first_abort_source);
        put(B+16,get(B+16).max(r.cleanup_elapsed_us));
        put(B+17,get(B+17).min(r.quiet_samples)); put(B+18,get(B+18).max(r.quiet_max_gap_us));
        put(B+19,get(B+19).checked_add(r.received).unwrap());
        put(B+20,get(B+20).checked_add(r.higher_priority_wakes).unwrap());
        let bytes=ptr::addr_of!(buffer.bytes).read_volatile();
        if generation==1 {put(B+21,u32::from_be_bytes(bytes[..4].try_into().unwrap()));}
        else {for i in 0..8 {put(B+22+i,u32::from_be_bytes(bytes[i*4..i*4+4].try_into().unwrap()));}}
        if result.is_err() {increment(B+11);}
        let _receipt=result.ok().unwrap();
        assert!(r.generation==generation && r.received as usize==length && r.irq_entries>0);
        assert!(r.first_fatal_causes==0 && r.first_abort_source==0 && r.discarded_after_failure==0);
        for (i,byte) in bytes.into_iter().enumerate() {
            assert!(byte==frame_byte(generation,i).unwrap_or(0xc3));
        }
        assert!(os::i2c1::active_generation()==0 && !os::i2c1::cancel(generation));
        assert!(get(B+30)==24 && r.higher_priority_wakes>0);
        increment(B+2); put(B+1,3);
        assert!(done.send(generation,0).unwrap());
    }
    assert!(ready.receive(0).unwrap().is_none());
    complete(B)
} }

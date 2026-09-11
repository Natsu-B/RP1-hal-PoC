#!/usr/bin/env python3
"""Actual pair arithmetic/queue handshake with host stubs; not scheduler/HW proof."""
from pathlib import Path
import re
import subprocess
import tempfile

if not __debug__:raise SystemExit('assertions required')
root=Path(__file__).resolve().parents[1]
source=(root/'examples/minimal/src/mixed_i2c_pair.rs').read_text()
mixed=(root/'examples/minimal/src/freertos_mixed.rs').read_text()
r1=(root/'examples/minimal/src/freertos_r1.rs').read_text()
assert 'U32Queue::create(2,1)' in source and 'U32Queue::create(3,1)' in source
assert 'notification_' not in source
assert r1.index('mixed::pair::prepare()')<r1.index('let entries:')
assert mixed.index('os::uart0::active_generation() == 0')<mixed.index('pair::grant_and_wait(sequence)')<mixed.index('driver.write_all(&ack')
assert source.index('ready.receive(30_000)')<source.index('driver.receive(0x2d')
assert source.index('os::i2c1::active_generation()==0')<source.index('done.send(generation,0)')

def function(name):
    start=source.index('pub unsafe fn '+name+'('); depth=0
    for m in re.finditer(r'[{}]',source[start:]):
        depth+=1 if m[0]=='{' else -1
        if not depth:return source[start:start+m.end()]
    raise AssertionError(name)

frame=source.split('// PAIR_FRAME_BEGIN:',1)[1].split('\n',1)[1].split('// PAIR_FRAME_END',1)[0]
program=r'''
use core::ptr;
use std::sync::{Mutex,atomic::{AtomicBool,AtomicU32,Ordering::SeqCst}};
#[derive(Clone,Copy)] struct U32Queue(u32);
static mut READY:Option<U32Queue>=None;
static mut DONE:Option<U32Queue>=None;
static SENT:AtomicBool=AtomicBool::new(true);
static NEXT:AtomicU32=AtomicU32::new(1);
static CALLS:Mutex<Vec<(u32,u32,u32)>>=Mutex::new(Vec::new());
impl U32Queue {
    unsafe fn create(slot:u32,capacity:u32)->Result<Self,()> {
        assert!(capacity==1 && (slot==2 || slot==3));Ok(Self(slot))
    }
    unsafe fn send(self,value:u32,ticks:u32)->Result<bool,()> {
        CALLS.lock().unwrap().push((self.0,value,ticks));Ok(SENT.load(SeqCst))
    }
    unsafe fn receive(self,ticks:u32)->Result<Option<u32>,()> {
        CALLS.lock().unwrap().push((self.0,0,ticks));
        let n=NEXT.load(SeqCst);Ok(if n==0 {None} else {Some(n)})
    }
}
'''+frame+function('prepare')+'\n'+function('grant_and_wait')+r'''
fn main() {
    std::panic::set_hook(Box::new(|_|{}));
    assert!(frame_length(0).is_none() && frame_length(3).is_none());
    assert!(frame_byte(u32::MAX,usize::MAX).is_none());
    for (g,n) in [(1,2),(2,31)] {
        assert_eq!(frame_length(g),Some(n));
        for i in 0..n {assert_eq!(frame_byte(g,i),Some((0x31+(g-1)*0x83+i as u32*0x1d) as u8));}
        assert_eq!(frame_byte(g,n),None);
    }
    assert_eq!(frame_byte(1,0),Some(0x31));assert_eq!(frame_byte(1,1),Some(0x4e));
    unsafe {prepare();}
    for g in [1,2] {
        NEXT.store(g,SeqCst);CALLS.lock().unwrap().clear();
        unsafe {grant_and_wait(g);}
        assert_eq!(*CALLS.lock().unwrap(),vec![(2,g,0),(3,0,1000)]);
    }
    for (g,sent,next) in [(0,true,1),(3,true,1),(1,false,1),(1,true,0),(1,true,2)] {
        SENT.store(sent,SeqCst);NEXT.store(next,SeqCst);CALLS.lock().unwrap().clear();
        assert!(std::panic::catch_unwind(||unsafe {grant_and_wait(g)}).is_err());
        assert_eq!(CALLS.lock().unwrap().len(),if !(1..=2).contains(&g) {0} else if !sent {1} else {2});
    }
    println!("STATIC pair actual arithmetic/queue handshake: two payloads, five refusals, bounded waits PASS");
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-i2c-pair-test-') as d:
    p=Path(d);(p/'main.rs').write_text(program)
    subprocess.run(['rustc','+stable','--edition=2024','-O','-Dwarnings',str(p/'main.rs'),'-o',str(p/'check')],check=True)
    subprocess.run([str(p/'check')],check=True)

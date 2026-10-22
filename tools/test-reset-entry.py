#!/usr/bin/env python3
"""Reuse the existing disabled-handoff fixture; all evidence here is synthetic."""
from pathlib import Path
import runpy

T=runpy.run_path(str(Path(__file__).with_name('test-watchdog-postack.py')))
M=runpy.run_path(str(Path(__file__).with_name('check-reset-entry.py')))
nonce=0x1357
request=[int.from_bytes(b'WQ06','little'),6,1,1,0xffffff,256,nonce,0x57445432^6^1^1^0xffffff^256^nonce]
ack=[int.from_bytes(b'QA06','little'),6,1,2,0,0,nonce,0x57445432^6^1^2^nonce]
w=T['w'].copy();w[96]=int.from_bytes(b'WDT6','little');w[97]=6;w[139]=nonce;w[176:184]=request
text=T['uart'](w).replace('[WQ4]','[WQ6]')
for old,new in [(T['REQUEST'],request),(T['ACK'],ack)]:
    text=text.replace(' '.join(f'{x:08x}' for x in old),' '.join(f'{x:08x}' for x in new))
def packet(n=nonce,reason=2,kind=0xb):
    v=(kind<<28)|(n<<12)|(reason<<4);check=0
    for shift in range(4,32,4):check^=(v>>shift)&15
    return v|check
events=T['trace']([packet()],bits=32)
def run(t=text,ev=events):return M['validate'](t,*T['inputs'](ev))
assert run()['frame']['nonce']==nonce
negative=0
def reject(t=text,ev=events,args=None):
    global negative
    try:M['validate'](t,*(T['inputs'](ev) if args is None else args))
    except (AssertionError,ValueError,KeyError,TypeError):negative+=1
    else:raise AssertionError('invalid software-entry capture accepted')
for n in range(len(events)):reject(ev=events[:n])
for bit in range(32):reject(ev=T['trace']([packet()^(1<<bit)],bits=32))
for word in [packet(n=1),packet(n=0),packet(reason=1),packet(reason=255),packet(kind=7)]:
    reject(ev=T['trace']([word],bits=32))
reject(ev=T['trace']([packet(),packet()],bits=32))
for field in ['request','record 000','record 252','ack-issued','observer-quiesced']:
    reject(t=text.replace('[WQ6] '+field,'absent '+field))
reject(t=text.replace('[WQ6]','[WQ5]'))
for index in [139,176,182,183,184]:
    old=f'[WQ6] record {index//4*4:03} '+' '.join(f'{x:08x}' for x in w[index//4*4:index//4*4+4])
    bad=w.copy();bad[index]^=1
    new=f'[WQ6] record {index//4*4:03} '+' '.join(f'{x:08x}' for x in bad[index//4*4:index//4*4+4])
    reject(t=text.replace(old,new))
protocol,before,after=T['inputs'](events)
for args in [(protocol,before,after.replace('overflow=0','overflow=1')),
             (protocol,before,after.replace(f'count={len(events)}','count=1'))]:reject(args=args)
# Instrumentation errors must not become a valid nonce/entry claim.
from copy import deepcopy
for index,field,value in [(10,'timestamp_us',events[9]['timestamp_us']+100000),
                           (11,'timestamp_us',events[10]['timestamp_us']+100000),
                           (10,'level',1),(10,'edge','rising'),
                           (10,'timestamp_us',events[9]['timestamp_us']),
                           (len(events)-1,'timestamp_us',events[-2]['timestamp_us']+100000)]:
    bad=deepcopy(events);bad[index][field]=value;reject(ev=bad)
# Earlier complete wrong frame is never skipped to select a later good frame.
reject(ev=T['trace']([packet(n=1),packet()],bits=32))
for n in [1,0xffff]:
    assert M['decode'](T['trace']([packet(n=n)],bits=32),n,2)['nonce']==n
# The known boot transient is ZERO-width, not an entry's leading ONE.
noise=[dict(seq=0,timestamp_us=0,source='gpio25',level=0,edge='falling'),
       dict(seq=1,timestamp_us=600000,source='gpio25',level=1,edge='rising'),
       dict(seq=2,timestamp_us=639203,source='gpio25',level=0,edge='falling')]
shifted=[dict(e,seq=e['seq']+3,timestamp_us=e['timestamp_us']+1000000) for e in events[1:]]
noise+=shifted
for i,e in enumerate(noise):e['seq']=i
assert run(ev=noise)['frame']['nonce']==nonce
print(f'PASS: software-entry synthetic positive / {negative} refusals; NOT HW/reset/runtime restart')

#!/usr/bin/env python3
"""Synthetic WDT7 mixed-frame mutations, plus existing WDT4/5/6 regression."""
from copy import deepcopy
from pathlib import Path
import runpy

A=runpy.run_path(str(Path(__file__).with_name('test-reset-entry.py')))
T=A['T']; M=runpy.run_path(str(Path(__file__).with_name('check-expiry-entry.py')))
nonce=A['nonce']
request=[int.from_bytes(b'WQ07','little'),7,1,1,0xffffff,256,nonce,0x57445432^7^1^1^0xffffff^256^nonce]
ack=[int.from_bytes(b'QA07','little'),7,1,2,0,0,nonce,0x57445432^7^1^2^nonce]
w=A['w'].copy(); w[96]=int.from_bytes(b'WDT7','little'); w[97]=7;w[176:184]=request
text=T['uart'](w).replace('[WQ4]','[WQ7]')
for old,new in [(T['REQUEST'],request),(T['ACK'],ack)]:
    text=text.replace(' '.join(f'{x:08x}' for x in old),' '.join(f'{x:08x}' for x in new))

def trace(word=None,interval=16_777_215,arm=0xa21c):
    first=T['trace']([arm]); second=T['trace']([A['packet'](reason=1) if word is None else word],bits=32)
    start=9
    shift=first[start]['timestamp_us']+interval-second[start]['timestamp_us']
    events=first+[dict(e,timestamp_us=e['timestamp_us']+shift) for e in second[start:]]
    for i,e in enumerate(events):e['seq']=i
    return events

events=trace()
def run(t=text,ev=events):return M['validate'](t,*T['inputs'](ev))
assert run()['result']==M['PASS']
assert run(ev=trace(A['packet'](reason=3)))['result']==M['PASS']
negative=0
def reject(t=text,ev=events,args=None):
    global negative
    try:M['validate'](t,*(T['inputs'](ev) if args is None else args))
    except (AssertionError,ValueError,KeyError,TypeError):negative+=1
    else:raise AssertionError('invalid expiry capture accepted')

for n in range(len(events)):
    if n==43:assert run(ev=events[:n])['result']=='INCONCLUSIVE_ARMED_ONLY'
    else:reject(ev=events[:n])
for bit in range(32):reject(ev=trace(A['packet'](reason=1)^(1<<bit)))
for bit in range(16):reject(ev=trace(arm=0xa21c^(1<<bit)))
for word in [A['packet'](reason=r) for r in (0,2,4,255)]+[A['packet'](n=0,reason=1),A['packet'](n=1,reason=1)]:
    reject(ev=trace(word))
for dt in [16_000_000,19_999_999]:assert run(ev=trace(interval=dt))['result']==M['PASS']
for dt in [15_999_999,20_000_000]:reject(ev=trace(interval=dt))
for words in [[0xa41a],[0xa21c,0xa41a],[0xa21c]]:
    assert run(ev=T['trace'](words))['result'].startswith('INCONCLUSIVE_')
assert run(ev=T['trace'](interval=19_000_000))['result']=='SAFE_NEGATIVE_ZERO_ALIVE_DISABLED'
reject(ev=T['trace'](interval=16_777_215))
reject(ev=A['events']) # software-only entry is not actual expiry
for part in ['request','record 252','ack-issued','observer-quiesced']:
    reject(t=text.replace('[WQ7] '+part,'absent '+part))
for n in [139,176,182,183,184]:
    line=f'[WQ7] record {n//4*4:03} '+' '.join(f'{x:08x}' for x in w[n//4*4:n//4*4+4])
    bad=w.copy();bad[n]^=1
    other=f'[WQ7] record {n//4*4:03} '+' '.join(f'{x:08x}' for x in bad[n//4*4:n//4*4+4])
    reject(t=text.replace(line,other))
for index in range(9,len(events)):
    bad=deepcopy(events[:index]+events[index+1:])
    for i,e in enumerate(bad):e['seq']=i
    reject(ev=bad)
for index,field,value in [(10,'level',1),(10,'edge','rising'),(10,'timestamp_us',0),
                         (len(events)-1,'timestamp_us',events[-2]['timestamp_us']+100000)]:
    bad=deepcopy(events);bad[index][field]=value;reject(ev=bad)
protocol,before,after=T['inputs'](events)
reject(args=(protocol,before,after.replace('overflow=0','overflow=1')))
reject(args=(protocol,before,after.replace(f'count={len(events)}','count=1')))
reject(ev=events+T['trace']([0xa41a]))
for n in [1,0xffff]:
    assert M['decode'](trace(A['packet'](n=n,reason=1)),n)['result']==M['PASS']
print(f'PASS: WDT7 synthetic mixed-frame positive and {negative} refusals; NOT HW')

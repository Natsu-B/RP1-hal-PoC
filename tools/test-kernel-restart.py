#!/usr/bin/env python3
"""Synthetic WDT8 adversarial checks plus unchanged WDT4/5/6/7 regressions."""
from copy import deepcopy
from pathlib import Path
import runpy
import subprocess
import sys
from tempfile import TemporaryDirectory

A=runpy.run_path(str(Path(__file__).with_name('test-expiry-entry.py')))
T=A['T']; B=A['A']; M=runpy.run_path(str(Path(__file__).with_name('check-kernel-restart.py')))
nonce=A['nonce']
request=[int.from_bytes(b'WQ08','little'),8,1,1,0xffffff,256,nonce,0x57445432^8^1^1^0xffffff^256^nonce]
ack=[int.from_bytes(b'QA08','little'),8,1,2,0,0,nonce,0x57445432^8^1^2^nonce]
w=A['w'].copy();w[96]=int.from_bytes(b'WDT8','little');w[97]=8;w[176:184]=request
text=T['uart'](w).replace('[WQ4]','[WQ8]')
for old,new in [(T['REQUEST'],request),(T['ACK'],ack)]:
    text=text.replace(' '.join(f'{x:08x}' for x in old),' '.join(f'{x:08x}' for x in new))


def packet(n=nonce,reason=1,kind=0xc):return B['packet'](n=n,reason=reason,kind=kind)
def trace(word=None,interval=22_277_215,arm=0xa21c):
    return A['trace'](packet() if word is None else word,interval=interval,arm=arm)


events=trace();positive=negative=inconclusive=0
def run(t=text,ev=events,admitted=True):
    return M['validate'](t,*T['inputs'](ev),fixed_source_admitted=admitted)
def accept(t=text,ev=events):
    global positive
    result=run(t,ev)
    assert result['result']==M['PASS'] and result['runtime_restart_proven']
    assert result['hardware_reset_entry_proven'] and result['fixed_source_admitted']
    assert result['R3']=='OPEN' and not result['exact_progress_counters_externally_sampled']
    assert not result['post_ack_registers_read_by_host'];positive+=1
def nonpass(ev=events,admitted=False,expected='INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED'):
    global inconclusive
    result=run(ev=ev,admitted=admitted)
    assert result['result']==expected and not result['runtime_restart_proven']
    assert not result['hardware_reset_entry_proven'];inconclusive+=1
def reject(t=text,ev=events,args=None,admitted=True):
    global negative
    try:M['validate'](t,*(T['inputs'](ev) if args is None else args),fixed_source_admitted=admitted)
    except (AssertionError,ValueError,KeyError,TypeError):negative+=1
    else:raise AssertionError('invalid kernel-restart capture accepted')


accept();accept(ev=trace(packet(reason=3)));nonpass()
assert M['validate'](text,*T['inputs'](events))['result']=='INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED'
for bad in (1,'true',None):reject(admitted=bad)
for reason in (1,3):
    assert packet(reason=reason)==B['packet'](reason=reason)^0x70000007
for dt in (21_000_000,25_999_999):accept(ev=trace(interval=dt))
for dt in (0,16_777_215,20_999_999,26_000_000):reject(ev=trace(interval=dt))
for n in range(len(events)):
    if n==43:nonpass(ev=events[:n],admitted=True,expected='INCONCLUSIVE_ARMED_ONLY')
    else:reject(ev=events[:n])
for bit in range(32):reject(ev=trace(packet()^(1<<bit)))
for bit in range(16):reject(ev=trace(arm=0xa21c^(1<<bit)))
for reason in (0,2,4,255):reject(ev=trace(packet(reason=reason)))
for kind in range(16):
    if kind!=0xc:reject(ev=trace(packet(kind=kind)))
for n in (0,1,0xffff):reject(ev=trace(packet(n=n)))
for n in (1,0xffff):
    assert M['decode'](trace(packet(n=n)),n,fixed_source_admitted=True)['result']==M['PASS'];positive+=1
for words in ([0xa41a],[0xa21c,0xa41a],[0xa21c]):
    expected='INCONCLUSIVE_ARMED_ONLY' if len(words)==1 and words[0]==0xa21c else 'INCONCLUSIVE_ERROR_EVENT'
    for admitted in (False,True):nonpass(ev=T['trace'](words),admitted=admitted,expected=expected)
for admitted in (False,True):
    nonpass(ev=T['trace'](interval=19_000_000),admitted=admitted,expected='SAFE_NEGATIVE_ZERO_ALIVE_DISABLED')
reject(ev=T['trace'](interval=16_777_215))
reject(ev=A['events']);reject(ev=B['events']) # WDT7 B and software entry are not WDT8 C.

for part in ('request','record 000','record 252','ack-issued','observer-quiesced','pre-ack-gem-stop'):
    reject(t=text.replace('[WQ8] '+part,'absent '+part))
    line=next(line for line in text.splitlines() if '[WQ8] '+part in line)
    reject(t=text+line+'\n')
for version in (3,4,5,6,7,9):reject(t=text.replace('[WQ8]',f'[WQ{version}]'))
for n in (96,97,109,110,112,139,176,177,182,183,184,252):
    line=f'[WQ8] record {n//4*4:03} '+' '.join(f'{x:08x}' for x in w[n//4*4:n//4*4+4])
    bad=w.copy();bad[n]^=1
    if n==109:bad[n]|=0x40000000
    if n==110:bad[n]=255
    other=f'[WQ8] record {n//4*4:03} '+' '.join(f'{x:08x}' for x in bad[n//4*4:n//4*4+4])
    reject(t=text.replace(line,other))
for ncr in (1,4,8,16,512):reject(t=text.replace('ncr=0x00000000',f'ncr=0x{ncr:08x}'))
for tail in ('[WQ8] failure=probe','[RTOS] unexpected','[WQ8] record 000'):
    reject(t=text+tail+'\n')
for index in range(9,len(events)):
    for selected in (events[:index]+events[index+1:],events[:index]+[events[index]]+events[index:]):
        bad=deepcopy(selected)
        for i,e in enumerate(bad):e['seq']=i
        reject(ev=bad)
for index,field,value in [(0,'seq',True),(0,'timestamp_us',-1),(10,'level',1),
    (10,'edge','rising'),(10,'timestamp_us',0),(43,'source','gpio23'),
    (44,'timestamp_us',events[43]['timestamp_us']+100_000),
    (45,'timestamp_us',events[44]['timestamp_us']+100_000),
    (len(events)-1,'timestamp_us',events[-2]['timestamp_us']+100_000)]:
    bad=deepcopy(events);bad[index][field]=value;reject(ev=bad)
for source in ('gpio25','gpio23'):
    for count in (1,2,33,66):
        bad=deepcopy(events)
        for i in range(count):bad.append(dict(seq=len(bad),source=source,level=(i+1)%2,
            edge='rising' if (i+1)%2 else 'falling',timestamp_us=bad[-1]['timestamp_us']+500_000))
        reject(ev=bad)
for count in (62,63,64,84):
    # Additional 1Hz prefix edges leave frame timing/contents unchanged.
    prefix=[dict(seq=i,source='gpio25',level=(i+count)%2,edge='rising' if (i+count)%2 else 'falling',
        timestamp_us=i*1_005_000) for i in range(count)]
    bad=prefix+[dict(e,timestamp_us=e['timestamp_us']+count*1_005_000) for e in events]
    for i,e in enumerate(bad):e['seq']=i
    if count<=63:accept(ev=bad) # 71/72 prefix edges, 171/172 total.
    else:reject(ev=bad)
# The prefix budget includes non-marker boot/control events as well.
for count in (63,64,84):
    prefix=[dict(seq=i,source='gpio23',level=i%2,edge='rising' if i%2 else 'falling',
        timestamp_us=i) for i in range(count)]
    bad=prefix+[dict(e,timestamp_us=e['timestamp_us']+count) for e in events]
    for i,e in enumerate(bad):e['seq']=i
    if count==63:accept(ev=bad)
    else:reject(ev=bad)
# Both the ARM and C frame retain the selected bit/gap/delimiter tolerance.
for first,bits in ((9,16),(43,32)):
    zero=next(i for i in range(bits) if events[first+2*i+1]['timestamp_us']-events[first+2*i]['timestamp_us']==50_000)
    for offset,widths in ((0,(125_000,175_000)),(2*zero,(25_000,75_000)),
                          (2*bits,(375_000,425_000))):
        fall=first+offset+1
        for width in (*widths,widths[0]-1,widths[1]+1):
            bad=deepcopy(events)
            shift=width-(bad[fall]['timestamp_us']-bad[fall-1]['timestamp_us'])
            for e in bad[fall:]:e['timestamp_us']+=shift
            if width in widths:accept(ev=bad)
            else:reject(ev=bad)
    for gap in (25_000,75_000,24_999,75_001):
        bad=deepcopy(events);rise=first+2
        shift=gap-(bad[rise]['timestamp_us']-bad[rise-1]['timestamp_us'])
        for e in bad[rise:]:e['timestamp_us']+=shift
        if gap in (25_000,75_000):accept(ev=bad)
        else:reject(ev=bad)
protocol,before,after=T['inputs'](events)
for args in [(protocol,before,after.replace('overflow=0','overflow=1')),
    (protocol,before,after.replace(f'count={len(events)}','count=1')),
    (protocol,before,after.replace(f'edges={len(events)}','edges=1')),
    (protocol,before.replace('count=0','count=1'),after),
    (protocol.replace('\nOK\n',''),before,after),(protocol+'[]',before,after)]:reject(args=args)
# Shared type-B default remains strict; only the explicit type-C selector accepts C.
try:M['E']['decode'](T['trace']([packet()],bits=32),nonce,1)
except AssertionError:negative+=1
else:raise AssertionError('type-C accepted by default type-B decoder')
# CLI defaults cannot report PASS; explicit admission is required by the cohort wrapper.
with TemporaryDirectory(prefix='kernel-restart-test-') as directory:
    paths=[]
    for name,data in zip(('uart','trace','before','after'),(text,protocol,before,after)):
        path=Path(directory)/name;path.write_text(data);paths.append(str(path))
    for flags,status in (([],1),(['--fixed-source-admitted'],0)):
        result=subprocess.run([sys.executable,'-B',str(Path(__file__).with_name('check-kernel-restart.py')),
            *flags,*paths],capture_output=True,text=True,check=False)
        assert result.returncode==status,(result.stdout,result.stderr)
        decoded=T['json'].loads(result.stdout)
        assert decoded['runtime_restart_proven']==bool(flags)
        assert (decoded['result']==M['PASS'])==bool(flags)
        if flags:positive+=1
        else:inconclusive+=1
for name in ('check-kernel-restart.py','check-kernel-restart-elf.py'):
    result=subprocess.run([sys.executable,'-B','-O',str(Path(__file__).with_name(name))],
        capture_output=True,text=True,check=False)
    assert result.returncode!=0 and 'assertions required' in result.stderr
    negative+=1
print(f'PASS: WDT8 synthetic {positive} positives / {inconclusive} explicit non-PASS / {negative} refusals; NOT HW; source gate is not external counter sampling')

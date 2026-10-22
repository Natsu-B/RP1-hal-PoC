#!/usr/bin/env python3
"""WDT9 synthetic/adversarial checks; optional unchanged AR raw replay."""
import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import sys
from tempfile import TemporaryDirectory

if not __debug__:raise SystemExit('assertions required')
# Execute the unchanged WDT4/5/6/7/8 regressions before extending their fixtures.
K=runpy.run_path(str(Path(__file__).with_name('test-kernel-restart.py')))
T=K['T']; B=K['B']; M=runpy.run_path(str(Path(__file__).with_name('check-warm-guard.py')))
nonce=K['nonce']
request=[int.from_bytes(b'WQ09','little'),9,1,1,0xffffff,256,nonce,0x57445432^9^1^1^0xffffff^256^nonce]
ack=[int.from_bytes(b'QA09','little'),9,1,2,0,0,nonce,0x57445432^9^1^2^nonce]
w=K['w'].copy();w[96]=int.from_bytes(b'WDT9','little');w[97]=9;w[176:184]=request
text=T['uart'](w).replace('[WQ4]','[WQ9]')
for old,new in ((T['REQUEST'],request),(T['ACK'],ack)):
    text=text.replace(' '.join(f'{x:08x}' for x in old),' '.join(f'{x:08x}' for x in new))


def numbered(events):
    # Synthetic fixture construction only. Never renumber the actual AR replay.
    return [dict(e,seq=i) for i,e in enumerate(events,1)]
def packet(n=nonce,gate=0x23,kind=0xe):return B['packet'](n=n,reason=gate,kind=kind)
def trace(word=None,interval=16_777_215,arm=0xa21c):
    return numbered(K['trace'](packet() if word is None else word,interval=interval,arm=arm))


events=trace();kernel=trace(packet(gate=1,kind=0xc),interval=22_277_215)
positive=negative=inconclusive=0
def run(t=text,ev=events,admitted=True):
    return M['validate'](t,*T['inputs'](ev),fixed_source_admitted=admitted)
def accept(ev=events,kernel=False):
    global positive
    result=run(ev=ev)
    expected=M['PASS'] if kernel else M['DIAGNOSTIC']
    assert result['result']==expected and result['classification']==('HW' if kernel else 'diagnostic')
    assert result['runtime_restart_proven']==kernel and result['hardware_reset_entry_proven']==kernel
    assert result['warm_guard_identity_proven']==(not kernel) and result['fixed_source_admitted']
    assert not result['post_ack_registers_read_by_host'] and result['R3']=='OPEN'
    assert not result['exact_progress_counters_externally_sampled']
    if not kernel:
        assert 'RESTART' not in result['result'] and not result['result'].endswith('_PASS')
        ext=result['external'];assert ext['gate_code'] in M['GATE_CODES']
        assert ext['ispr1_exact_0x00200000']==(ext['gate_code']==0x23)
    positive+=1
def nonpass(ev=events,admitted=False,expected='INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED'):
    global inconclusive
    result=run(ev=ev,admitted=admitted)
    assert result['result']==expected and not result['runtime_restart_proven']
    assert not result['hardware_reset_entry_proven'] and not result['warm_guard_identity_proven']
    if 'ispr1_exact_0x00200000' in result['external']:
        assert not result['external']['ispr1_exact_0x00200000']
    inconclusive+=1
def reject(t=text,ev=events,args=None,admitted=True):
    global negative
    try:M['validate'](t,*(T['inputs'](ev) if args is None else args),fixed_source_admitted=admitted)
    except (AssertionError,ValueError,KeyError,TypeError,IndexError):negative+=1
    else:raise AssertionError('invalid warm-guard capture accepted')


assert tuple(M['GATE_CODES'])==(*range(1,6),*range(0x10,0x13),*range(0x20,0x24),*range(0x30,0x35))
for gate in range(256):
    if gate in M['GATE_CODES']:accept(ev=trace(packet(gate=gate)))
    else:reject(ev=trace(packet(gate=gate)))
accept(ev=kernel,kernel=True)
accept(ev=trace(packet(gate=3,kind=0xc),interval=22_277_215),kernel=True)
for ev in (events,kernel):
    nonpass(ev=ev)
    assert M['validate'](text,*T['inputs'](ev))['result']=='INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED'
    for bad in (1,'true',None):reject(ev=ev,admitted=bad)
for kind,low,high in ((0xe,16_000_000,19_000_000),(0xc,21_000_000,26_000_000)):
    for dt in (low,high-1):accept(ev=trace(packet(kind=kind,gate=1),interval=dt),kernel=kind==0xc)
    for dt in (0,low-1,high):reject(ev=trace(packet(kind=kind,gate=1),interval=dt))
for ev in (events,kernel):
    for n in range(len(ev)):
        if n==43:nonpass(ev=ev[:n],admitted=True,expected='INCONCLUSIVE_ARMED_ONLY')
        else:reject(ev=ev[:n])
    for bit in range(32):
        word=packet() if ev is events else packet(kind=0xc,gate=1)
        reject(ev=trace(word^(1<<bit),interval=16_777_215 if ev is events else 22_277_215))
    for index in range(9,len(ev)):
        reject(ev=numbered(ev[:index]+ev[index+1:]))
        reject(ev=numbered(ev[:index]+[ev[index]]+ev[index:]))
    for source in ('gpio25','gpio23'):
        for count in (1,2,33,66):
            bad=deepcopy(ev)
            for i in range(count):bad.append(dict(seq=len(bad)+1,source=source,level=(i+1)%2,
                edge='rising' if (i+1)%2 else 'falling',timestamp_us=bad[-1]['timestamp_us']+500_000))
            reject(ev=bad)
for bit in range(16):reject(ev=trace(arm=0xa21c^(1<<bit)))
for kind in range(16):
    if kind not in (0xc,0xe):reject(ev=trace(packet(kind=kind)))
for gate in (0,2,4,255):reject(ev=trace(packet(kind=0xc,gate=gate),interval=22_277_215))
for n in (0,1,0xffff):reject(ev=trace(packet(n=n)))
for n in (1,0xffff):
    assert M['decode'](trace(packet(n=n)),n,fixed_source_admitted=True)['result']==M['DIAGNOSTIC'];positive+=1
for bad in (0,-1,0x10000,True,'1',None):
    try:M['decode'](events,bad,fixed_source_admitted=True)
    except (AssertionError,TypeError):negative+=1
    else:raise AssertionError('invalid nonce admitted')
for words in ([0xa41a],[0xa21c,0xa41a],[0xa21c]):
    expected='INCONCLUSIVE_ARMED_ONLY' if words==[0xa21c] else 'INCONCLUSIVE_ERROR_EVENT'
    for admitted in (False,True):nonpass(ev=numbered(T['trace'](words)),admitted=admitted,expected=expected)
for admitted in (False,True):
    nonpass(ev=numbered(T['trace'](interval=19_000_000)),admitted=admitted,expected='SAFE_NEGATIVE_ZERO_ALIVE_DISABLED')
reject(ev=numbered(T['trace'](interval=16_777_215)))
reject(ev=numbered(K['A']['events']));reject(ev=numbered(B['events']))
reject(ev=K['events']) # The old zero-based synthetic WDT8 contract is not live WDT9.
for index,field,value in ((0,'seq',0),(0,'seq',True),(1,'seq',1),(1,'seq',3),
    (0,'timestamp_us',-1),(0,'timestamp_us',False),(10,'level',1),(10,'level',False),
    (10,'edge','rising'),(10,'timestamp_us',0),(43,'source','gpio23'),
    (44,'timestamp_us',events[43]['timestamp_us']+100_000),
    (45,'timestamp_us',events[44]['timestamp_us']+100_000),
    (len(events)-1,'timestamp_us',events[-2]['timestamp_us']+100_000)):
    bad=deepcopy(events);bad[index][field]=value;reject(ev=bad)
for part in ('request','record 000','record 252','ack-issued','observer-quiesced','pre-ack-gem-stop'):
    reject(t=text.replace('[WQ9] '+part,'absent '+part))
    line=next(line for line in text.splitlines() if '[WQ9] '+part in line)
    reject(t=text+line+'\n')
for version in (3,4,5,6,7,8,10):reject(t=text.replace('[WQ9]',f'[WQ{version}]'))
reject(t=K['text']) # Actual old-version evidence cannot be relabeled WDT9.
for n in (96,97,109,110,112,139,176,177,182,183,184,252):
    line=f'[WQ9] record {n//4*4:03} '+' '.join(f'{x:08x}' for x in w[n//4*4:n//4*4+4])
    bad=w.copy();bad[n]^=1
    if n==109:bad[n]|=0x40000000
    if n==110:bad[n]=255
    other=f'[WQ9] record {n//4*4:03} '+' '.join(f'{x:08x}' for x in bad[n//4*4:n//4*4+4])
    reject(t=text.replace(line,other))
for ncr in (1,4,8,16,512):reject(t=text.replace('ncr=0x00000000',f'ncr=0x{ncr:08x}'))
for tail in ('[WQ9] failure=probe','[RTOS] unexpected','[WQ9] record 000'):reject(t=text+tail+'\n')
for source in ('gpio25','gpio23'):
    for count in (63,64,84):
        prefix=[dict(seq=i+1,source=source,level=i%2,edge='rising' if i%2 else 'falling',
            timestamp_us=i*(1_005_000 if source=='gpio25' else 1)) for i in range(count)]
        shift=count*1_005_000
        bad=numbered(prefix+[dict(e,timestamp_us=e['timestamp_us']+shift) for e in events])
        if source=='gpio25':
            # Keep the synthetic marker prefix alternating into the original low.
            for e in prefix:e['level']=1-e['level'];e['edge']='rising' if e['level'] else 'falling'
            bad=numbered(prefix+[dict(e,timestamp_us=e['timestamp_us']+shift) for e in events])
        if count==63:accept(ev=bad)
        else:reject(ev=bad)
for ev in (events,kernel):
    # Shared pulse tolerances apply to both ARM and the diagnostic/kernel frame.
    for first,bits in ((9,16),(43,32)):
        zero=next(i for i in range(bits) if ev[first+2*i+1]['timestamp_us']-ev[first+2*i]['timestamp_us']==50_000)
        for offset,widths in ((0,(125_000,175_000)),(2*zero,(25_000,75_000)),(2*bits,(375_000,425_000))):
            fall=first+offset+1
            for width in (*widths,widths[0]-1,widths[1]+1):
                bad=deepcopy(ev);shift=width-(bad[fall]['timestamp_us']-bad[fall-1]['timestamp_us'])
                for e in bad[fall:]:e['timestamp_us']+=shift
                if width in widths:accept(ev=bad,kernel=ev is kernel)
                else:reject(ev=bad)
        for gap in (25_000,75_000,24_999,75_001):
            bad=deepcopy(ev);rise=first+2;shift=gap-(bad[rise]['timestamp_us']-bad[rise-1]['timestamp_us'])
            for e in bad[rise:]:e['timestamp_us']+=shift
            if gap in (25_000,75_000):accept(ev=bad,kernel=ev is kernel)
            else:reject(ev=bad)
protocol,before,after=T['inputs'](events)
for args in ((protocol,before,after.replace('overflow=0','overflow=1')),
    (protocol,before,after.replace(f'count={len(events)}','count=1')),
    (protocol,before,after.replace(f'edges={len(events)}','edges=1')),
    (protocol,before.replace('count=0','count=1'),after),
    (protocol,before.replace('overflow=0','overflow=1'),after),
    (protocol.replace('\nOK\n',''),before,after),(protocol+'[]',before,after)):
    reject(args=args)
# E must be explicitly requested; the default B and explicit C codecs stay strict.
for kind in (0xb,0xc,0xe):
    single=numbered(T['trace']([packet(kind=kind,gate=1)],bits=32))
    assert M['E']['decode'](single,nonce,1,expected_type=kind)['raw_reason']==1;positive+=1
    if kind!=0xb:
        try:M['E']['decode'](single,nonce,1)
        except AssertionError:negative+=1
        else:raise AssertionError('non-B frame accepted by default decoder')
    if kind==0xe:
        try:M['E']['decode'](single,nonce,1,expected_type=0xc)
        except AssertionError:negative+=1
        else:raise AssertionError('diagnostic accepted as kernel frame')
with TemporaryDirectory(prefix='warm-guard-test-') as directory:
    paths=[str(Path(directory)/name) for name in ('uart','trace','before','after')]
    for ev,accepted in ((events,M['DIAGNOSTIC']),(kernel,M['PASS']),
                        (events[:43],'INCONCLUSIVE_ARMED_ONLY')):
        for path,data in zip(paths,(text,*T['inputs'](ev))):Path(path).write_text(data)
        for flags in ([],['--fixed-source-admitted']):
            result=subprocess.run([sys.executable,'-B',str(Path(__file__).with_name('check-warm-guard.py')),
                *flags,*paths],capture_output=True,text=True,check=False)
            expected=accepted if flags or ev==events[:43] else 'INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED'
            assert result.returncode==int(expected not in (M['DIAGNOSTIC'],M['PASS'])),result.stderr
            decoded=json.loads(result.stdout);assert decoded['result']==expected
            assert decoded['runtime_restart_proven']==(expected==M['PASS'])
            if result.returncode==0:positive+=1
            else:inconclusive+=1
for name in ('check-warm-guard.py','test-warm-guard.py'):
    result=subprocess.run([sys.executable,'-B','-O',str(Path(__file__).with_name(name))],
        capture_output=True,text=True,check=False)
    assert result.returncode!=0 and 'assertions required' in result.stderr;negative+=1
print(f'PASS: WDT9 synthetic {positive} positives / {inconclusive} explicit non-PASS / {negative} refusals; NOT HW')


def replay_ar(analysis):
    """Hash-check original files, keep seq1..105, and prove ONLY ARM was captured."""
    data=json.loads(analysis.read_text())
    files={Path(name):digest for name,digest in data['raw_sha256'].items()}
    for path,digest in files.items():assert hashlib.sha256(path.read_bytes()).hexdigest()==digest,path
    protocol_path=next(path for path in files if path.name=='trace.json.protocol')
    uart_path=next(path for path in files if path.name=='uart10.log')
    protocol=protocol_path.read_text();before=(protocol_path.parent/'trace-before.txt').read_text()
    after=(protocol_path.parent/'trace-status.txt').read_text();uart=uart_path.read_text()
    words=M['P']['W3']['validate_uart'](uart,version=8)
    gates=M['P']['gates'](uart,version=8)
    assert gates['before_reload_ncr']==gates['pre_ack_ncr']==0
    raw=M['P']['W3']['W']['R1']['decode_trace'](protocol,before,after)
    assert len(raw)==105 and [e['seq'] for e in raw]==list(range(1,106))
    assert words[182]==data['nonce']
    result=M['decode'](raw,words[182],fixed_source_admitted=False)
    assert result['result']=='INCONCLUSIVE_ARMED_ONLY'
    assert not result['runtime_restart_proven'] and not result['warm_guard_identity_proven']
    assert not result['hardware_reset_entry_proven']
    # Full WDT9 live validation still refuses old WDT8 UART identity, as required.
    try:M['validate'](uart,protocol,before,after,fixed_source_admitted=True)
    except AssertionError:pass
    else:raise AssertionError('AR capture mislabeled as AS WDT9')
    for path,digest in files.items():assert hashlib.sha256(path.read_bytes()).hexdigest()==digest,path
    print('PASS: AR original raw SHA256 replay: seq1..105, ARM-only, runtime_restart_proven=false; no raw normalization or WDT9 relabeling')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ar-analysis',type=Path,help='Original AR commissioning-analysis.json for read-only raw replay')
    args=parser.parse_args()
    if args.ar_analysis:replay_ar(args.ar_analysis)

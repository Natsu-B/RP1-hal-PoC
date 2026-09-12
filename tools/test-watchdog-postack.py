#!/usr/bin/env python3
"""Runnable synthetic WDT4 mutations. No synthetic result is hardware evidence."""
import json
from pathlib import Path
import runpy

M = runpy.run_path(str(Path(__file__).with_name('check-watchdog-postack.py')))
REQUEST = [int.from_bytes(b'WQ04','little'),4,1,1,0xffffff,256,0,0x57445432^4^1^1^0xffffff^256]
ACK = [int.from_bytes(b'QA04','little'),4,1,2,0,0,0,0x57445432^4^1^2]
w = [0]*256
w[:5] = [0x31305452,1,5,0,0]
w[66] = w[82] = 2; w[67] = 0x20002000; w[83] = 0x20003000
w[96:104] = [int.from_bytes(b'WDT4','little'),4,4,0,1,100,500,1]
w[104:116] = [0,3,50,0x40fffffe,0x40fffefc,0x00fffefc,256,369,2,2,0,0]
w[128:136] = [1,2,3,4,2,3,4,5]; w[140:145] = [1,2001,1,5001,450]
w[176:184] = REQUEST


def uart(words):
    lines = ['[TFTP] Rp1Gem release before RP1 reload complete ncr=0x00000000',
             '[RP1BOOT] image loaded', '[RP1BOOT] proc0 started',
             '[RP1PCIE:post-rp1-reload-reinit] attempt=0 result=success',
             '[WQ4] request '+' '.join(f'{x:08x}' for x in REQUEST)]
    lines += [f'[WQ4] record {n:03} '+' '.join(f'{x:08x}' for x in words[n:n+4]) for n in range(0,256,4)]
    lines += ['[WQ4] pre-ack-gem-stop ncr=0x00000000',
              '[WQ4] ack-issued '+' '.join(f'{x:08x}' for x in ACK),
              '[WQ4] observer-quiesced no-more-rp1-access=1']
    return '\n'.join(lines)+'\n'


def trace(words=(0xa21c,0xa31d), bits=16):
    events = []
    def edge(t, level):
        events.append(dict(seq=len(events),timestamp_us=t,source='gpio25',level=level,
                           edge='rising' if level else 'falling'))
    edge(0,0)
    # Test a normal heartbeat prefix as well as arbitrary low wait between packets.
    for i in range(8): edge((i+1)*1005000,1 if i%2 == 0 else 0)
    t = events[-1]['timestamp_us']+500000
    first = t
    for index, word in enumerate(words):
        if index: t = max(t+500000, first+index*16777215)
        for b in range(bits-1,-1,-1):
            edge(t,1); t += 150000 if word & (1<<b) else 50000
            edge(t,0); t += 50000
        edge(t,1); t += 400000; edge(t,0)
    return events


def inputs(events):
    return (json.dumps(events)+'\nOK\n', 'trace_count=0 overflow=0 gpio_edges=0 armed=1\nOK\n',
            f'trace_count={len(events)} overflow=0 gpio_edges={len(events)} armed=0\nOK\n')


good = uart(w); events = trace()
def validate(text=good, ev=events): return M['validate'](text,*inputs(ev))
assert validate()['result'].endswith('_PASS')
assert not validate()['restart_proven']
assert validate(ev=trace([0xa41a]))['result'] == 'INCONCLUSIVE_ERROR_EVENT'
assert validate(ev=trace([0xa21c,0xa41a]))['result'] == 'INCONCLUSIVE_ERROR_EVENT'
assert validate(ev=trace([0xa21c]))['result'] == 'INCONCLUSIVE_ARMED_ONLY'
negative = 0
def reject(text=good, ev=events, args=None):
    global negative
    try: M['validate'](text,*(inputs(ev) if args is None else args))
    except (AssertionError,ValueError,KeyError,TypeError): negative += 1
    else: raise AssertionError('accepted invalid mutation')


for n,v in [(0,0),(3,1),(66,0),(67,0x20002001),(83,w[67]),(96,0),(97,3),(98,3),
            (100,0),(103,0),(107,0xffffff),(109,0x40fffefc),(110,1001),(114,1),(145,1),(176,0),(252,1)]:
    bad = w.copy(); bad[n] = v; reject(uart(bad))
for phrase in ['[WQ4] request ', '[WQ4] record 000 ', '[WQ4] record 252 ', '[WQ4] ack-issued ',
               '[WQ4] pre-ack-gem-stop ', '[RP1BOOT] image loaded', '[RP1BOOT] proc0 started',
               '[RP1PCIE:post-rp1-reload-reinit]', '[TFTP] Rp1Gem release before RP1 reload complete']:
    reject(good.replace(phrase,'absent '))
    line = next(line for line in good.splitlines() if phrase in line)
    reject(good+line+'\n')
for phrase in ['[WQ4] failure=probe', '[RTOS] unexpected', '[WQ4] record 000 ', 'stop failed; RP1 reload refused']:
    reject(good+phrase+'\n')
for bit in [4,8,16,512,0xffffffff]:
    reject(good.replace('ncr=0x00000000',f'ncr=0x{bit:08x}'))
reject(good.replace('[WQ4] record 008 ', '[WQ4] record 004 '))
for order in [[0xa31d],[0xa31d,0xa21c],[0xa21c,0xa21c],[0xa21c,0xa31d,0xa41a],
              [0xa21c,0xa31d,0xa31d],[0xa41a,0xa21c]]: reject(ev=trace(order))
for bit in range(16): reject(ev=trace([0xa21c^(1<<bit),0xa31d]))
# XOR is weak: this two-bit mutation maps ARMED to ZERO. Order must refuse it.
reject(ev=trace([0xa21c^0x0101,0xa31d]))
for bits in [15,17,32]: reject(ev=trace([0xa50101ff] if bits == 32 else [0xa21c,0xa31d],bits))
for end in range(len(events)):
    if end == 43: # Exactly the first complete frame: an explicit INCONCLUSIVE, never PASS.
        assert validate(ev=events[:end])['result'] == 'INCONCLUSIVE_ARMED_ONLY'
    else: reject(ev=events[:end])
for i in range(9,len(events)):
    bad = [e.copy() for e in events[:i]+events[i+1:]]
    for seq,e in enumerate(bad): e['seq'] = seq
    reject(ev=bad)
    bad = [e.copy() for e in events[:i]+[events[i]]+events[i:]]
    for seq,e in enumerate(bad): e['seq'] = seq
    reject(ev=bad)
for index,field,value in [(10,'seq',999),(10,'timestamp_us',0),(10,'level',1),(10,'edge','rising'),
                          (10,'source','gpio23'),(10,'timestamp_us',events[9]['timestamp_us']+100000),
                          (42,'timestamp_us',events[41]['timestamp_us']+300000)]:
    bad = [e.copy() for e in events]; bad[index][field] = value; reject(ev=bad)
for dt in [15_999_999,20_000_000]:
    bad = [e.copy() for e in events]
    for e in bad[43:]: e['timestamp_us'] += dt-16777215
    reject(ev=bad)
for length in [1,2,12,33]:
    reject(ev=trace([0xa21c,0xa31d,0xa41a])[:len(events)+length])
protocol,before,after = inputs(events)
for args in [(protocol.replace('\nOK\n',''),before,after),
             (protocol+'[]',before,after),(protocol,before.replace('count=0','count=1'),after),
             (protocol,before,after.replace('overflow=0','overflow=1')),
             (protocol,before,after.replace(f'count={len(events)}','count=999')),
             (protocol,before,after.replace(f'edges={len(events)}','edges=999'))]: reject(args=args)
print(f'PASS: synthetic WDT4 1 selected positive / 3 explicit inconclusive / {negative} refusals; NOT HW')

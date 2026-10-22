#!/usr/bin/env python3
"""Synthetic WDT3 UART/typed-frame refusal checks; no hardware claim."""
import json
from pathlib import Path
import runpy

M=runpy.run_path(str(Path(__file__).with_name('check-watchdog-quiescence.py')))
w=[0]*256
w[:5]=[0x31305452,1,5,0,0]
w[66]=w[82]=2;w[67]=0x20002000;w[83]=0x20003000
w[96:104]=[int.from_bytes(b'WDT3','little'),3,4,0,1,100,500,1]
w[104:116]=[0,3,50,0x40fffffe,0x40fffefc,0x00fffefc,256,369,2,2,0,0]
w[128:136]=[1,2,3,4,2,3,4,5];w[140:145]=[1,2001,1,5001,450]
w[176:184]=M['REQUEST']
def uart(words):
    lines=['[WQ3] request '+' '.join(f'{x:08x}' for x in M['REQUEST'])]
    lines += [f'[WQ3] record {n:03} '+' '.join(f'{x:08x}' for x in words[n:n+4]) for n in range(0,256,4)]
    lines += ['[WQ3] ack-issued '+' '.join(f'{x:08x}' for x in M['ACK']),M['END']]
    return '\n'.join(lines)+'\n'
events=[]
def edge(t,level):events.append(dict(seq=len(events),timestamp_us=t,source='gpio25',level=level,edge='rising' if level else 'falling'))
edge(0,0);t=500000
for bit in range(31,-1,-1):
    edge(t,1);t+=150000 if M['FRAME']&(1<<bit) else 50000;edge(t,0);t+=50000
edge(t,1);t+=400000;edge(t,0)
for i in range(36):t+=1005000;edge(t,1 if i%2==0 else 0)
def inputs(ev):
    return (json.dumps(ev)+'\nOK\n','trace_count=0 overflow=0 gpio_edges=0 armed=1\nOK\n',
        f'trace_count={len(ev)} overflow=0 gpio_edges={len(ev)} armed=0\nOK\n')
good=uart(w);result=M['validate'](good,*inputs(events));assert result['event']['word']=='a50101ff' and not result['expiry_tested']
negative=0
def reject(text,ev=events):
    global negative
    try:M['validate'](text,*inputs(ev))
    except (AssertionError,ValueError,KeyError):negative+=1
    else:raise AssertionError('mutation accepted')
for n,v in [(0,0),(3,1),(66,0),(67,0x20002001),(83,w[67]),(96,0),(97,2),(98,3),
            (100,0),(103,0),(107,0xffffff),(109,0x40fffefc),(110,1001),(114,1),(145,1),(176,0),(252,1)]:
    bad=w.copy();bad[n]=v;reject(uart(bad))
reject(good.replace('[WQ3] record 008 ','[WQ3] record 004 '))
reject(good.replace('[WQ3] record 004 ','[WQ3] absent 004 '))
reject(good.replace('[WQ3] ack-issued ','[WQ3] absent '))
reject(good+M['END']+'\n')
reject(good+'[WQ3] record 000 00000000 00000000 00000000 00000000\n')
reject(good.replace(M['END'],'[WQ3] failure=bounded-wait'))
for index,field,value in [(2,'timestamp_us',600000),(3,'timestamp_us',800000),(2,'level',1),
                         (2,'seq',999),(66,'timestamp_us',t),(2,'source','gpio23')]:
    bad=[e.copy() for e in events];bad[index][field]=value;reject(good,bad)
reject(good,events[:60]);reject(good,events[:-1])
for bit in range(32):
    bad=[e.copy() for e in events]
    fall=2+2*bit
    shift=-100000 if M['FRAME']&(1<<(31-bit)) else 100000
    for e in bad[fall:]:e['timestamp_us']+=shift
    reject(good,bad) # valid timing, but each possible packet bit is wrong
bad=[e.copy() for e in events]+[dict(e,timestamp_us=e['timestamp_us']+events[-1]['timestamp_us']+1000000) for e in events]
for i,e in enumerate(bad):e['seq']=i
reject(good,bad)
bad=[e.copy() for e in events[:10]+events[11:]]
for i,e in enumerate(bad):e['seq']=i
reject(good,bad) # missing physical edge, even when protocol seq is contiguous
print(f'PASS: synthetic WDT3 positive and {negative} negative; NOT HW')

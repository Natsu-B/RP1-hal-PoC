#!/usr/bin/env python3
"""Synthetic receipt mutations over supplied R1 records; no hardware claim."""
from pathlib import Path
import runpy
import sys

M=runpy.run_path(str(Path(__file__).with_name('check-freertos-watchdog.py')))
if len(sys.argv)==2:
    original=Path(sys.argv[1]).read_text()
else:
    assert len(sys.argv)==1
    # Self-contained arithmetic fixture, not a captured or simulated device.
    baseline=[]
    for i in range(31):
        w=[0]*256
        for k,v in {0:0x31305452,1:1,2:5,5:50000000,6:50000000,7:50000000,
            8:1000*(i+1),9:2000*(i+1),11:1000000*(i+1),12:999,13:1001,
            14:i+1,15:2*(i+1),17:i+1,18:256,19:0x13579bdf,22:49999,
            29:2,30:0x20001000,31:0x2000f000,39:1,49:i+1,50:i+1,
            64:i+1,66:2,67:0x20002000,80:i+1,82:2,83:0x20003000}.items():w[k]=v
        w[32:39]=[200]*7
        for offset in range(0,256,4):
            baseline.append(f'[RTOS] {i} {offset:03} '+' '.join(f'{v:08x}' for v in w[offset:offset+4]))
    original='\n'.join(baseline+[M['R1']['READ_ONLY_FOOTER']])+'\n'
rows=M['R1']['decode'](original)
for w in rows:
    w[96:184]=[0]*88
    w[96:104]=[M['MAGIC'],2,4,0,1,100,500,1]
    w[104:116]=[0,3,50,0x40ffffff,0x40fffefb,0x00fffefa,256,1000,2,2,0,0]
    w[128:136]=[1,2,3,4,2,3,4,5]
    w[140:145]=[1,2001,1,5001,270]
    w[176:184]=M['REQUEST']

def render(data):
    lines=['[WDT2] request '+' '.join(f'{v:08x}' for v in M['REQUEST'])]
    for n,w in enumerate(data):
        for offset in range(0,256,4):
            lines.append(f'[RTOS] {n} {offset:03} '+' '.join(f'{v:08x}' for v in w[offset:offset+4]))
    lines.append(M['R1']['WATCHDOG_FOOTER'])
    return '\n'.join(lines)+'\n'

good=render(rows);result=M['validate'](good)
assert not result['observer_read_only'] and not result['watchdog']['restart_proven']
negative=0
def reject(text):
    global negative
    try:M['validate'](text)
    except (AssertionError,ValueError):negative+=1
    else:raise AssertionError('accepted mutation')

for offset,value in [(96,0),(97,1),(98,3),(99,1),(100,0),(101,0),(102,101),(103,0),
    (104,1),(105,1),(106,0),(107,0xffffff),(108,0x40ffffff),(108,0xc0fffefb),
    (109,0x40fffefa),(110,0),(110,1001),(111,0),(111,100001),(112,0),(113,0),
    (114,1),(115,1),(116,1),(128,0),(132,1),(136,1),(141,1),(143,1),(144,0),(144,512),(145,1),
    *[(i,0xdeadbeef) for i in range(176,184)]]:
    bad=[w.copy() for w in rows];bad[-1][offset]=value;reject(render(bad))
reject(good.replace(M['R1']['WATCHDOG_FOOTER'],M['R1']['READ_ONLY_FOOTER']))
reject(good+M['R1']['READ_ONLY_FOOTER']+'\n')
reject(good.replace('[WDT2] request ','[WDT2] absent ',1))
reject(good+good.splitlines()[0]+'\n')
# Byte-identical shared R1 field/trace logic must still accept the original fixture.
old=M['R1']['validate'](original);assert old['observer_read_only']
print(f'PASS: synthetic positive, {negative} negative, original read-only regression; NOT HW')

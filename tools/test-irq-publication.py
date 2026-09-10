#!/usr/bin/env python3
"""STATIC: test actual adapter store order at each task-preemption boundary."""
from pathlib import Path
import re

root=Path(__file__).resolve().parents[1]/'crates/rp1-freertos/src'
def hazards(stores):
    state={'GENERATION':0,'WAITER':0,'ACTIVE':0,'CANCEL':0}
    bad=[]
    for name,value in stores:
        state[name]=value
        if state['GENERATION'] and not (state['WAITER'] and state['ACTIVE']): bad.append(name)
    return bad

assert hazards([('GENERATION',1),('WAITER',8),('ACTIVE',1)])
for name in ('i2c1.rs','spi0.rs'):
    text=(root/name).read_text()
    stores=re.findall(r'addr_of_mut!\((GENERATION|WAITER|ACTIVE|CANCEL)\)\.write_volatile\(([^;]+)\);',text)
    values=[(key,0 if value in ('0','ptr::null_mut()') else 1) for key,value in stores]
    assert len([x for x in values if x[0]=='GENERATION'])==2
    assert not hazards(values), (name,values)
print('STATIC: 2 real adapter publication/withdrawal store orders PASS; old order rejected')

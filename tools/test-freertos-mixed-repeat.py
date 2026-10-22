#!/usr/bin/env python3
"""Synthetic S mutation checks derived from retained L numeric evidence, not HW."""
from pathlib import Path
import runpy
import sys

source=Path(__file__).resolve().parent
N=runpy.run_path(str(source/'check-freertos-mixed-repeat.py'))
L=runpy.run_path(str(source/'check-freertos-mixed.py'))
assert len(sys.argv)==2,'retained selected L observer log'
old=L['decode'](Path(sys.argv[1]).read_text())[-1]
rows=[]
for n in range(36):
    w=old.copy();w[120:152]=old[128:160];w[152:184]=old[160:192];w[184:]=[0]*72
    for i in (9,14,15,49,64,80):w[i]=10000+n*100
    w[8]=1000+n*60000;w[11]=1_000_000+n*60_000_000
    for b,m,count in [(96,b'SPM2',384),(120,b'ICM2',18000),(152,b'UAM2',384)]:
        w[b]=int.from_bytes(m,'little');w[b+1]=4;w[b+2]=count;w[b+3]=n
        w[b+4]=count*4;w[b+8]=10_000_000;w[b+9]=1_925_000_000;w[b+10]=count
    w[112:120]=[384,1_924_250_000,1_924_252_000,2,384,384,10000,192]
    w[170:176]=[1_924_200_000,1_925_000_000,384,2,1,10000]
    w[177:182]=[int.from_bytes(b'HOST2RP1 IRQ 0384\r\n\xc3'[i:i+4],'big') for i in range(0,20,4)]
    rows.append(w)

def text():
    return '\n'.join(f'[RTOS] {n} {off:03d} '+' '.join(f'{x:08x}' for x in w[off:off+4])
        for n,w in enumerate(rows) for off in range(0,256,4))+'\n[RTOS] observer-complete read-only=1\n'

good=text();assert N['validate'](good)['spi']['completions']==384
def reject(value):
    try:N['validate'](value)
    except (AssertionError,ValueError):return
    raise AssertionError('mutation admitted')
for word,value in [(3,1),(184,1),(70,1),(98,383),(116,383),(119,191),(122,0),(132,0),
                   (144,0),(145,0),(151,0),(154,383),(169,1),(174,100),(175,5000),(181,0),(182,1),(45,0)]:
    before=[w[word] for w in rows]
    for w in rows:w[word]=value
    reject(text())
    for w,v in zip(rows,before):w[word]=v
rows[-1][173]+=1;reject(text());rows[-1][173]-=1
rows[10][98]=385;reject(text());rows[10][98]=384
rows[10][154]=383;reject(text());rows[10][154]=384
reject(good+good.splitlines()[0]+'\n')
reject(good.replace('[RTOS] 35 000','[RTOS] 36 000'))
reject(good.replace('[RTOS] observer-complete read-only=1',''))
print('PASS synthetic S numeric1positive/24refusals; no HW')

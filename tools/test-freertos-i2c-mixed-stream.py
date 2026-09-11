#!/usr/bin/env python3
"""Synthetic ICMS numeric boundary tests, including finite IRP2 regression."""
import copy
from pathlib import Path
import runpy

if not __debug__:raise SystemExit('assertions required')
root=Path(__file__).resolve().parent
pair=runpy.run_path(str(root/'test-freertos-i2c-mixed-pair.py'))
V=runpy.run_path(str(root/'check-freertos-i2c-mixed-stream.py'))
rows=[]
for n in range(36):
    w=pair['records']()[-1]
    for i in (9,14,15,49,64,80):w[i]=10000+n*100
    w[8]=1000+n*60000;w[11]=1000000+n*60000000
    for b in (96,120,152):
        w[b+1:b+5]=[4,384,n,384*4];w[b+10]=384
        w[b+8:b+10]=[10000000,1925000000]
    w[120]=int.from_bytes(b'ICMS','little')
    w[112:120]=[384,1924250000,1924252000,2,384,384,10000,192]
    w[129]=1925006000;w[139]=6336;w[140]=384;w[151]=0x2d000180
    data=bytes([0x4e,0x4f])+bytes((0xb4+i*0x1d+383*7)&255 for i in range(2,31))+b'\xc3'
    w[142:150]=[int.from_bytes(data[i:i+4],'big') for i in range(0,32,4)]
    w[170:176]=[1924200000,1925000000,384,2,1,10000]
    data=b'HOST2RP1 IRQ 0384\r\n\xc3'
    w[177:182]=[int.from_bytes(data[i:i+4],'big') for i in range(0,20,4)]
    rows.append(w)
encode=pair['encode']
good=encode(rows)
assert V['validate'](good)['i2c']['received']==6336
refused=0
def reject(text):
    global refused
    try:V['validate'](text)
    except (AssertionError,ValueError):refused+=1;return
    raise AssertionError('corruption admitted')
for index,value in [(3,1),(18,4096),(45,0),(60,1),(61,10000),(62,10),(63,0),
                    (70,1),(98,383),(116,383),(119,191),(120,0),(121,3),
                    (122,383),(124,383),(125,50000),(130,385),(131,1),(132,0x40),
                    (133,0x800001),(134,0),(136,0),(137,0),(138,10000),(139,6335),
                    (140,383),(141,0),(142,0),(149,0),(150,25),(151,0x2d000002),
                    (154,383),(164,24),(169,1),(174,100),(177,0),(184,1),(192,1),
                    (129,1924999000)]:
    bad=copy.deepcopy(rows)
    for w in bad:w[index]=value
    reject(encode(bad))
for index in (98,100,122,124,130,154,156):
    bad=copy.deepcopy(rows);bad[10][index]-=1;reject(encode(bad))
bad=copy.deepcopy(rows);bad[-1][139]+=1;reject(encode(bad))
for text in (encode(rows[:-1]),good+good,good.replace('observer-complete','unfinished')):reject(text)
try:pair['V']['validate'](good)
except AssertionError:refused+=1
else:raise AssertionError('finite-pair validator admitted ICMS')
print(f'STATIC SYNTHETIC ICMS positive1/refusal{refused} PASS; no hardware')

#!/usr/bin/env python3
"""Synthetic numeric-validator checks, never real hardware evidence."""
import copy
from pathlib import Path
import runpy

if not __debug__:raise SystemExit('assertions required')
root=Path(__file__).resolve().parent
old=runpy.run_path(str(root/'test-freertos-mixed.py'))
V=runpy.run_path(str(root/'check-freertos-i2c-mixed-pair.py'))

def records():
    rows=old['records']()
    for n,w in enumerate(rows):
        w[96:]=[0]*160;w[60:64]=[0,2000,0,int.from_bytes(b'MD01','little')]
        for base,magic,irq,timing in [(96,b'SPM2',8,1600),(120,b'ICMP',33,15000),(152,b'UAM2',10,2000000)]:
            w[base]=int.from_bytes(magic,'little');w[base+1:base+4]=[4,2,n+1]
            w[base+4:base+8]=[irq,timing,100,80];w[base+10]=2
            w[base+8:base+10]=[4000000,11000000]
            w[base+14:base+16]=[0x5aa5a55a,0xa55a5aa5]
        w[108:110]=[0x69963c01,0x69963c02]
        w[112:120]=[2,9100000,9101600,2,2,2,10000,1]
        w[136:142]=[5000,6,1000,33,2,0x314ec3c3]
        data=bytes((0x31+0x83+i*0x1d)&255 for i in range(31))+b'\xc3'
        w[142:150]=[int.from_bytes(data[i:i+4],'big') for i in range(0,32,4)]
        w[150:152]=[24,0x2d000002]
        w[164:166]=[41,19];w[168]=10
        w[170:177]=[9000000,11000000,2,0,0,10000,9000]
        data=b'HOST2RP1 IRQ 0002\r\n\xc3'
        w[177:182]=[int.from_bytes(data[i:i+4],'big') for i in range(0,20,4)]
        w[183]=0x101
    return rows

rows=records();encode=old['encode']
assert V['validate'](encode(rows))['external_peer_and_gpio_required']
refused=0
for index,value in [(2,0xffffffff),(39,0),(60,1),(61,10000),(62,10),(63,0),
                    (120,0),(121,3),(122,1),(130,3),(131,1),(132,0x40),
                    (133,0x800001),(134,0),(136,0),(137,0),(138,10000),(139,32),
                    (140,0),(141,0),(142,0),(149,0),(150,25),(151,0x2e000002),
                    (184,1),(192,1),(108,0),(116,1),(164,24),(177,0)]:
    altered=copy.deepcopy(rows)
    for w in altered:w[index]=value
    try:V['validate'](encode(altered))
    except AssertionError:refused+=1
    else:raise AssertionError(('accepted corruption',index))
for text in [encode(rows[:-1]),encode(rows)+encode(rows),encode(rows).replace('observer-complete','unfinished')]:
    try:V['validate'](text)
    except AssertionError:refused+=1
    else:raise AssertionError('accepted incomplete/duplicate observer')
print(f'STATIC SYNTHETIC finite pair positive1/refusal{refused} PASS; no hardware')

#!/usr/bin/env python3
"""Synthetic validator admission/refusal only; never hardware evidence."""
import copy
from pathlib import Path
import runpy

v = runpy.run_path(str(Path(__file__).with_name('check-freertos-mixed.py')))

def records():
    out=[]
    for n in range(31):
        w=[0]*256
        fixed={0:0x31305452,1:1,2:5,5:200_000_000,18:912,19:0x13579bdf,
            22:199999,28:0,29:2,30:0x2000b000,31:0x2000f000,39:1,40:0x3158494d,
            41:5,42:3,43:2560,44:256,45:236,46:0x2000b800,47:0x2000c000,51:0x2000c800,
            52:0x840,53:1,54:1,55:16,66:2,67:0x2000b100,82:2,83:0x2000b300}
        for i,val in fixed.items():w[i]=val
        w[32:39]=[180,110,110,200,220,400,170]
        for i in [8,9,14,15,49,64,80]:w[i]=100000+n*1280
        w[11]=w[8]*1000
        for base,magic,count in [(96,b'SPM1',2),(128,b'ICM1',256),(160,b'UAM1',2)]:
            w[base]=int.from_bytes(magic,'little');w[base+1:base+4]=[4,count,n+1]
            w[base+4:base+8]=[count,5000,100,80]
            w[base+10]=count;w[base+14:base+16]=[0x5aa5a55a,0xa55a5aa5]
        w[108:110]=[0x69963c01,0x69963c02]
        w[112:120]=[7_000_000,7_001_600,1,0x69963c01,4,1529,45,54]
        w[120:128]=[11_000_000,11_001_600,2,0x69963c02,4,1529,45,54]
        w[140:142]=[0x40,0x00800001];w[144:147]=[5000,5,1000]
        w[149]=256;w[151]=40;w[152:157]=[0xc3c3c3c3,0xc0,0xc0,0xc0,1]
        w[172:174]=[41,19];w[176]=10
        w[178:184]=[6_000_000,9_000_000,1,10_000_000,12_000_000,2]
        w[184]=9000
        data=b'HOST2RP1 IRQ 0002\r\n\xc3'
        w[185:190]=[int.from_bytes(data[i:i+4],'big') for i in range(0,20,4)]
        w[191]=0x101;out.append(w)
    return out

def encode(rows):
    return '\n'.join(f'[RTOS] {n} {off:03d} '+' '.join(f'{x:08x}' for x in w[off:off+4])
        for n,w in enumerate(rows) for off in range(0,256,4))+'\n[RTOS] observer-complete read-only=1\n'

def rejects(text):
    try:v['validate'](text)
    except AssertionError:return
    raise AssertionError('unsafe synthetic input was admitted')

rows=records();text=encode(rows)
assert v['validate'](text)['external_peer_and_gpio_required'] is True
bad_words={0:0,2:0xffffffff,3:1,18:4096,19:0,22:1,29:0,32:0,39:0,
    40:0,43:2816,44:512,45:0,46:0x2000b000,50:1,52:0,55:0,70:1,83:0x2000b100,
    97:3,98:1,99:0,106:1,107:1,108:0,110:0,114:9,116:0,117:50_000,
    129:3,130:255,140:0,141:1,144:20_000,145:0,146:10_000,147:1,149:0,151:0,153:0,
    156:0,161:3,162:1,171:1,172:24,173:18,176:0,177:1,178:7_001_000,
    180:2,182:16_000_000,184:0,185:0,190:1,191:0,192:0x31544652}
for i,value in bad_words.items():
    broken=copy.deepcopy(rows)
    for w in broken[-3:]:w[i]=value
    rejects(encode(broken))
for i in [8,9,14,15,49,64,80]:
    broken=copy.deepcopy(rows);broken[15][i]=broken[14][i];rejects(encode(broken))
rejects(text.replace('[RTOS] observer-complete read-only=1',''))
rejects(text+text.splitlines()[0]+'\n')
rejects('\n'.join(text.splitlines()[1:]))
assert v['elapsed'](0xfffffff0,0x10)==32
print(f'STATIC SYNTHETIC positive1/refusal{len(bad_words)+10} PASS; no hardware')

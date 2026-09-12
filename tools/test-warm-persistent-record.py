#!/usr/bin/env python3
"""Synthetic BC01 record refusals only; no hardware or serial use."""
from pathlib import Path
import runpy
globals().update({k:v for k,v in runpy.run_path(str(Path(__file__).with_name('check-warm-persistent-record.py'))).items() if not k.startswith('__')})
def self_test():
    w = [0]*256
    w[:5] = [0x31305452,1,5,0,0]; w[96:105] = [0x384e524b,23,1,16,0x1234,6000,10000,100,100]
    w[124:126] = [int.from_bytes(b'BC01','little'),8]; w[19]=0x13579bdf; w[39]=1; w[18]=1024
    for i in PROGRESS: w[i]=10000
    for i,cap in zip(HWM,STACK_WORDS):w[i]=cap-32
    for start,psp in [(28,0x20008000),(65,0x20009000),(81,0x2000a000)]:w[start:start+4]=[0,2,psp,0x2000f000]
    w[175:178]=[0,2,0x2000b000];w[178:180]=[13000,29000]
    w[150:156]=[48,8,8,9000,24,9000]
    w[156:160]=[8,8,8,1];w[161:166]=[1,10000,48,7000,9000]
    for k in range(3):
        expected=payload(k,8)
        r=[8,0xa5000000|k<<16|len(expected)<<8,digest(expected),1,IPSRS[k]|(256 if k==1 else 0),10000,100 if k==0 else 9000,100 if k==0 else 4]
        for i,v in zip(receipt_indices(k),r):w[i]=v
    w[169:175]=[8,8,8,*IPSRS]
    z=w.copy()
    for i in PROGRESS:z[i]+=20
    def encode(pair):
        rows=['armed version=1 nonce=23 counter_hz=1000 ack_counter=100 delays_s=120,122 addr=0x2000f800 bytes=1024 samples=2 bracket_words=12 post-ack-rp1-read=1 post-ack-rp1-write=0 no-reinit=1']
        for n,words in enumerate(pair):
            t=120100+n*2000;ident=' '.join(f'{words[i]:08x}' for i in IDENTITY)
            rows += [f'sample={n} BEGIN deadline_counter={t} begin_counter={t}',f'sample={n} identity-before {ident}']
            rows += [f'sample={n} words {r:03} '+' '.join(f'{v:08x}' for v in words[r:r+4]) for r in range(0,256,4)]
            rows += [f'sample={n} identity-after {ident}',f'sample={n} END read_begin_counter={t+1} read_end_counter={t+2}']
        rows += ['observer-complete samples=2 post-ack-rp1-read=1 post-ack-rp1-write=0 no-reinit=1']
        return '\n'.join('[WQLATE] '+r for r in rows)
    text=encode([w,z]);require(validate(text,23,16,0x1234)['hardware_acceptance'] is False,'not HW alone')
    for values in [[10020]*4,[0xfffffff0]*4]:
        a,b=w.copy(),z.copy();a[101:105]=b[101:105]=values
        require(validate(encode([a,b]),23,16,0x1234)['hardware_acceptance'] is False,'ordered/wrapped publication counters')
    negatives=[text.replace('version=1','version=2'),text+'\n'+text,text.rsplit('\n',1)[0],
        text.replace('deadline_counter=122100','deadline_counter=122099'),text+'\nobserver-quiesced no-more-rp1-access=1',
        text.replace('sample=0 words 004','sample=0 words 000'),text.replace('read_end_counter=120102','read_end_counter=119999')]
    for i,value in [(0,0),(97,24),(98,2),(100,0),(125,1),(70,1),(192,0x31544652),(193,3),(198,0x100),(160,31),
        (175,35),(177,0x2000e000),(169,9),(172,41),(128,0),(183,1),(178,1),(101,0),(18,4096)]:
        bad=z.copy();bad[i]=value;negatives.append(encode([w,bad]))
    for i in PROGRESS:
        bad=z.copy();bad[i]=w[i];negatives.append(encode([w,bad]))
    bad=z.copy();bad[160]+=1;negatives.append(encode([w,bad]))
    for i in [105,106,107,166,167,168,184,185,191,193,198,209,255,101,102,103,104]:
        a,b=w.copy(),z.copy();a[i]=b[i]=0x7fffffff if i in [101,102,103,104] else 1
        negatives.append(encode([a,b]))
    negatives.append(text.replace('ack_counter=100',f'ack_counter={2**64-1}'))
    for i,v in [(150,49),(151,7),(152,3),(154,23),(159,0),(161,0),(163,46),(162,5000),(164,9000),(165,7000),(126,7),(134,7),(142,7),(156,7)]:
        bad=z.copy();bad[i]=v;negatives.append(encode([w,bad]))
    for bad in negatives:
        try:validate(bad,23,16,0x1234)
        except ValueError:continue
        raise AssertionError('negative accepted')
    require(delta(0xfffffff0,16)==32,'wrap')
    print(f'PASS synthetic BC terminal8 two-sample parser, {len(negatives)} refusals and wrap; no hardware')


self_test()

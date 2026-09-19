#!/usr/bin/env python3
"""RFT1 local-task frame observation, not same-boot/recovery/HW admission.

PC comes from the reviewed ELF, never from a log's claimed firmware identity.
All31 fixed observer copies are required. No filtered subset is admitted.
"""
import importlib.util
import json
from pathlib import Path
import re
import sys

assert __debug__

def decode(text):
    rows={}
    for m in re.finditer(r'\[RTOS\] (\d+) (\d{3}) ((?:[0-9a-f]{8} ?){4})',text):
        number,offset=map(int,m.group(1,2));sample=rows.setdefault(number,{})
        assert offset not in sample,'duplicate row'
        sample[offset]=[int(w,16) for w in m[3].split()]
    assert text.count('[RTOS] observer-complete read-only=1')==1,'observer completion'
    assert sorted(rows)==list(range(31)),'fixed31 samples'
    records=[]
    for number in range(31):
        assert sorted(rows[number])==list(range(0,256,4)),'incomplete record'
        records.append([w for offset in sorted(rows[number]) for w in rows[number][offset]])
    return records

def validate(text,pc):
    assert type(pc) is int and 0x20000000<=pc<0x2000e000 and pc%2==0
    healthy=[];post=[]
    for w in decode(text):
        assert w[:2]==[0x31305452,1] and w[3:5]==[0,0]
        assert w[19:21]==[0x13579bdf,0] and w[70]==w[86]==0
        assert w[28]==0 and w[29]&3==2 and w[30]%8==0
        assert 0x10003800<=w[30]<0x10004000,'monitor was not local'
        assert w[31]%8==0 and 0x2000e000<=w[31]<=0x2000f000
        if w[192]==0x31544652:
            assert w[14]==5 and w[40]==1 and w[41]>=5000 and w[42]>0
            assert w[193]==6 and w[197]==0xfffffffd,'UsageFault from task PSP'
            assert w[198:200]==[0x10000,0],'only UNDEFINSTR, no escalation'
            assert w[194]%8==0 and 0x2000e000<=w[194]<=0x2000f000
            assert w[195]%8==0 and 0x10003800<=w[195]<=0x10003fe0,'full32-byte local frame'
            assert w[202:207]==[0x101,0x202,0x303,0x404,0x1212]
            assert w[207]&1==1 and w[208]==pc and w[209]&0x010001ff==0x01000000
            if post:assert w==post[-1],'halt state kept changing'
            post.append(w)
        else:
            assert not post and w[192]==0 and w[2] in (4,5),'lost/unknown fault publication'
            if w[2]==5:
                if healthy:
                    assert all(0<(w[i]-healthy[-1][i])&0xffffffff<0x80000000 for i in [8,9,14,64,80])
                healthy.append(w)
    assert len(healthy)>=2 and len(post)>=20,'healthy prefix and stable halt interval'
    return dict(classification='OBSERVATION_ONLY',result='LOCAL_R1_UDF_FRAME_OBSERVED',
        hardware_acceptance=False,healthy_samples=len(healthy),halt_samples=len(post),
        monitor_psp=post[-1][30],fault_frame_psp=post[-1][195],fault_words=post[-1][192:210],
        recovery='SEPARATE_BOOT_REQUIRED',
        boundary='Needs same-run ELF/capture association, external GPIO and recovery joins. Not BE health/warm or general fault recovery.')

def self_test(pc):
    records=[]
    for i in range(31):
        w=[0]*256;w[:3]=[0x31305452,1,5];w[19]=0x13579bdf
        w[28:32]=[0,2,0x10003f20,0x2000f000]
        for k in [8,9,14,64,80]:w[k]=i+1
        if i>=5:
            for k in [8,9,14,64,80]:w[k]=5001
            w[14]=5;w[40:43]=[1,5000,5001000]
            w[192:210]=[0x31544652,6,0x2000f000,0x10003e80,0,0xfffffffd,
                0x10000,0,0,0,0x101,0x202,0x303,0x404,0x1212,0x20001235,pc,0x01000000]
        records.append(w)
    def encode(words):
        return ''.join(f'[RTOS] {n} {i:03} '+ ' '.join(f'{v:08x}' for v in w[i:i+4])+'\n'
            for n,w in enumerate(words) for i in range(0,256,4))+'[RTOS] observer-complete read-only=1\n'
    good=encode(records);assert validate(good,pc)['halt_samples']==26
    bad=[good+good,good.replace('[RTOS] 1 004 ','[OTHER] 1 004 ',1)]
    for i,v in [(30,0x20009000),(30,0x10004000),(193,3),(197,0xfffffff9),
                (198,0),(199,1),(195,0x20009000),(195,0x100037f8),(195,0x10003fe8),
                (195,0x10003e81),(194,0x10003f00),(203,0),(207,0),(208,pc+2),
                (209,0),(70,1),(14,6),(8,7000),(192,0)]:
        changed=[w.copy() for w in records];changed[-1][i]=v;bad.append(encode(changed))
    for text in bad:
        try:validate(text,pc)
        except AssertionError:pass
        else:raise AssertionError('bad local fault record accepted')
    return dict(classification='BUILD',positive=1,refusals=len(bad),hardware=False)

if __name__=='__main__':
    assert len(sys.argv)==3,'--self-test ELF | UART ELF'
    spec=importlib.util.spec_from_file_location('linked',Path(__file__).with_name('check-local-r1-elf.py'))
    linked=importlib.util.module_from_spec(spec);spec.loader.exec_module(linked)
    review,_=linked.check(sys.argv[2],'freertos-r1-local-stack-fault')
    pc=review['probe_pc']
    result=self_test(pc) if sys.argv[1]=='--self-test' else validate(Path(sys.argv[1]).read_text(),pc)
    result['elf_sha256']=review['elf_sha256']
    print(json.dumps(result,indent=2))

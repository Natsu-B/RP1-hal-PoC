#!/usr/bin/env python3
"""SPM2/ICM2/UAM2 fixed-image sustained numeric proof; external peer required."""
import json
from pathlib import Path
import runpy
import sys

if not __debug__:
    raise SystemExit('Refuse disabled evidence assertions')
BASE=runpy.run_path(str(Path(__file__).with_name('check-freertos-mixed.py')))
elapsed=BASE['elapsed']

def decode(text):
    # Same strict row framing as selected MIX1, extended to exactly36 samples.
    import re
    records={}
    for m in re.finditer(r'\[RTOS\] (\d+) (\d{3}) ((?:[0-9a-f]{8} ?){4})',text):
        n,off=map(int,m.group(1,2)); record=records.setdefault(n,{})
        assert off not in record,'duplicate row'
        record[off]=[int(v,16) for v in m[3].split()]
    assert text.count('[RTOS] observer-complete read-only=1')==1,'observer incomplete/duplicate'
    assert sorted(records)==list(range(36)),'missing/extra samples'
    for record in records.values(): assert sorted(record)==list(range(0,256,4)),'incomplete sample'
    return [[v for off in sorted(records[n]) for v in records[n][off]] for n in range(36)]

def validate(text):
    rows=decode(text)
    for n,w in enumerate(rows):
        assert w[:2]==[0x31305452,1] and w[2]!=0xffffffff and w[3]==0,'RT01 fault/schema'
        assert not any(w[184:]),'reserved fault region changed'
        if n<3: continue
        assert w[2]==5 and w[19:21]==[0x13579bdf,0],'startup/monitor'
        assert w[28]==0 and w[29]&3==2 and w[30]&7==0,'monitor context'
        assert 0x2000e000<=w[31]<=0x2000f000
        assert w[21]&0x700==0 and w[22]+1==w[5]//1000,'tick/priority'
        assert w[39]==1 and w[17]==w[50]==0,'queue admission/not mutex cohort'
        assert w[40:45]==[0x3158494d,5,3,2560,256],'MIX1 memory/tasks'
        free=w[32:39]+[w[45]];limits=[256,128,128,256,256,512,512,512]
        assert all(32<v<=limit for v,limit in zip(free,limits)),'task stack guard'
        assert 0<w[18]<=3968,'MSP guard'
        assert w[70]==w[86]==0,'R4-R11 context'
        assert w[65]==w[81]==0 and w[66]&3==w[82]&3==2,'spin context'
        assert w[67]!=w[83] and w[67]&7==w[83]&7==0,'spin PSP'
        assert all(0x20000000<=w[i]<0x2000e000 for i in (30,67,83))
        if n>3:
            for index in (8,9,14,15,49,64,80): elapsed(rows[n-1][index],w[index])
        for base,magic in [(96,b'SPM2'),(120,b'ICM2'),(152,b'UAM2')]:
            assert w[base]==int.from_bytes(magic,'little') and w[base+11]==0,'IO fault/schema'
            limit=24000 if base==120 else 384
            assert all(0<=w[base+i]<=limit for i in (2,10)),'live count/generation bound'
            assert 0<=w[base+4]<0x80000000,'live IRQ count bound'
            if n>3:
                # Each aligned word is sampled independently. Do not assert a
                # coherent relation between generation and completed count.
                assert all(rows[n-1][base+i]<=w[base+i] for i in (2,4,10)),'live IO counter regression'
        assert 0<=w[112]<=384 and 0<=w[116]<=384 and 0<=w[119]<=192,'live SPI handshake bounds'
        assert 0<=w[173]<=384 and 0<=w[174]<100,'live UART release bounds'
        if n>3:
            assert all(rows[n-1][i]<=w[i] for i in (112,116,119,173,174)),'live handshake counter regression'
    final=rows[-3:]; w=final[-1]
    stable=[i for i in range(96,184) if i not in (99,123,155)]
    assert all([f[i] for i in stable]==[final[0][i] for i in stable] for f in final),'unfinished ledger'
    assert w[52]&0xfe0==0x840 and w[53:55]==[1,1] and w[55]&16,'clock contract'
    psps=[w[i] for i in (30,67,83,46,47,51)]
    assert len(set(psps))==6 and all(0x20000000<=p<0x2000e000 and p&7==0 for p in psps),'owner PSP'
    assert 17000<=w[122]<24000,'I2C lifetime count'
    for base,count in [(96,384),(120,w[122]),(152,384)]:
        assert w[base+1:base+3]==[4,count] and w[base+10]==count,'IO incomplete/generation'
        assert w[base+14:base+16]==[0x5aa5a55a,0xa55a5aa5],'buffer canary'
        elapsed(final[0][base+3],w[base+3])
        assert w[base+4]>=count and 0<w[base+6]<10000 and 0<w[base+7]<10000,'IRQ/task wake'
        assert 1_900_000_000<=elapsed(w[base+8],w[base+9])<=1_990_000_000,'not32minute IO interval'
    assert w[108:110]==[0x69963c01,0x69963c02],'SPI task payload'
    assert w[112]==384 and w[115:120]==[2,384,384,10000,192],'SPI handshake/overlap/cadence'
    assert 0<w[101]<50000 and 0<elapsed(w[113],w[114])<50000,'SPI bound'
    assert w[132:134]==[0x40,0x00800001],'I2C NACK'
    assert 4000<=w[136]<20000 and w[137]>=4 and w[138]<10000,'I2C checked cleanup'
    assert w[139:141]==[0,0] and w[141]>0 and w[143]>0,'I2C wake/overlap'
    assert w[144:149]==[0xc3c3c3c3,0xc0,0xc0,0xc0,1] and w[151]==0x2e,'I2C sentinel/priority/route/address'
    assert w[164:166]==[41,19] and w[168]>0 and w[169]==0,'UART IRQ/error'
    assert w[172]==384 and w[173]<=384 and w[174]<100 and w[175]==10000,'UART cadence'
    assert 8000<=w[176]<40000 and w[182]==0 and w[183]&0x301==0x101,'UART cleanup'
    data=b''.join(v.to_bytes(4,'big') for v in w[177:182])
    assert data==b'HOST2RP1 IRQ 0384\r\n\xc3','UART final task payload'
    duration=elapsed(w[170],w[171])
    assert 0<w[157]<=4_600_000 and duration<4_600_000,'UART bound'
    assert elapsed(w[170],w[113])<elapsed(w[170],w[114])<duration,'last SPI/UART overlap'
    ticks=elapsed(rows[3][8],w[8]);us=elapsed(rows[3][11],w[11])
    assert 1_900_000_000<=us<=2_000_000_000 and 950<=us/ticks<=1050,'sustained tick mean/span'
    return dict(classification='HW',result='R2_MIXED_REPEAT_NUMERIC_PASS',external_peer_and_gpio_required=True,
        samples=36,elapsed_us=us,mean_tick_us=us/ticks,switches=w[9],queue_completions=w[15],
        msp_used_bytes=w[18],task_free_words=w[32:39]+[w[45]],
        spi=dict(completions=w[98],irq_entries=w[100],max_us=w[101:104],overlap_count=w[116]),
        i2c=dict(expected_nacks=w[122],irq_entries=w[124],max_us=w[125:128],uart_armed_overlaps=w[143]),
        uart=dict(completions=w[154],irq_entries=w[156],max_us=w[157:160],late_releases=w[173],max_late_ticks=w[174]),
        simultaneous_wire_payloads_proven=False,mutex_inheritance_in_this_image=False,
        R1='PARTIAL',R2='PARTIAL_SUSTAINED_SPI_UART_I2C_NACK',R3='OPEN/BLOCKED')

if __name__=='__main__':
    print(json.dumps(validate(Path(sys.argv[1]).read_text(errors='replace')),indent=2))

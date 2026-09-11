#!/usr/bin/env python3
"""Finite SPM2/ICMP/UAM2 numeric proof. ESP/host/GPIO joins still required."""
import json
from pathlib import Path
import runpy
import sys

if not __debug__:raise SystemExit('assertions required')
B=runpy.run_path(str(Path(__file__).with_name('check-freertos-mixed.py')))
elapsed=B['elapsed']

def validate(text, *, stream=False):
    assert type(stream) is bool
    count=384 if stream else 2
    decode=runpy.run_path(str(Path(__file__).with_name('check-freertos-mixed-repeat.py')))['decode'] if stream else B['decode']
    rows=decode(text); B['validate_runtime'](rows)
    for n,w in enumerate(rows):
        assert not any(w[184:]),'reserved fault region'
        if n<3:continue
        assert w[63]==int.from_bytes(b'MD01','little') and w[60]==0,'monitor schema/miss'
        assert 0<w[61]<10000 and w[62]<10,'monitor timing'
        for base,magic in [(96,b'SPM2'),(120,b'ICMS' if stream else b'ICMP'),(152,b'UAM2')]:
            assert w[base]==int.from_bytes(magic,'little') and w[base+11]==0,'IO schema/error'
            assert all(w[base+i]<=count for i in (2,10)),'request count bound'
            if n>3:assert all(rows[n-1][base+i]<=w[base+i] for i in (2,4,10)),'counter regression'
        if n>3:assert all(rows[n-1][i]<=w[i] for i in (61,62,140)),'monitor/wake counter regression'
    final=rows[-3:]; w=final[-1]
    stable=[i for i in range(96,184) if i not in (99,123,155)]
    assert all([f[i] for i in stable]==[final[0][i] for i in stable] for f in final),'unfinished ledger'
    assert w[52]&0xfe0==0x840 and w[53:55]==[1,1] and w[55]&16,'clocks'
    psps=[w[i] for i in (30,67,83,46,47,51)]
    assert len(set(psps))==6 and all(0x20000000<=p<0x2000e000 and p&7==0 for p in psps),'owner PSP'
    for base in (96,120,152):
        assert w[base+1:base+3]==[4,count] and w[base+10]==count,'request budget incomplete'
        assert w[base+14:base+16]==[0x5aa5a55a,0xa55a5aa5],'buffer canary'
        elapsed(final[0][base+3],w[base+3])
        assert w[base+4]>=count and 0<w[base+6]<10000 and 0<w[base+7]<10000,'IRQ wake'
        interval=elapsed(w[base+8],w[base+9])
        assert (1900000000<=interval<=1990000000 if stream else interval<10000000),'request interval'
    assert w[108:110]==[0x69963c01,0x69963c02],'SPI payload'
    assert w[112]==count and w[115:120]==[2,count,count,10000,count//2],'SPI handshake'
    assert 0<w[101]<50000 and elapsed(w[113],w[114])<50000,'SPI bound'
    assert w[132:134]==[0,0] and w[139]==33*count//2,'I2C normal IRQ byte count'
    # Mixed owners can have equal priority; a delivered notification need not
    # request an immediate switch. Require at least one such switch in the run,
    # not one per transaction. Every transaction still requires IRQ and payload.
    assert 0<w[140]<=count,'I2C aggregate higher-priority wake evidence'
    assert 4000<=w[136]<20000 and w[137]>=4 and w[138]<10000,'I2C cleanup'
    assert w[141]==0x314ec3c3 and w[150]==24 and w[151]==0x2d000000|count,'I2C first frame/IRQ/address'
    second=b''.join(v.to_bytes(4,'big') for v in w[142:150])
    expected=(bytes([(count-1)&255 ^ 0x31,((count-1)>>8)^0x4e])+
        bytes((0xb4+i*0x1d+(count-1)*7)&255 for i in range(2,31)) if stream else
        bytes((0x31+0x83+i*0x1d)&255 for i in range(31)))+b'\xc3'
    assert second==expected,'I2C second frame/tail'
    assert 0<w[125]<50000,'I2C deadline'
    assert w[164:166]==[41,19] and w[168]>0 and w[169]==0,'UART IRQ/error'
    assert w[172]==count and w[173]<=count and w[174]<100 and w[175]==10000,'UART cadence'
    assert 8000<=w[176]<40000 and w[182]==0 and w[183]&0x301==0x101,'UART cleanup'
    data=b''.join(v.to_bytes(4,'big') for v in w[177:182])
    assert data==f'HOST2RP1 IRQ {count:04d}\r\n'.encode()+b'\xc3','UART task payload'
    duration=elapsed(w[170],w[171])
    assert 0<w[157]<4600000 and duration<4600000,'UART bound'
    assert elapsed(w[170],w[113])<elapsed(w[170],w[114])<duration,'SPI/UART overlap'
    if stream:assert elapsed(w[171],w[129])<1000000,'last I2C did not follow UART token within grant deadline'
    ticks=elapsed(rows[3][8],w[8]); us=elapsed(rows[3][11],w[11])
    assert (1900000000<=us<=2000000000 if stream else 20000000<=us<=80000000) and 950<=us/ticks<=1050,'tick span/mean'
    return dict(classification='HW',result='R2_I2C_MIXED_STREAM_NUMERIC_PASS' if stream else 'R2_FINITE_I2C_MIXED_PAIR_NUMERIC_PASS',
        external_peer_and_gpio_required=True,samples=len(rows),elapsed_us=us,mean_tick_us=us/ticks,
        switches=w[9],queue_completions=w[15],msp_used_bytes=w[18],task_free_words=w[32:39]+[w[45]],
        spi=dict(completions=count,irq_entries=w[100],max_us=w[101:104],uart_overlap_count=w[116]),
        i2c=dict(completions=count,received=w[139],irq_entries=w[124],ipsr=w[150],max_us=w[125:128],
                 higher_priority_wakes=w[140],
                 first=[0x31,0x4e],second=list(second[:31])),
        uart=dict(completions=count,irq_entries=w[156],max_us=w[157:160]),
        monitor=dict(misses=w[60],max_body_us=w[61],max_late_ticks=w[62]),
        simultaneous_wire_payloads_proven=False,continuous_i2c_proven=False,
        R1='PARTIAL',R2='PARTIAL_BOUNDED_NORMAL_I2C_STREAM_WITH_SPI_UART' if stream else 'PARTIAL_FINITE_NORMAL_I2C_PAIR_WITH_SPI_UART',R3='OPEN/BLOCKED')

if __name__=='__main__':print(json.dumps(validate(Path(sys.argv[1]).read_text(errors='replace')),indent=2))

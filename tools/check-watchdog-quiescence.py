#!/usr/bin/env python3
"""WDT3 already-disabled receipt -> last host ACK -> typed GPIO event. Not expiry."""
import json
from pathlib import Path
import re
import runpy
import sys

if not __debug__:raise SystemExit('assertions required')
W=runpy.run_path(str(Path(__file__).with_name('check-freertos-watchdog.py')))
REQUEST=[int.from_bytes(b'WQ03','little'),3,1,1,0xffffff,256,0,0x57445432^3^1^1^0xffffff^256]
ACK=[int.from_bytes(b'QA03','little'),3,1,2,0,0,0,0x57445432^3^1^2]
END='[WQ3] observer-quiesced no-more-rp1-access=1'
FRAME=0xa50101ff

def validate_uart(text):
    assert text.count(END)==1 and '[WQ3] failure=' not in text
    records=re.findall(r'\[WQ3\] record (\d{3}) ((?:[0-9a-f]{8} ?){4})',text)
    assert [int(n) for n,_ in records]==list(range(0,256,4)),'missing/duplicate record'
    words=[int(n,16) for _,row in records for n in row.split()]
    for label,expected in [('request',REQUEST),('ack-issued',ACK)]:
        lines=re.findall(r'\[WQ3\] '+label+r' ((?:[0-9a-f]{8} ?){8})',text)
        assert len(lines)==1 and [int(n,16) for n in lines[0].split()]==expected
    assert text.index('[WQ3] request ')<text.index('[WQ3] record 000 ')<text.index('[WQ3] record 252 ')<text.index('[WQ3] ack-issued ')<text.index(END)
    assert '[WQ3]' not in text.split(END,1)[1] and '[RTOS]' not in text.split(END,1)[1]
    assert words[:5]==[0x31305452,1,5,0,0] and (words[70]|words[86])==0
    assert words[66]==words[82]==2 and words[67]!=words[83]
    for n in [67,83]:assert 0x20000000<=words[n]<0x2000e000 and words[n]&7==0
    W['validate_disabled_record'](words,magic=int.from_bytes(b'WDT3','little'),version=3,request=REQUEST)
    return words

def decode_frame(events):
    marker=[e for e in events if e['source']=='gpio25']
    assert all(e['level'] in [0,1] and e['edge']==('rising' if e['level'] else 'falling') for e in marker)
    frames=[]
    for start in range(1,len(marker)-65):
        if marker[start]['level']!=1 or marker[start]['timestamp_us']-marker[start-1]['timestamp_us']<400000:continue
        pairs=[marker[start+2*i:start+2*i+2] for i in range(33)]
        if not all(a['level']==1 and b['level']==0 for a,b in pairs):continue
        highs=[b['timestamp_us']-a['timestamp_us'] for a,b in pairs]
        lows=[pairs[i+1][0]['timestamp_us']-pairs[i][1]['timestamp_us'] for i in range(32)]
        if not all(25000<=x<=75000 for x in lows) or not 375000<=highs[-1]<=425000:continue
        if not all(25000<=x<=75000 or 125000<=x<=175000 for x in highs[:32]):continue
        word=0
        for duration in highs[:32]:word=(word<<1)|int(duration>100000)
        assert word==FRAME,'wrong type/sequence/checksum'
        frames.append(dict(first_edge=start,last_edge=start+65,word=f'{word:08x}',
            high_min_us=min(highs[:32]),high_max_us=max(highs[:32]),
            gap_min_us=min(lows),gap_max_us=max(lows)))
    assert len(frames)==1,'missing/duplicate typed frame'
    assert len(marker)-1-frames[0]['last_edge']>=36,'missing post-event heartbeat tail'
    return frames[0]

def validate(text,protocol,before,after):
    words=validate_uart(text)
    tail=W['R1']['validate_trace'](protocol,before,after)
    events,_=json.JSONDecoder().raw_decode(protocol.lstrip())
    frame=decode_frame(events)
    return dict(classification='HW',result='RP1_WATCHDOG_DISABLED_OBSERVER_QUIESCENCE_SELECTED_PASS',
        watchdog_enabled_ctrl=words[107:109],watchdog_disabled_ctrl=words[109],
        bounded_probe_us=words[110],owner_stack_free_before_event=words[144],
        event=frame,external_tail=tail,host_requests=2,ack_readback=False,
        host_silence_basis='selected source caller-to-halt; not a general PCIe traffic measurement',
        expiry_tested=False,restart_proven=False)

if __name__=='__main__':
    assert len(sys.argv)==5,'UART trace before after'
    print(json.dumps(validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:])),indent=2))

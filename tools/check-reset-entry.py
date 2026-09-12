#!/usr/bin/env python3
"""WDT6 software Reset-entry selftest, not watchdog/automatic runtime restart."""
import json
from pathlib import Path
import runpy
import sys

assert __debug__
P=runpy.run_path(str(Path(__file__).with_name('check-watchdog-postack.py')))

def decode(events, nonce, expected_reason):
    marker=[e for e in events if e['source']=='gpio25']
    assert all(type(e['timestamp_us']) is int and e['timestamp_us']>=0 and
        type(e['level']) is int and e['level'] in (0,1) and
        e['edge']==('rising' if e['level'] else 'falling') for e in marker)
    assert all(a['level']!=b['level'] and a['timestamp_us']<b['timestamp_us']
        for a,b in zip(marker,marker[1:]))
    starts=[i for i in range(1,len(marker)-1) if marker[i]['level']==1 and
        marker[i]['timestamp_us']-marker[i-1]['timestamp_us']>=400_000 and
        125_000<=marker[i+1]['timestamp_us']-marker[i]['timestamp_us']<=175_000]
    assert starts,'missing entry frame'
    start=starts[0]
    assert len(marker)-start==66,'truncated/duplicate/extra suffix'
    pairs=[marker[start+2*i:start+2*i+2] for i in range(33)]
    assert all(a['level']==1 and b['level']==0 for a,b in pairs)
    highs=[b['timestamp_us']-a['timestamp_us'] for a,b in pairs]
    lows=[pairs[i+1][0]['timestamp_us']-pairs[i][1]['timestamp_us'] for i in range(32)]
    assert all(25_000<=x<=75_000 for x in lows)
    assert 375_000<=highs[-1]<=425_000,'missing full delimiter'
    word=0
    for width in highs[:32]:
        assert 25_000<=width<=75_000 or 125_000<=width<=175_000,'bit width'
        word=(word<<1)|int(width>100_000)
    check=0
    for shift in range(0,32,4):check^=(word>>shift)&15
    assert word>>28==0xb and check==0,'type/checksum'
    assert (word>>12)&0xffff==nonce and 0<nonce<=0xffff,'run nonce'
    assert (word>>4)&255==expected_reason==2,'software entry reason changed'
    return dict(word=f'{word:08x}',nonce=nonce,raw_reason=expected_reason,
        first_timestamp_us=marker[start]['timestamp_us'],last_timestamp_us=marker[-1]['timestamp_us'],
        prefix_edges=start,events=len(events),high_min_us=min(highs[:32]),
        high_max_us=max(highs[:32]),low_min_us=min(lows),low_max_us=max(lows),
        timestamp_resolution_us=1,absolute_clock_accuracy_proven=False)

def validate(text,protocol,before,after):
    words=P['W3']['validate_uart'](text,version=6)
    gate=P['gates'](text,version=6)
    events=P['W3']['W']['R1']['decode_trace'](protocol,before,after)
    frame=decode(events,words[182],words[112])
    return dict(classification='HW',result='RP1_SOFTWARE_RESET_ENTRY_IDENTITY_SELECTED_PASS',
        frame=frame,handoff=gate,watchdog_disabled_ctrl=words[109],
        watchdog_expiry_tested=False,hardware_reset_proven=False,runtime_restart_proven=False,
        boundary='Controlled branch to Reset; early ID capture and halt before PCIe/kernel reinit. Not autonomous watchdog restart.')

if __name__=='__main__':
    assert len(sys.argv)==5,'UART trace before after'
    print(json.dumps(validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:])),indent=2))

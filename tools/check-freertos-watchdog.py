#!/usr/bin/env python3
"""WDT2 same-run fixed request/accept/countdown/disable; NOT restart proof."""
import json
from pathlib import Path
import re
import runpy
import sys

if not __debug__: raise SystemExit('assertions required')
R1=runpy.run_path(str(Path(__file__).with_name('check-freertos-runtime.py')))
LOAD=0xffffff
REQUEST=[int.from_bytes(b'WQ02','little'),2,1,1,LOAD,256,0,0x57445432^2^1^1^LOAD^256]
MAGIC=int.from_bytes(b'WDT2','little')

def validate_disabled_record(w,magic=MAGIC,version=2,request=REQUEST):
    assert len(w)==256 and w[96:100]==[magic,version,4,0] and w[100]==1 and w[103]==1
    assert w[176:184]==request and not any(w[184:256])
    v=w[104:116]
    assert v[0:3]==[0,3,50]
    assert v[3]&0xff000000==v[4]&0xff000000==0x40000000
    assert (v[3]&LOAD)>(v[4]&LOAD)>LOAD-65536
    assert v[5]&0xff000000==0
    assert 256<=v[6]<=1000 and 0<v[7]<=100000
    assert v[8:12]==[2,2,0,0]
    assert 0<w[101] and v[6]<=((w[102]-w[101])&0xffffffff)<10000
    if version in (6,7):
        assert 0<request[6]<=0xffff and w[139]==request[6]
        assert not any(w[136:139])
    else: assert not any(w[136:140])
    assert not any(w[116:128]+w[145:176])
    for before,after in zip(w[128:132],w[132:136]):
        assert before>0 and 0<((after-before)&0xffffffff)<0x80000000
    for before,after in [(w[140],w[141]),(w[142],w[143])]:
        assert 0<((after-before)&0xffffffff)<0x80000000
    assert 0<w[144]<512

def validate(text):
    footer=R1['WATCHDOG_FOOTER']
    result=R1['validate'](text,footer=footer)
    samples=R1['decode'](text,footer=footer)
    requests=re.findall(r'\[WDT2\] request ((?:[0-9a-f]{8} ?){8})',text)
    assert len(requests)==1 and [int(x,16) for x in requests[0].split()]==REQUEST
    assert '[WDT2] request readback failed' not in text
    assert all(w[99]==0 and w[98]!=0xffffffff for w in samples)
    terminal=None
    for w in samples[8:]:
        validate_disabled_record(w)
        if terminal is None: terminal=w[96:184]
        assert terminal==w[96:184], 'terminal receipt changed/rearmed'
    final=samples[-1]
    result.update(result='RP1_FREERTOS_WATCHDOG_BOUNDED_ARM_RECEIPT_SELECTED_PASS',R3='PARTIAL',
        watchdog=dict(requests=1,firmware_accepted_sequence=final[100],load=LOAD,
            enabled_ctrl=final[107:109],disabled_ctrl=final[109],elapsed_us=final[110],
            countdown_ticks=(final[107]&LOAD)-(final[108]&LOAD),iterations=final[111],
            reason=final[112:114],primask=final[114:116],owner_stack_free_words=final[144],
            ticks_after_ready=(final[141]-final[140])&0xffffffff,
            switches_after_ready=(final[143]-final[142])&0xffffffff,
            expiry_tested=False,restart_proven=False,health_feeding_proven=False,
            recovery_scope='ENABLE/writable CTRL clear; inactive count intentionally not restored'))
    return result

if __name__=='__main__':
    assert len(sys.argv) in (2,5)
    result=validate(Path(sys.argv[1]).read_text(errors='replace'))
    if len(sys.argv)==5:result['external_gpio']=R1['validate_trace'](*(Path(p).read_text() for p in sys.argv[2:]))
    print(json.dumps(result,indent=2))

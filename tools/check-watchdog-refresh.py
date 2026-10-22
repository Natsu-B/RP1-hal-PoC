#!/usr/bin/env python3
"""WDL1 one enabled-LOAD counter refresh then checked disable; not health feeding."""
import json
from pathlib import Path
import re
import runpy
import sys

assert __debug__
W=runpy.run_path(str(Path(__file__).with_name('check-freertos-watchdog.py')))
R1=W['R1']

def validate(text):
    footer=R1['WATCHDOG_FOOTER']
    result=R1['validate'](text,footer=footer)
    samples=R1['decode'](text,footer=footer)
    requests=re.findall(r'\[WDL1\] request ((?:[0-9a-f]{8} ?){8})',text)
    assert len(requests)==1
    request=[int(v,16) for v in requests[0].split()]
    assert request==W['refresh_request'](request[6])
    assert '[WDL1] request readback failed' not in text
    assert all(w[99]==0 and w[98]!=0xffffffff for w in samples)
    terminal=None
    for words in samples[8:]:
        W['validate_disabled_record'](words,magic=W['REFRESH_MAGIC'],version=10,
                                     request=request,load_refresh=True)
        if terminal is None:terminal=words[96:184]
        assert terminal==words[96:184], 'terminal receipt changed/rearmed'
    assert terminal is not None
    w=samples[-1];load=W['LOAD']
    result.update(result='RP1_WATCHDOG_ENABLED_LOAD_REFRESH_SELECTED_PASS',R3='PARTIAL',
        nonce=request[6],watchdog=dict(requests=1,source_enabled_reload_writes=1,load=load,
            mmio_writes_individually_traced=False,
            initial_ctrl=w[104],enabled_ctrl=w[107:109],reloaded_ctrl=w[116],disabled_ctrl=w[109],
            counter_refresh_ticks=(w[116]&load)-(w[108]&load),
            pre_reload_descent_ticks=(w[107]&load)-(w[108]&load),
            pre_reload_wait_us=w[110],iterations=w[111],reason=w[112:114],primask=w[114:116],
            owner_stack_free_words=w[144],
            accepted_to_post_probe_us=(w[102]-w[101])&0xffffffff,
            ticks_after_ready=(w[141]-w[140])&0xffffffff,
            switches_after_ready=(w[143]-w[142])&0xffffffff,
            expiry_tested=False,restart_proven=False,health_feeding_proven=False,
            sustained_post_reload_descent_proven=False,
            masked_interval_duration_proven=False,
            boundary='One upward counter refresh in a fixed cold masked probe. Pre-reload wait excludes arm/reload/disable overhead; wider worker bracket is not exact PRIMASK time. No post-ACK arm or warm/peripheral feeding.'))
    return result

if __name__=='__main__':
    assert len(sys.argv) in (2,5)
    result=validate(Path(sys.argv[1]).read_text(errors='replace'))
    if len(sys.argv)==5:result['external_gpio']=R1['validate_trace'](*(Path(p).read_text() for p in sys.argv[2:]))
    print(json.dumps(result,indent=2))

#!/usr/bin/env python3
"""WDT7: same-run ARM plus nonce/reason early Reset entry; NOT warm RTOS restart."""
import json
from pathlib import Path
import runpy
import sys

assert __debug__
E=runpy.run_path(str(Path(__file__).with_name('check-reset-entry.py')))
P=E['P']
PASS='RP1_WATCHDOG_EXPIRY_EARLY_ENTRY_SELECTED_PASS'

def decode(events,nonce):
    marker,start=E['markers'](events)
    # The unchanged 16-bit codec handles complete error/zero/ARM-only outcomes.
    # They never become restart proof, even if their own diagnostic is valid.
    size=len(marker)-start
    if size in (34,68):
        old=P['decode_packets'](marker)
        result=old['result']
        if result.endswith('_PASS'):
            dt=old['armed_to_terminal_first_edge_us']
            assert 18_990_000<=dt<20_010_000,'WDT7 must allow expiry until19s'
            result='SAFE_NEGATIVE_ZERO_ALIVE_DISABLED'
        return dict(result=result,frames=old['frames'],version=7,
            hardware_reset_entry_proven=False,runtime_restart_proven=False)
    assert size==100,'expected complete16-bit ARM plus32-bit entry; no discarded suffix'
    arm=P['decode_packets'](marker[:start+34])
    assert [f['word'] for f in arm['frames']]==['a21c'],'first event is not ARM'
    # Include the ARM delimiter's falling edge to check the entry low guard.
    entry=E['decode'](marker[start+33:],nonce,(1,3))
    dt=entry['first_timestamp_us']-arm['frames'][0]['first_timestamp_us']
    assert 16_000_000<=dt<20_000_000,'selected external expiry envelope'
    return dict(result=PASS,armed=arm['frames'][0],entry=entry,version=7,
        armed_to_entry_first_edge_us=dt,marker_edges=len(marker),trace_events=len(events),
        prefix_marker_edges=start,hardware_reset_entry_proven=True,runtime_restart_proven=False,
        timestamp_resolution_us=1,absolute_clock_accuracy_proven=False)

def validate(text,protocol,before,after):
    words=P['W3']['validate_uart'](text,version=7)
    handoff=P['gates'](text,version=7)
    events=P['W3']['W']['R1']['decode_trace'](protocol,before,after)
    external=decode(events,words[182])
    return dict(classification='HW',result=external['result'],external=external,handoff=handoff,
        nonce=words[182],prior_short_probe_us=words[110],prior_disabled_ctrl=words[109],
        hardware_reset_entry_proven=external['hardware_reset_entry_proven'],
        runtime_restart_proven=False,R3='OPEN',
        boundary='Selected expiry-to-early-entry identity only with fixed-source admission; halt before PCIe/kernel reinitialization. No full recovery claim.')

if __name__=='__main__':
    assert len(sys.argv)==5,'UART trace before after'
    result=validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:]))
    print(json.dumps(result,indent=2))
    raise SystemExit(0 if result['result']==PASS else 1)

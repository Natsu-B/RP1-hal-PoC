#!/usr/bin/env python3
"""WDT8 selected fresh proc0 kernel witness; source admission is mandatory."""
import json
from pathlib import Path
import runpy
import sys

if not __debug__:raise SystemExit('assertions required')
A=runpy.run_path(str(Path(__file__).with_name('check-expiry-entry.py')))
E=A['E']; P=A['P']
PASS='RP1_WATCHDOG_FRESH_PROC0_KERNEL_RESTART_SELECTED_PASS'


def decode(events,nonce,*,fixed_source_admitted=False):
    assert type(fixed_source_admitted) is bool,'explicit source admission required'
    assert type(nonce) is int and 0<nonce<=0xffff,'run nonce'
    assert len(events)<=192,'trace capacity exceeded'
    assert all(type(e['seq']) is int and e['seq']==i and
        type(e['timestamp_us']) is int and e['timestamp_us']>=0
        for i,e in enumerate(events)),'invalid trace sequence/time'
    marker,start=E['markers'](events)
    prefix_events=events.index(marker[start])
    assert prefix_events<=72,'selected prefix edge budget exceeded'
    assert events[prefix_events:]==marker[start:],'foreign post-ARM edge/suffix'
    size=len(marker)-start
    if size in (34,68):
        # Reuse WDT7's complete ARM/error/zero-alive classifications and limits.
        # No such diagnostic, admitted or otherwise, is fresh-kernel proof.
        negative=A['decode'](events,nonce)
        negative.update(version=8,fixed_source_admitted=fixed_source_admitted)
        return negative
    assert size==100,'expected complete16-bit ARM plus32-bit kernel frame; no discarded suffix'
    arm=P['decode_packets'](marker[:start+34])
    assert [f['word'] for f in arm['frames']]==['a21c'],'first event is not ARM'
    entry=E['decode'](marker[start+33:],nonce,(1,3),expected_type=0xc)
    dt=entry['first_timestamp_us']-arm['frames'][0]['first_timestamp_us']
    assert 21_000_000<=dt<26_000_000,'selected fresh-kernel external timing envelope'
    return dict(result=PASS if fixed_source_admitted else 'INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED',
        candidate_result=PASS,armed=arm['frames'][0],entry=entry,version=8,
        armed_to_entry_first_edge_us=dt,marker_edges=len(marker),trace_events=len(events),
        prefix_marker_edges=start,prefix_trace_events=prefix_events,fixed_source_admitted=fixed_source_admitted,
        hardware_reset_entry_proven=fixed_source_admitted,runtime_restart_proven=fixed_source_admitted,
        timestamp_resolution_us=1,absolute_clock_accuracy_proven=False)


def validate(text,protocol,before,after,*,fixed_source_admitted=False):
    words=P['W3']['validate_uart'](text,version=8)
    handoff=P['gates'](text,version=8)
    assert handoff['before_reload_ncr']==handoff['pre_ack_ncr']==0,'selected host NCR must be zero'
    events=P['W3']['W']['R1']['decode_trace'](protocol,before,after)
    external=decode(events,words[182],fixed_source_admitted=fixed_source_admitted)
    return dict(classification='HW',result=external['result'],external=external,handoff=handoff,
        nonce=words[182],prior_short_probe_us=words[110],prior_disabled_ctrl=words[109],
        hardware_reset_entry_proven=external['hardware_reset_entry_proven'],
        runtime_restart_proven=external['runtime_restart_proven'],
        fixed_source_admitted=fixed_source_admitted,post_ack_registers_read_by_host=False,
        progress_basis='Fixed-source gate after five fresh 1s monitor passes, >=5000 ticks/switches, both spinners, queue/inheritance progress and no faults; not externally sampled counters.',
        exact_progress_counters_externally_sampled=False,R3='OPEN',
        boundary='Selected fresh proc0 kernel only, conditional on frozen source/ELF/hash admission. No PCIe/peripheral recovery, full R2 recovery, autonomous feeding or general recovery claim.')


if __name__=='__main__':
    admitted=sys.argv[1:2]==['--fixed-source-admitted']
    if admitted:del sys.argv[1]
    assert len(sys.argv)==5,'[--fixed-source-admitted] UART trace before after'
    result=validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:]),
                    fixed_source_admitted=admitted)
    print(json.dumps(result,indent=2))
    raise SystemExit(0 if result['result']==PASS else 1)

#!/usr/bin/env python3
"""AU warm UART owner diagnostic OR selected D witness, never old C."""
import json
from pathlib import Path
import runpy
import sys

if not __debug__:raise SystemExit('assertions required')
A=runpy.run_path(str(Path(__file__).with_name('check-expiry-entry.py')))
E=A['E']; P=A['P']
PASS='RP1_WATCHDOG_FRESH_PROC0_UART_IRQ_OWNER_SELECTED_PASS'
DIAGNOSTIC='RP1_WATCHDOG_WARM_UART_DIAGNOSTIC_CAPTURED'
GATE_CODES=(*range(1,6),*range(0x10,0x13),*range(0x20,0x24),*range(0x30,0x35),
    *(n for n in range(0x40,0x50) if n!=0x45),*range(0x50,0x56))


def decode(events,nonce,*,fixed_source_admitted=False):
    assert type(fixed_source_admitted) is bool,'explicit source admission required'
    assert type(nonce) is int and 0<nonce<=0xffff,'run nonce'
    assert len(events)<=192,'trace capacity exceeded'
    # ESP trace_clear restarts seq at ONE. Validate the saved raw capture as-is;
    # WDT8's zero-based synthetic contract must not leak into live admission.
    assert all(type(e['seq']) is int and e['seq']==i and
        type(e['timestamp_us']) is int and e['timestamp_us']>=0
        for i,e in enumerate(events,1)),'invalid trace sequence/time (expected1-based)'
    assert all(a['timestamp_us']<=b['timestamp_us'] for a,b in zip(events,events[1:])), 'trace time reversed'
    marker,start=E['markers'](events)
    prefix_events=events.index(marker[start])
    assert prefix_events<=72,'selected prefix edge budget exceeded'
    assert events[prefix_events:]==marker[start:],'foreign post-ARM edge/suffix'
    size=len(marker)-start
    common=dict(version=9,marker_edges=len(marker),trace_events=len(events),
        prefix_marker_edges=start,prefix_trace_events=prefix_events,
        fixed_source_admitted=fixed_source_admitted,trace_sequence_origin=1,
        timestamp_resolution_us=1,absolute_clock_accuracy_proven=False)
    if size in (34,68):
        # Complete ARM/error/zero-alive frames only, never a restart PASS.
        negative=A['decode'](events,nonce)
        negative.update(common,classification='diagnostic',warm_guard_identity_proven=False)
        return negative
    assert size==100,'expected complete16-bit ARM plus32-bit guard/kernel frame; no discarded suffix'
    arm=P['decode_packets'](marker[:start+34])
    assert [f['word'] for f in arm['frames']]==['a21c'],'first event is not ARM'
    # Select only the leading nibble; the shared parser validates every edge,
    # delimiter, type, nonce, allowed reason/gate, checksum and complete suffix.
    kind=0
    for bit in range(4):
        rise,fall=marker[start+34+2*bit:start+36+2*bit]
        kind=(kind<<1)|int(fall['timestamp_us']-rise['timestamp_us']>100_000)
    assert kind in (0xd,0xe),'not an AU guard/UART owner frame; AT C is not AU proof'
    diagnostic=kind==0xe
    entry=E['decode'](marker[start+33:],nonce,GATE_CODES if diagnostic else (1,3),expected_type=kind)
    dt=entry['first_timestamp_us']-arm['frames'][0]['first_timestamp_us']
    low,high=(16_000_000,19_000_000) if diagnostic and entry['raw_reason']!=0x55 else (21_000_000,26_000_000)
    assert low<=dt<high,'selected warm-guard/fresh-kernel external timing envelope'
    candidate=DIAGNOSTIC if diagnostic else PASS
    result=dict(common,classification='diagnostic' if diagnostic else 'HW',
        result=candidate if fixed_source_admitted else 'INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED',
        candidate_result=candidate,armed=arm['frames'][0],entry=entry,
        armed_to_entry_first_edge_us=dt,warm_guard_identity_proven=diagnostic and fixed_source_admitted,
        hardware_reset_entry_proven=not diagnostic and fixed_source_admitted,
        runtime_restart_proven=not diagnostic and fixed_source_admitted,
        warm_uart_owner_proven=not diagnostic and fixed_source_admitted)
    if diagnostic:
        result['gate_code']=entry['raw_reason']
        result['ispr1_exact_0x00200000']=entry['raw_reason']==0x23 and fixed_source_admitted
    return result


def validate(text,protocol,before,after,*,fixed_source_admitted=False):
    words=P['W3']['validate_uart'](text,version=9)
    handoff=P['gates'](text,version=9)
    assert handoff['before_reload_ncr']==handoff['pre_ack_ncr']==0,'selected host NCR must be zero'
    events=P['W3']['W']['R1']['decode_trace'](protocol,before,after)
    external=decode(events,words[182],fixed_source_admitted=fixed_source_admitted)
    return dict(classification=external['classification'],result=external['result'],
        external=external,handoff=handoff,nonce=words[182],prior_short_probe_us=words[110],
        prior_disabled_ctrl=words[109],hardware_reset_entry_proven=external['hardware_reset_entry_proven'],
        runtime_restart_proven=external['runtime_restart_proven'],
        warm_uart_owner_proven=external.get('warm_uart_owner_proven',False),
        warm_guard_identity_proven=external['warm_guard_identity_proven'],
        fixed_source_admitted=fixed_source_admitted,post_ack_registers_read_by_host=False,
        progress_basis=('Fixed-source D gate after two exact19-byte generations from real IRQ25/IPSR41, checked cleanup/canaries/no-rearm, and five fresh 1s monitor passes, >=5000 ticks/switches, '
            'both spinners, queue/inheritance progress and no faults; not externally sampled counters.'
            if external.get('candidate_result')==PASS else 'No fresh-kernel progress claim: diagnostic only.'),
        exact_progress_counters_externally_sampled=False,R3='OPEN',
        boundary='E is diagnostic, never AU success. D is selected fresh proc0 kernel plus first warm UART IRQ ownership, conditional on source/ELF/hash and independent real peer admission. Old AT C is rejected. No in-flight UART recovery, PCIe/SPI/I2C recovery, full R2 recovery, autonomous feeding or general recovery claim.')


if __name__=='__main__':
    admitted=sys.argv[1:2]==['--fixed-source-admitted']
    if admitted:del sys.argv[1]
    assert len(sys.argv)==5,'[--fixed-source-admitted] UART trace before after'
    result=validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:]),
                    fixed_source_admitted=admitted)
    print(json.dumps(result,indent=2))
    raise SystemExit(0 if result['result'] in (PASS,DIAGNOSTIC) else 1)

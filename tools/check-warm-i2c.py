#!/usr/bin/env python3
"""AY: validate every ARM/READYACK/type9 edge; never discard the bus handshake."""
import json
from pathlib import Path
import runpy
import sys

assert __debug__
A=runpy.run_path(str(Path(__file__).with_name('check-expiry-entry.py')))
E=A['E']; P=A['P']
PASS='RP1_WATCHDOG_FRESH_PROC0_I2C_IRQ_OWNER_SELECTED_PASS'
DIAGNOSTIC='RP1_WATCHDOG_WARM_I2C_DIAGNOSTIC_CAPTURED'
WIDTHS=(13,17,19,23,29)
GATE_CODES=(*range(1,6),*range(0x10,0x13),*range(0x20,0x24),*range(0x30,0x35),
    *(n for n in range(0x40,0x50) if n!=0x45),*range(0x50,0x55),*range(0x70,0x7a))

def decode(events,nonce,*,fixed_source_admitted=False):
    assert type(fixed_source_admitted) is bool,'explicit source admission required'
    assert type(nonce) is int and 0<nonce<=0xffff,'run nonce'
    assert len(events)<=192,'trace capacity'
    assert all(type(e['seq']) is int and e['seq']==i and
        type(e['timestamp_us']) is int and e['timestamp_us']>=0
        for i,e in enumerate(events,1)),'sequence/time'
    assert all(a['timestamp_us']<=b['timestamp_us'] for a,b in zip(events,events[1:])), 'time reversed'
    marker,start=E['markers'](events)
    prefix=events.index(marker[start]); size=len(marker)-start
    assert prefix<=72 and events[prefix:]==marker[start:],'prefix budget or foreign post-ARM event'
    common=dict(version=9,trace_events=len(events),marker_edges=len(marker),
        prefix_trace_events=prefix,prefix_marker_edges=start,trace_sequence_origin=1,
        fixed_source_admitted=fixed_source_admitted,timestamp_resolution_us=1,
        absolute_clock_accuracy_proven=False,warm_i2c_owner_proven=False,
        hardware_reset_entry_proven=False,runtime_restart_proven=False,warm_guard_identity_proven=False)
    if size in (34,68):
        negative=A['decode'](events,nonce)
        negative.update(common,classification='diagnostic')
        return negative
    assert size>=100 and (size-100)%2==0 and size<=110,'incomplete/extra handshake or suffix'
    pairs=(size-100)//2
    arm=P['decode_packets'](marker[:start+34])
    assert [f['word'] for f in arm['frames']]==['a21c'],'not ARM'
    hand=[]
    for i in range(pairs):
        rise,fall=marker[start+34+2*i:start+36+2*i]
        width=fall['timestamp_us']-rise['timestamp_us']
        # Same tolerance as the native READYACK peer, without repairing edges.
        assert (WIDTHS[i]-1)*1000-100<=width<=(WIDTHS[i]+1)*1000+100,'READYACK pulse width/order'
        hand.append(dict(width_us=width,rise_seq=rise['seq'],fall_seq=fall['seq'],
            first_timestamp_us=rise['timestamp_us'],last_timestamp_us=fall['timestamp_us']))
    at=start+34+2*pairs
    kind=0
    for bit in range(4):
        rise,fall=marker[at+2*bit:at+2*bit+2]
        kind=(kind<<1)|int(fall['timestamp_us']-rise['timestamp_us']>100_000)
    assert kind in (9,0xe),'old B/C/D/F is not AY ownership'
    diagnostic=kind==0xe
    entry=E['decode'](marker[at-1:],nonce,GATE_CODES if diagnostic else (1,3),expected_type=kind)
    dt=entry['first_timestamp_us']-arm['frames'][0]['first_timestamp_us']
    code=entry['raw_reason']
    if not diagnostic:
        assert pairs==5 and 21_000_000<=dt<32_000_000,'selected complete owner envelope'
    elif code==0x78:
        assert 96_000_000<=dt<105_000_000,'80-pass diagnostic envelope'
    elif code in (0x77,0x79):
        assert 16_000_000<=dt<105_000_000,'bounded worker/handoff diagnostic envelope'
        if code==0x79:assert pairs==5,'handoff/revocation requires completed pulse sequence'
    else:
        assert pairs==0 and 16_000_000<=dt<19_000_000,'preparation diagnostic envelope'
    if hand:
        assert 16_000_000<=hand[0]['first_timestamp_us']-arm['frames'][0]['first_timestamp_us']<20_000_000,'warm READY envelope'
    candidate=DIAGNOSTIC if diagnostic else PASS
    return dict(common,classification='diagnostic' if diagnostic else 'HW',
        result=candidate if fixed_source_admitted else 'INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED',
        candidate_result=candidate,armed=arm['frames'][0],entry=entry,handshake=hand,
        handshake_pairs=pairs,armed_to_entry_first_edge_us=dt,
        warm_i2c_owner_proven=not diagnostic and fixed_source_admitted,
        hardware_reset_entry_proven=not diagnostic and fixed_source_admitted,
        runtime_restart_proven=not diagnostic and fixed_source_admitted,
        warm_guard_identity_proven=diagnostic and fixed_source_admitted,
        gate_code=code if diagnostic else None)

def validate(text,protocol,before,after,*,fixed_source_admitted=False):
    words=P['W3']['validate_uart'](text,version=9);handoff=P['gates'](text,version=9)
    assert handoff['before_reload_ncr']==handoff['pre_ack_ncr']==0,'selected NCR'
    events=P['W3']['W']['R1']['decode_trace'](protocol,before,after)
    external=decode(events,words[182],fixed_source_admitted=fixed_source_admitted)
    return dict(classification=external['classification'],result=external['result'],external=external,
        handoff=handoff,nonce=words[182],fixed_source_admitted=fixed_source_admitted,
        hardware_reset_entry_proven=external['hardware_reset_entry_proven'],
        runtime_restart_proven=external['runtime_restart_proven'],
        warm_i2c_owner_proven=external['warm_i2c_owner_proven'],
        warm_guard_identity_proven=external['warm_guard_identity_proven'],
        post_ack_registers_read_by_host=False,exact_progress_counters_externally_sampled=False,R3='OPEN',
        boundary='Type9 is a fixed-source witness for checked NACK then real314e via IRQ8/IPSR24, cleanup/canaries/generation, marker handback and five fresh R1 passes. Native lease/reset/real peer receipts must independently join. E never proves runtime recovery; old C/D/F cannot substitute. No combined warmR2, general slave recovery, calibrated physical timing, PCIe recovery or health feeding claim.')

if __name__=='__main__':
    admitted=sys.argv[1:2]==['--fixed-source-admitted']
    if admitted:del sys.argv[1]
    assert len(sys.argv)==5,'[--fixed-source-admitted] UART trace before after'
    result=validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:]),fixed_source_admitted=admitted)
    print(json.dumps(result,indent=2))
    raise SystemExit(0 if result['result'] in (PASS,DIAGNOSTIC) else 1)

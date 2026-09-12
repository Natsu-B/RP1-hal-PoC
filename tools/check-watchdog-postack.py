#!/usr/bin/env python3
"""WDT4 disabled handoff and post-ACK external events. Silence is NOT reset."""
import json
from pathlib import Path
import re
import runpy
import sys

if not __debug__:
    raise SystemExit('assertions required')
W3 = runpy.run_path(str(Path(__file__).with_name('check-watchdog-quiescence.py')))
ARMED, ZERO, ERROR = 0xa21c, 0xa31d, 0xa41a
LATE = 0xa618


def gates(text, *, version=4):
    assert version in (4,5,6,7,8,9)
    patterns = [
        r'\[TFTP\] Rp1Gem release before RP1 reload complete ncr=0x([0-9a-f]{8})',
        r'\[RP1BOOT\] image loaded', r'\[RP1BOOT\] proc0 started',
        r'\[RP1PCIE:post-rp1-reload-reinit\] attempt=\d+ result=success',
        r'\[WQ4\] request ', r'\[WQ4\] record 252 ',
        r'\[WQ4\] pre-ack-gem-stop ncr=0x([0-9a-f]{8})',
        r'\[WQ4\] ack-issued ', r'\[WQ4\] observer-quiesced no-more-rp1-access=1',
    ]
    matches = [list(re.finditer(p.replace('WQ4',f'WQ{version}'), text)) for p in patterns]
    assert all(len(m) == 1 for m in matches), 'missing/duplicate handoff gate'
    events = [m[0] for m in matches]
    assert all(a.end() < b.start() for a, b in zip(events, events[1:])), 'handoff order'
    assert 'stop failed; RP1 reload refused' not in text and f'[WQ{version}] failure=' not in text
    ncr = [int(events[i][1], 16) for i in (0, 6)]
    assert all(n & 0x21c == 0 for n in ncr), 'GEM active at handoff'
    return dict(before_reload_ncr=ncr[0], pre_ack_ncr=ncr[1], required_clear_mask=0x21c,
                dma_drain_proven=False, after_ack_access_basis='source caller-to-WFE; not a bus trace')


def decode_packets(events, *, version=4):
    assert version in (4,5)
    terminal = LATE if version == 5 else ZERO
    marker = [e for e in events if e['source'] == 'gpio25']
    assert all(type(e['timestamp_us']) is int and e['timestamp_us'] >= 0 and
               type(e['level']) is int and e['level'] in (0, 1) and
               e['edge'] == ('rising' if e['level'] else 'falling') for e in marker), 'bad edge'
    assert all(a['level'] != b['level'] and a['timestamp_us'] < b['timestamp_us']
               for a, b in zip(marker, marker[1:])), 'lost/reversed physical edge'
    def bit(dt):
        assert 25_000 <= dt <= 75_000 or 125_000 <= dt <= 175_000, 'bit width'
        return int(dt > 100_000)
    # Prefix is the existing boot/1Hz execution witness. Once a short pulse
    # follows a long low guard AND the known magic-leading ONE, consume the
    # entire remaining marker stream. Boot's ~39ms guarded ZERO is not a start.
    # Never search for a nicer terminal packet or discard an incomplete suffix.
    starts = [i for i in range(1, len(marker)-1) if marker[i]['level'] == 1 and
              marker[i]['timestamp_us']-marker[i-1]['timestamp_us'] >= 400_000 and
              125_000 <= marker[i+1]['timestamp_us']-marker[i]['timestamp_us'] <= 175_000]
    assert starts, 'no post-ACK diagnostic frame'
    start = starts[0]
    prefix_edges = start
    frames = []
    while start < len(marker):
        assert len(marker)-start >= 34, 'truncated frame/delimiter'
        assert marker[start]['timestamp_us']-marker[start-1]['timestamp_us'] >= 400_000, 'leading low guard'
        pairs = [marker[start+2*i:start+2*i+2] for i in range(17)]
        assert all(a['level'] == 1 and b['level'] == 0 for a, b in pairs), 'frame polarity'
        highs = [b['timestamp_us']-a['timestamp_us'] for a, b in pairs]
        lows = [pairs[i+1][0]['timestamp_us']-pairs[i][1]['timestamp_us'] for i in range(16)]
        assert all(25_000 <= x <= 75_000 for x in lows), 'low bit gap'
        assert 375_000 <= highs[-1] <= 425_000, 'missing full 400-tick delimiter'
        word = 0
        for width in highs[:16]:
            word = (word << 1) | bit(width)
        assert word in (ARMED, terminal, ERROR), 'type/sequence/checksum'
        frames.append(dict(word=f'{word:04x}', first_timestamp_us=marker[start]['timestamp_us'],
                           last_timestamp_us=marker[start+33]['timestamp_us'],
                           high_min_us=min(highs[:16]), high_max_us=max(highs[:16]),
                           low_min_us=min(lows), low_max_us=max(lows)))
        start += 34
    words = [int(f['word'], 16) for f in frames]
    assert words in ([ARMED, terminal], [ERROR], [ARMED, ERROR], [ARMED]), 'event order/duplicate/suffix'
    result = 'INCONCLUSIVE_ARMED_ONLY' if words == [ARMED] else 'INCONCLUSIVE_ERROR_EVENT'
    elapsed = None
    if words == [ARMED, terminal]:
        elapsed = frames[1]['first_timestamp_us']-frames[0]['first_timestamp_us']
        # ARMED follows a one-tick decrement check; terminal does not. Edge
        # separation is not raw enable-to-disable time. Selected 10ms allowance
        # covers that offset/tick quantization, not a universal scheduler bound.
        low, high = (14_990_000,15_210_000) if version == 5 else (16_000_000,20_000_000)
        assert low <= elapsed < high, 'selected countdown timing envelope'
        result = ('RP1_WATCHDOG_POSTACK_LATE_COUNT_DISABLED_SELECTED_PASS' if version == 5 else
                  'RP1_WATCHDOG_POSTACK_COUNTER_ZERO_ALIVE_DISABLED_SELECTED_PASS')
    return dict(result=result, frames=frames, prefix_marker_edges=prefix_edges,
                marker_edges=len(marker), trace_events=len(events),
                armed_to_zero_first_edge_us=elapsed if version == 4 else None,
                armed_to_terminal_first_edge_us=elapsed, version=version, timestamp_resolution_us=1,
                edge_window_allowance_us=10_000 if version == 5 else 0,
                absolute_clock_accuracy_proven=False, restart_proven=False)


def validate(text, protocol, before, after, *, version=4):
    words = W3['validate_uart'](text, version=version)
    handoff = gates(text, version=version)
    events = W3['W']['R1']['decode_trace'](protocol, before, after)
    packet = decode_packets(events, version=version)
    return dict(classification='HW', result=packet['result'], external=packet, handoff=handoff,
                prior_short_probe_us=words[110], prior_disabled_ctrl=words[109],
                post_ack_registers_read_by_host=False, restart_proven=False,
                R3='OPEN', boundary='Typed firmware event and external timing; no reset identity, no general recovery proof.')


if __name__ == '__main__':
    version = 4
    if sys.argv[1:2] == ['--late-disable']:
        version = 5
        del sys.argv[1]
    assert len(sys.argv) == 5, 'UART trace before after'
    result = validate(*(Path(p).read_text(errors='replace') for p in sys.argv[1:]), version=version)
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['result'].endswith('_PASS') else 1)

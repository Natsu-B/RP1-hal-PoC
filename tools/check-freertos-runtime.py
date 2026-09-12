#!/usr/bin/env python3
"""Shared R1 evidence checks; explicit fixed-request footer is NOT read-only proof."""
import json
from pathlib import Path
import re
import sys

if not __debug__:
    raise SystemExit('Evidence validators require assertions enabled; refuse -O/PYTHONOPTIMIZE')


READ_ONLY_FOOTER = "[RTOS] observer-complete read-only=1"
WATCHDOG_FOOTER = "[RTOS] observer-complete read-only=0 watchdog-request=1"


def decode(text, *, footer=READ_ONLY_FOOTER):
    samples = {}
    for match in re.finditer(r'\[RTOS\] (\d+) (\d{3}) ((?:[0-9a-f]{8} ?){4})', text):
        sample, offset = map(int, match.group(1, 2))
        words = [int(w, 16) for w in match[3].split()]
        record = samples.setdefault(sample, {})
        assert offset not in record, 'duplicate row'
        record[offset] = words
    assert footer in (READ_ONLY_FOOTER, WATCHDOG_FOOTER)
    assert text.count(footer) == text.count('[RTOS] observer-complete ') == 1, 'observer incomplete/mixed'
    assert sorted(samples) == list(range(31)), 'missing samples'
    words = []
    for number, record in sorted(samples.items()):
        assert sorted(record) == list(range(0, 256, 4)), 'incomplete record'
        w = [word for row in sorted(record) for word in record[row]]
        words.append(w)
    return words


def validate(text, *, footer=READ_ONLY_FOOTER):
    samples = decode(text, footer=footer)
    decoded = []
    for number, w in enumerate(samples):
        assert w[0] == 0x31305452 and w[1] == 1, 'wrong firmware telemetry'
        assert w[2] != 0xffffffff and w[3] == 0 and w[192] != 0x31544652, f'fault at sample {number}'
        if number < 3:
            continue
        assert w[2] == 5, f'monitor not progressing {number}, stage={w[2]}'
        assert w[19:21] == [0x13579bdf, 0], 'data/BSS initialization failed'
        assert w[28] == 0 and w[29] & 3 == 2 and w[30] & 7 == 0, 'first task context'
        assert 0x2000e000 <= w[31] <= 0x2000f000
        assert w[21] & 0x700 == 0 and w[22] + 1 == w[5] // 1000
        assert w[39] == 1 and w[17] > 0 and w[50] > 0
        assert 0 < w[18] < 4096 and all(v > 0 for v in w[32:39]), 'stack guard exhausted'
        assert w[70] == 0 and w[86] == 0, 'register pattern damaged'
        assert w[65] == w[81] == 0 and w[66] & 3 == w[82] & 3 == 2
        assert w[67] != w[83] and w[67] & 7 == w[83] & 7 == 0, 'PSP not separate/aligned'
        assert 0x20000000 <= w[67] < 0x2000e000 and 0x20000000 <= w[83] < 0x2000e000
        if decoded:
            old = decoded[-1]
            for index in [8, 9, 14, 15, 17, 49, 50, 64, 80]:
                assert 0 < (w[index] - old[index]) & 0xffffffff < 0x80000000, f'no progress word {index}'
        decoded.append(w)
    a, b = decoded[0], decoded[-1]
    ticks = (b[8] - a[8]) & 0xffffffff
    elapsed = (b[11] - a[11]) & 0xffffffff
    mean_us = elapsed / ticks
    assert 950 <= mean_us <= 1050, f'tick mean {mean_us}'
    return {'classification':'HW', 'result':'R1_COMMISSIONING_CANDIDATE_PASS',
            'observer_read_only': footer == READ_ONLY_FOOTER,
            'samples':len(samples), 'elapsed_us':elapsed, 'ticks':ticks,
            'mean_tick_us':mean_us, 'tick_min_us':b[12], 'tick_max_us':b[13],
            'switches':b[9], 'monitor_cycles':b[14], 'queue_completions':b[15],
            'inheritances':b[17], 'msp_used_bytes':b[18], 'task_stack_free_words':b[32:39],
            'calibration_hz':b[5:8], 'R1':'PARTIAL', 'R2':'OPEN', 'R3':'OPEN'}


def decode_trace(protocol, before, after):
    """Validate the complete transport envelope before interpreting GPIO payload."""
    def counts(status):
        match = re.search(r'^trace_count=(\d+) overflow=(\d+) gpio_edges=(\d+) ', status, re.M)
        assert match and status.rstrip().endswith('OK'), 'trace status incomplete'
        return tuple(map(int, match.groups()))
    assert counts(before) == (0, 0, 0), 'trace was not empty'
    events, end = json.JSONDecoder().raw_decode(protocol.lstrip())
    assert protocol.lstrip()[end:].strip() == 'OK', 'trace dump incomplete'
    count, overflow, edges = counts(after)
    assert isinstance(events, list) and count == edges == len(events) and overflow == 0, 'trace loss'
    assert all(isinstance(e, dict) for e in events)
    for previous, event in zip(events, events[1:]):
        assert event['seq'] == previous['seq'] + 1, 'trace sequence gap'
        assert event['timestamp_us'] >= previous['timestamp_us'], 'trace time reversed'
    return events

def validate_trace(protocol, before, after):
    """Independent 1Hz GPIO22 -> ESP GPIO25 witness, not a context-switch trace."""
    events = decode_trace(protocol, before, after)
    count = len(events)
    marker = [e for e in events if e['source'] == 'gpio25']
    # Boot/power edges precede the workload. Validate a fixed final 36-edge
    # window, never cherry-pick a middle interval while a stalled tail is ignored.
    assert len(marker) >= 36, 'short external observation'
    marker = marker[-36:]
    intervals = []
    for previous, event in zip(marker, marker[1:]):
        assert event['level'] in (0, 1) and event['level'] != previous['level'], 'marker edge missing'
        assert event['edge'] == ('rising' if event['level'] else 'falling'), 'marker edge mismatch'
        dt = event['timestamp_us'] - previous['timestamp_us']
        assert 990_000 <= dt <= 1_010_000, f'external marker interval {dt}'
        intervals.append(dt)
    return {'trace_events': count, 'checked_marker_edges': len(marker),
            'marker_interval_min_us': min(intervals), 'marker_interval_max_us': max(intervals),
            'marker_checked_elapsed_us': sum(intervals), 'overflow': 0,
            'timestamp_resolution_us': 1, 'absolute_clock_accuracy_proven': False}


if __name__ == '__main__':
    assert len(sys.argv) in (2, 5), 'UART log [trace.json.protocol trace-before.txt trace-status.txt]'
    result = validate(Path(sys.argv[1]).read_text(errors='replace'))
    if len(sys.argv) == 5:
        result['external_gpio'] = validate_trace(*(Path(p).read_text() for p in sys.argv[2:]))
    print(json.dumps(result, indent=2))

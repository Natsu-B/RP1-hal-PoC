#!/usr/bin/env python3
"""Synthetic AZ edge/nonce/refusal check; never a hardware success."""
from copy import deepcopy
from pathlib import Path
import runpy
assert __debug__
O = runpy.run_path(str(Path(__file__).with_name('test-warm-guard.py')))
M = runpy.run_path(str(Path(__file__).with_name('check-warm-combined.py')))
T = O['T']
positive = negative = 0


def trace(kind=8, code=1, pairs=2, dt=23_000_000, word=None):
    ev = O['trace'](O['packet'](kind=kind, gate=code) if word is None else word, interval=dt)
    marker, start = M['E']['markers'](ev)
    at = ev.index(marker[start]) + 34
    origin = marker[start]['timestamp_us']
    hand = []
    for i, width in enumerate(M['WIDTHS'][:pairs]):
        now = origin + 16_900_000 + i*3_000_000
        hand.extend([dict(source='gpio25', level=1, edge='rising', timestamp_us=now),
                     dict(source='gpio25', level=0, edge='falling', timestamp_us=now+width*1000)])
    return O['numbered'](ev[:at] + hand + ev[at:])


def run(ev, admitted=True):
    return M['validate'](O['text'], *T['inputs'](ev), fixed_source_admitted=admitted)


def reject(ev):
    global negative
    try:
        run(ev)
    except (AssertionError, KeyError, ValueError, TypeError, IndexError):
        negative += 1
    else:
        raise AssertionError('invalid AZ capture accepted')


for reason in (1, 3):
    ev = trace(code=reason)
    assert run(ev)['result'] == M['PASS'] and run(ev)['warm_combined_owner_proven']
    positive += 1
    assert not run(ev, False)['warm_combined_owner_proven']
    assert run(ev, False)['result'] == 'INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED'
    word = O['packet'](kind=8, gate=reason)
    for bit in range(32):
        reject(trace(word=word ^ (1 << bit)))
    for count in range(len(ev)):
        try:
            r = run(ev[:count])
        except (AssertionError, KeyError, ValueError, TypeError, IndexError):
            negative += 1
        else:
            assert not r['warm_combined_owner_proven']
for code in M['GATE_CODES']:
    dt = 98_000_000 if code == 0x78 else 23_000_000 if code in (0x77, 0x79) else 16_777_215
    for pairs in range(3) if code in (0x77, 0x78) else [2] if code == 0x79 else [0]:
        r = run(trace(0xe, code, pairs, dt))
        assert r['result'] == M['DIAGNOSTIC'] and not r['runtime_restart_proven']
        positive += 1
for kind in range(16):
    if kind not in (8, 0xe):
        reject(trace(kind))
for pairs in range(2):
    reject(trace(pairs=pairs))
for reason in (0, 2, 4, 255):
    reject(trace(code=reason))
for code in (0x45, 0x55, 0x62, 0x68, 0x69, 0x7a, 0xff):
    reject(trace(0xe, code, 0, 16_777_215))
for dt in (20_999_999, 100_000_000):
    reject(trace(dt=dt))
ev = trace()
marker, start = M['E']['markers'](ev)
at = ev.index(marker[start]) + 34
for i in range(at, at+4):
    bad = deepcopy(ev); bad[i]['source'] = 'gpio23'; reject(bad)
    for delta in (-2500, 2500):
        bad = deepcopy(ev); bad[i]['timestamp_us'] += delta; reject(bad)
for i in range(9, len(ev)):
    reject(O['numbered'](ev[:i]+ev[i+1:]))
    reject(O['numbered'](ev[:i]+[ev[i]]+ev[i:]))
reject([dict(e, seq=i) for i, e in enumerate(ev)])
reject(O['numbered'](ev + [dict(ev[-1], timestamp_us=ev[-1]['timestamp_us']+500_000)]))
for n in (0, 1, 0xffff):
    reject(trace(word=O['packet'](kind=8, n=n, gate=1)))
single = O['numbered'](T['trace']([O['packet'](kind=8, gate=1)], bits=32))
try:
    M['E']['decode'](single, O['nonce'], 1)
except AssertionError:
    negative += 1
else:
    raise AssertionError('default B decoder accepts8')
print(f'PASS: AZ8/E {positive} positives/{negative} refusals plus WDT9 regressions; NOT HW')

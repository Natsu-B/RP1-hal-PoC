#!/usr/bin/env python3
"""Reuse WDT9 adversarial fixtures; F adds SPI ownership, C/D cannot substitute."""
from pathlib import Path
import runpy
assert __debug__
O=runpy.run_path(str(Path(__file__).with_name('test-warm-guard.py')))
M=runpy.run_path(str(Path(__file__).with_name('check-warm-spi.py')))
T=O['T'];positive=negative=0
def trace(kind=0xf,code=1,dt=22_277_215):
    return O['trace'](O['packet'](kind=kind,gate=code),interval=dt)
def run(ev,admitted=True):
    return M['validate'](O['text'],*T['inputs'](ev),fixed_source_admitted=admitted)
def reject(ev):
    global negative
    try:run(ev)
    except (AssertionError,KeyError,ValueError,TypeError,IndexError):negative+=1
    else:raise AssertionError('invalid AX frame accepted')
for reason in (1,3):
    ev=trace(code=reason);r=run(ev)
    assert r['result']==M['PASS'] and r['warm_spi_owner_proven'] and r['runtime_restart_proven']
    assert not r['warm_guard_identity_proven'];positive+=1
    r=run(ev,False);assert r['result']=='INCONCLUSIVE_FIXED_SOURCE_NOT_ADMITTED' and not r['warm_spi_owner_proven']
    word=O['packet'](kind=0xf,gate=reason)
    for bit in range(32):reject(O['trace'](word^(1<<bit),interval=22_277_215))
    for count in range(len(ev)):
        try:r=run(ev[:count])
        except (AssertionError,KeyError,ValueError,TypeError,IndexError):negative+=1
        else:assert not r['warm_spi_owner_proven'] and not r['runtime_restart_proven']
for code in M['GATE_CODES']:
    r=run(trace(0xe,code,52_277_215 if code==0x68 else 16_777_215))
    assert r['result']==M['DIAGNOSTIC'] and not r['warm_spi_owner_proven'] and not r['runtime_restart_proven']
    positive+=1
for kind in range(16):
    if kind not in (0xf,0xe):reject(trace(kind))
for reason in (0,2,4,255):reject(trace(code=reason))
for code in (0x45,0x55,0x56,0x62,0x6c,0x69,0xff):reject(trace(0xe,code,16_777_215))
reject(trace(0xe,0x68,16_777_215))
for dt in (49_999_999,56_000_000):reject(trace(0xe,0x68,dt))
for dt in (20_999_999,26_000_000):reject(trace(dt=dt))
ev=trace()
for i in range(9,len(ev)):
    reject(O['numbered'](ev[:i]+ev[i+1:]))
    reject(O['numbered'](ev[:i]+[ev[i]]+ev[i:]))
reject([dict(e,seq=i) for i,e in enumerate(ev)]) # real traces must stay1-based
print(f'PASS: AX F/E {positive} positives/{negative} refusals plus unchanged WDT9 regressions; NOT HW')

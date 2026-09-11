#!/usr/bin/env python3
"""Synthetic MD01/refusals over a real accepted older mixed record; no new HW claim."""
from pathlib import Path
import runpy
import sys

if not __debug__:
    raise SystemExit('tests require assertions')
C=runpy.run_path(str(Path(__file__).with_name('check-freertos-monitor-deadline.py')))
assert len(sys.argv)==2, 'accepted pre-MD01 mixed numeric log required'
text=Path(sys.argv[1]).read_text(); C['BASE']['validate'](text)

def refuses(text):
    try: C['validate'](text)
    except AssertionError: return
    raise AssertionError('bad MD01 accepted')

refuses(text) # Old selected PASS does not prove the new monitor implementation.
rows=C['BASE']['decode'](text)
for n,w in enumerate(rows): w[60:64]=[0,5000+n,1,int.from_bytes(b'MD01','little')]
def render():
    return ''.join(f'[RTOS] {n} {off:03} '+ ' '.join(f'{v:08x}' for v in w[off:off+4])+'\n'
        for n,w in enumerate(rows) for off in range(0,256,4))+'[RTOS] observer-complete read-only=1\n'
C['validate'](render())
for index,value in [(60,1),(61,0),(61,10000),(61,4999),(62,10),(62,0),(63,0)]:
    old=rows[10][index];rows[10][index]=value;refuses(render());rows[10][index]=old
print('STATIC PASS: synthetic MD01 +7 counter/bound/schema refusals; old HW rejected; new HW OPEN')

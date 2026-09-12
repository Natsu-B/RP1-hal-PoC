#!/usr/bin/env python3
"""Reuse existing synthetic R1/WDT2 fixtures; reject unrefreshed/old/replayed WDL1."""
import contextlib
import io
from pathlib import Path
import runpy

assert __debug__
P=Path(__file__).parent
with contextlib.redirect_stdout(io.StringIO()):
    old=runpy.run_path(str(P/'test-freertos-watchdog.py'))
M=runpy.run_path(str(P/'check-watchdog-refresh.py'))
request=M['W']['refresh_request'](17)
rows=[w.copy() for w in old['rows']]
for w in rows:
    w[96]=M['W']['REFRESH_MAGIC'];w[97]=10;w[116]=0x40fffff0
    w[139]=17;w[176:184]=request

def render(data,req=request):
    lines=['[WDL1] request '+' '.join(f'{v:08x}' for v in req)]
    for n,w in enumerate(data):
        for offset in range(0,256,4):
            lines.append(f'[RTOS] {n} {offset:03} '+' '.join(f'{v:08x}' for v in w[offset:offset+4]))
    return '\n'.join(lines+[M['R1']['WATCHDOG_FOOTER']])+'\n'

good=render(rows);result=M['validate'](good)
assert result['nonce']==17 and result['watchdog']['counter_refresh_ticks']==245
assert not result['watchdog']['health_feeding_proven']
negative=0
def reject(text):
    global negative
    try:M['validate'](text)
    except (AssertionError,ValueError,IndexError):negative+=1
    else:raise AssertionError('accepted invalid refresh fixture')

for k,value in [(96,old['M']['MAGIC']),(97,2),(98,3),(99,1),(100,0),(101,0),(102,101),
                (103,0),(104,1),(105,0),(106,0),(107,0x00ffffff),(108,0x40ffffff),
                (108,0xc0fffefb),(109,0x40fffefa),(110,0),(110,1001),(111,0),(111,100001),
                (112,1),(113,1),(114,1),(115,1),(116,0),(116,0x40fffefb),(116,0x40fffefa),
                (116,0x00fffff0),(116,0xc0fffff0),(128,0),(132,1),(139,0),(139,18),
                (141,1),(143,1),(144,0),(144,512),
                *[(i,1) for i in range(117,128)],*[(i,1) for i in range(145,176)],
                *[(i,0xdeadbeef) for i in range(176,184)]]:
    bad=[w.copy() for w in rows];bad[-1][k]=value;reject(render(bad))
bad=[w.copy() for w in rows];bad[-1][116]-=1;reject(render(bad)) # valid jump, changed immutable receipt
reject(old['good'])
reject(good.replace('[WDL1] request ','[WDT2] request '))
reject(good+good.splitlines()[0]+'\n')
reject(good+'[WDL1] request readback failed\n')
reject(good.replace(M['R1']['WATCHDOG_FOOTER'],M['R1']['READ_ONLY_FOOTER']))
for nonce in [0,65536,True]:
    try:M['W']['refresh_request'](nonce)
    except AssertionError:negative+=1
    else:raise AssertionError('bad nonce accepted')
for nonce in [1,65535]:
    req=M['W']['refresh_request'](nonce);data=[w.copy() for w in rows]
    for w in data:w[139]=nonce;w[176:184]=req
    assert M['validate'](render(data,req))['nonce']==nonce
try:M['W']['validate_disabled_record'](rows[-1],magic=M['W']['REFRESH_MAGIC'],version=10,request=request)
except AssertionError:negative+=1
else:raise AssertionError('legacy mode admitted refresh field')
incomplete=rows[-1].copy();incomplete[116]=incomplete[139]=0
try:M['W']['validate_disabled_record'](incomplete,magic=M['W']['REFRESH_MAGIC'],version=10,request=request)
except AssertionError:negative+=1
else:raise AssertionError('legacy mode admitted WDL1 with missing refresh and nonce receipts')
print(f'PASS: WDL1 synthetic refresh, {negative} refusals, nonce bounds and unchanged WDT2 regression; NOT HW')

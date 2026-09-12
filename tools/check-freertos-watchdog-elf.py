#!/usr/bin/env python3
"""Exact reviewed GCC/Rust WDT2 probe, not a general ARM/MMIO verifier."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

if not __debug__: raise SystemExit('assertions required')
# Reviewed94 instructions: own48-byte frame, no calls; guarded LOAD/ENABLE,
# bounded raw-timer loop, CTRL0/readback, original PRIMASK restored, then return.
REVIEWED='5b2036e833bd5897b584522e3b948ea73781e57731b053988bbe4c31775aa62f'
GUARDED='53c1ac6e5995477fe74fca37607fbd8fe6467fe5a3d03c3f89796b8faebf3d0e'
# Size/fat-LTO AU review: same guarded writes; 73 rows, own40-byte PSP frame.
WARM_UART='88521fc9652b40ac31ae5aa5990622568c89d9284613ec4dccba7b74b85c7970'
# O3/fat-LTO AV review: guarded writes, 98 rows, own48-byte PSP frame.
WARM_UART_O3='9ea1c32e531b5b665a05f2169290e6c50be5e7b27433ed1b4563544cfbc14c8d'

def check(text):
    matches=list(re.finditer(r'^([0-9a-f]+) <[^\n]*watchdog6target5probe[^\n]*>:\n(.*?)(?=^[0-9a-f]+ <|\Z)',text,re.M|re.S))
    assert len(matches)==1,'missing/duplicate watchdog probe'
    body=matches[0][2]
    rows=re.findall(r'^\s*[0-9a-f]+:\s+((?:[0-9a-f]{4}\s+)+)\S',body,re.M)
    code=' '.join(' '.join(r.split()) for r in rows)
    digest=hashlib.sha256(code.encode()).hexdigest()
    variants={REVIEWED:(94,48),GUARDED:(97,44),WARM_UART:(73,40),WARM_UART_O3:(98,48)}
    assert digest in variants and len(rows)==variants[digest][0],'probe changed; new review required'
    assert not re.search(r'\sblx?\s',body),'call in bounded probe'
    return dict(probe_address='0x'+matches[0][1],instructions=len(rows),
        opcode_sha256=digest,own_frame_bytes=variants[digest][1],external_calls=0,
        unknown_ctrl_disable_guard=digest in (GUARDED,WARM_UART,WARM_UART_O3))

if __name__=='__main__':
    args=sys.argv[1:]; test=args[:1]==['--self-test']
    if test:args=args[1:]
    assert len(args)==1,'[--self-test] ELF'
    elf=Path(args[0])
    foundation=json.loads(subprocess.check_output([sys.executable,'-B',str(Path(__file__).with_name('check-freertos-elf.py')),str(elf)]))
    assert foundation['status']=='PASS' and foundation['proc1'] is None
    text=subprocess.check_output(['arm-none-eabi-objdump','-d',str(elf)],text=True)
    proof=check(text)
    if test:
        for bad in [text.replace('b672','b673'),text.replace('watchdog6target5probe','watchdog6target5other')]:
            try:check(bad)
            except AssertionError:pass
            else:raise AssertionError('mutation accepted')
    print(json.dumps(dict(classification='BUILD',status='PASS',hardware='OPEN',
        elf_sha256=hashlib.sha256(elf.read_bytes()).hexdigest(),foundation=foundation,
        probe=proof,negative_tests=2 if test else 0),indent=2))

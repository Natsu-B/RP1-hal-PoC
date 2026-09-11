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

def check(text):
    matches=list(re.finditer(r'^([0-9a-f]+) <[^\n]*watchdog6target5probe[^\n]*>:\n(.*?)(?=^[0-9a-f]+ <|\Z)',text,re.M|re.S))
    assert len(matches)==1,'missing/duplicate watchdog probe'
    body=matches[0][2]
    rows=re.findall(r'^\s*[0-9a-f]+:\s+((?:[0-9a-f]{4}\s+)+)\S',body,re.M)
    code=' '.join(' '.join(r.split()) for r in rows)
    assert len(rows)==94 and hashlib.sha256(code.encode()).hexdigest()==REVIEWED,'probe changed; new review required'
    assert not re.search(r'\sblx?\s',body),'call in bounded probe'
    return dict(probe_address='0x'+matches[0][1],instructions=len(rows),
        opcode_sha256=REVIEWED,own_frame_bytes=48,external_calls=0)

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

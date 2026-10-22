#!/usr/bin/env python3
"""Exact AZ14 compiled checkpoint; not generic watchdog or hardware admission."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

if not __debug__:
    raise SystemExit('assertions required')
ELF_SHA = '84eca53f5e971036838ce74d1543505bcb487afa0d3d85f60d1005d368e81c33'
PROBE_SHA = 'e1ab8d3f40de356d5eb7a522af3499064964538bee4c92f73731969f416ed48c'

def identity(data):
    assert hashlib.sha256(data).hexdigest() == ELF_SHA, 'new image requires compiled review'

def probe(disassembly):
    rows = []
    for line in disassembly.splitlines():
        m = re.match(r'^\s*([0-9a-f]+):\s+((?:[0-9a-f]{4}\s+)+)(\S.*)', line)
        if m and 0x20003d10 <= int(m[1], 16) <= 0x20003dd6:
            rows.append(m)
    code = ' '.join(' '.join(m[2].split()) for m in rows)
    assert len(rows) == 77 and hashlib.sha256(code.encode()).hexdigest() == PROBE_SHA, 'inlined critical envelope changed'
    assert not any(re.match(r'blx?\b', m[3]) for m in rows), 'call in critical envelope'
    return {'start':'0x20003d10', 'last':'0x20003dd6', 'rows':77,
            'opcode_sha256':PROBE_SHA, 'calls':0, 'inlined_in_worker':True}

def check(path, self_test=False):
    data = Path(path).read_bytes()
    identity(data)
    spec = importlib.util.spec_from_file_location('foundation', Path(__file__).with_name('check-freertos-elf.py'))
    foundation = importlib.util.module_from_spec(spec); spec.loader.exec_module(foundation)
    base = foundation.check(path)
    assert base['status'] == 'PASS' and base['proc1'] is None
    assert {24,35,41} <= set(base['checked_vector_indices'])
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', str(path)], text=True)
    critical = probe(dis)
    negatives = 0
    if self_test:
        for index in (0, len(data)//2, len(data)-1):
            bad = bytearray(data); bad[index] ^= 1
            try: identity(bad)
            except AssertionError: negatives += 1
            else: raise AssertionError('changed image accepted')
        try: probe(dis.replace('cpsid', 'bl'))
        except AssertionError: negatives += 1
        else: raise AssertionError('critical call mutation accepted')
    return {'classification':'BUILD', 'status':'PASS', 'elf_sha256':ELF_SHA,
            'review':'AZ linked-review-14', 'foundation':base, 'critical':critical,
            'shadow_end':'0x2000df9c', 'application_headroom_bytes':100,
            'task_stack_pool_words':2304, 'owner_stack_bytes':2048,
            'msp_bytes':4096, 'negative_tests':negatives,
            'hardware':'OPEN', 'hardware_admitted':False,
            'cold_whole_path_timing_equivalence':False}

if __name__ == '__main__':
    args = sys.argv[1:]; test = args[:1] == ['--self-test']
    if test: args = args[1:]
    assert len(args) == 1, '[--self-test] ELF'
    print(json.dumps(check(args[0], test), indent=2))

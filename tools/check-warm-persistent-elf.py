#!/usr/bin/env python3
"""Exact reviewed BC12 build, not a generic or hardware admission."""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys

assert __debug__, 'assertions required'
ELF_SHA = 'abb0192767f849ccbaaaecf0c60d408084d84451192ad46a9c034dbfb1fa854a'
PROBE_SHA = '01646de898d50b65a0bc7c1fd8e8904ec88930c93e5cba281968a36200c1949d'
START, END = 0x200042d8, 0x2000439e


def identity(data):
    assert hashlib.sha256(data).hexdigest() == ELF_SHA, 'new image requires independent compiled review'


def interval(data):
    header = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
    found = []
    for i in range(header[10]):
        kind, offset, va, _, size, *_ = struct.unpack_from('<8I', data, header[5] + i * header[9])
        if kind == 1 and va <= START < END <= va + size:
            found.append(data[offset + START - va:offset + END - va])
    assert len(found) == 1 and len(found[0]) == 198
    return found[0]


def probe(code):
    assert hashlib.sha256(code).hexdigest() == PROBE_SHA, 'reviewed masked interval changed'


def check(path, self_test=False):
    data = Path(path).read_bytes()
    identity(data)
    probe(interval(data))
    spec = importlib.util.spec_from_file_location('foundation', Path(__file__).with_name('check-freertos-elf.py'))
    foundation = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(foundation)
    base = foundation.check(path)
    assert base['status'] == 'PASS' and base['proc1'] is None
    assert {24, 35, 41} <= set(base['checked_vector_indices'])
    symbols = {}
    for row in subprocess.check_output(['arm-none-eabi-nm', '-n', str(path)], text=True).splitlines():
        fields = row.split()
        if len(fields) == 3:
            symbols[fields[2]] = int(fields[0], 16)
    assert symbols['__warm_data_shadow_end'] == 0x2000df8c
    assert '__aeabi_uldivmod' not in symbols
    assert {'memset', '__aeabi_memclr', '__aeabi_memset'} <= set(symbols)
    negative = 0
    if self_test:
        for i in (0, len(data)//2, len(data)-1):
            altered = bytearray(data); altered[i] ^= 1
            try: identity(altered)
            except AssertionError: negative += 1
            else: raise AssertionError('changed image admitted')
        altered = bytearray(interval(data)); altered[4] ^= 1
        try: probe(altered)
        except AssertionError: negative += 1
        else: raise AssertionError('changed masked interval admitted')
    return dict(classification='BUILD', status='PASS', elf_sha256=ELF_SHA,
        foundation=base, critical_interval=[hex(START), hex(END)],
        critical_raw_bytes=198, critical_raw_sha256=PROBE_SHA,
        critical_review='BC12: 74 reachable instructions, no calls/external exits; old PRIMASK in r12 restored',
        shadow_end='0x2000df8c', application_headroom_bytes=116,
        task_stack_pool_words=2304, owner_stack_bytes=2048, msp_bytes=4096,
        negative_tests=negative, hardware='OPEN', hardware_admitted=False,
        cold_whole_path_timing_equivalence=False)


if __name__ == '__main__':
    args = sys.argv[1:]
    self_test = args[:1] == ['--self-test']
    if self_test: args = args[1:]
    assert len(args) == 1, '[--self-test] ELF'
    print(json.dumps(check(args[0], self_test), indent=2))

#!/usr/bin/env python3
"""Negative BUILD checks on an actual opt-in ELF. No deployment or HW claims."""
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile

assert __debug__ and len(sys.argv) == 2
spec = importlib.util.spec_from_file_location('foundation', Path(__file__).with_name('check-freertos-elf.py'))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
path = Path(sys.argv[1])
data = path.read_bytes()
assert base.check(path, True)['status'] == 'PASS'
try:
    base.check(path)
except AssertionError:
    pass
else:
    raise AssertionError('default validator admitted local stack')
h = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
null = [h[5]+i*h[9] for i in range(h[10]) if struct.unpack_from('<I', data, h[5]+i*h[9])[0] == 0]
assert len(null) == 1
with tempfile.TemporaryDirectory(prefix='rp1-local-stack-negative-', dir='/dev/shm') as tmp:
    target = Path(tmp)/'changed.elf'
    for field, value in [(0,1), (1,1), (2,0x100037f8), (3,0x10003808), (4,8), (5,2040), (6,7), (7,4)]:
        changed = bytearray(data)
        struct.pack_into('<I', changed, null[0]+4*field, value)
        target.write_bytes(changed)
        try:
            base.check(target, True)
        except AssertionError:
            pass
        else:
            raise AssertionError(f'changed PT_NULL field {field} admitted')
assert path.read_bytes() == data
print(json.dumps(dict(classification='BUILD', positive=1, negative=9, hardware=False)))

#!/usr/bin/env python3
"""Reject profile confusion and mutated two-task contracts in the actual ELF."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile

spec = importlib.util.spec_from_file_location('admission', Path(__file__).with_name('check-freertos-elf.py'))
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
image = Path(sys.argv[1])
kwargs = dict(require_scmi=True, require_ddr_diagnostics=True, amp_control=True)
m.check(image, **kwargs)
try: m.check(image)
except AssertionError: pass
else: raise AssertionError('AMP service admitted as legacy R1')
raw = image.read_bytes(); h = struct.unpack_from('<16sHHIIIIIHHHHHH', raw)
sections = [struct.unpack_from('<10I', raw, h[6]+i*h[11]) for i in range(h[12])]
strings = sections[h[13]]; names = raw[strings[4]:strings[4]+strings[5]]
slot = next(i for i,s in enumerate(sections) if names[s[0]:].split(b'\0',1)[0] == b'.amp_control_contract')
contract = sections[slot]
mutations = [(contract[4]+i*4, 0 if i != 5 else 1) for i in range(6)]
mutations += [(h[6]+slot*h[11]+field*4, value) for field,value in
              [(1,8),(2,0),(2,3),(2,6),(2,0x400002),(3,contract[3]+4),
               (3,0x2000d7c0),(4,contract[4]+4),(5,20),(5,28)]]
with tempfile.TemporaryDirectory(prefix='rp1-amp-control-negative-') as d:
    bad_path = Path(d)/'bad.elf'
    for offset,value in mutations:
        bad = bytearray(raw); struct.pack_into('<I',bad,offset,value); bad_path.write_bytes(bad)
        try: m.check(bad_path, **kwargs)
        except (AssertionError, ValueError): pass
        else: raise AssertionError(f'mutated AMP contract admitted at {offset:#x}')
print(f'HOST_PASS: explicit AMP service, legacy profile rejection, {len(mutations)} rejected contract mutations; no hardware')

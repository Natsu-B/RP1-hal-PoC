#!/usr/bin/env python3
"""Mutate the actual linked ELF; reject unallocated/overlapping diagnostic code."""
import argparse
import importlib.util
from pathlib import Path
import struct
import tempfile

spec = importlib.util.spec_from_file_location('admission',Path(__file__).with_name('check-freertos-elf.py'))
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('elf', type=Path)
ap.add_argument('--amp-control', action='store_true')
ap.add_argument('--amp-proc1', action='store_true')
ap.add_argument('--amp-proc1-gpio-irq', action='store_true')
ap.add_argument('--amp-observation', action='store_true')
args = ap.parse_args()
kwargs = dict(require_scmi=True, require_ddr_diagnostics=True, amp_control=args.amp_control,amp_proc1=args.amp_proc1,amp_proc1_gpio_irq=args.amp_proc1_gpio_irq,amp_observation=args.amp_observation)
image = args.elf; m.check(image, **kwargs)
raw = image.read_bytes(); h = struct.unpack_from('<16sHHIIIIIHHHHHH',raw)
sections = [struct.unpack_from('<10I',raw,h[6]+i*h[11]) for i in range(h[12])]
strings = sections[h[13]]; names = raw[strings[4]:strings[4]+strings[5]]
slot = next(i for i,s in enumerate(sections) if names[s[0]:].split(b'\0',1)[0] == b'.ddr_diagnostics_text')
with tempfile.TemporaryDirectory() as d:
    for field,value in [(1,8),(2,3),(3,0x2000d7c0),(5,0),(5,0x600)]:
        bad = bytearray(raw); struct.pack_into('<I',bad,h[6]+slot*h[11]+field*4,value)
        p = Path(d)/f'bad-{field}-{value}.elf'; p.write_bytes(bad)
        try: m.check(p, **kwargs)
        except AssertionError: pass
        else: raise AssertionError('changed diagnostic section admitted')
print('HOST_PASS: actual ELF and five rejected diagnostic section mutations')

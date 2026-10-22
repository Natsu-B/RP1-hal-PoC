#!/usr/bin/env python3
"""Mutate vector bytes in temporary ELF copies; never alter the selected image."""
import argparse
import importlib.util
from pathlib import Path
import struct
import tempfile

spec = importlib.util.spec_from_file_location('validator', Path(__file__).with_name('check-freertos-elf.py'))
v = importlib.util.module_from_spec(spec); spec.loader.exec_module(v)
ap = argparse.ArgumentParser(description=__doc__)
ap.add_argument('elf', type=Path)
ap.add_argument('--amp-control', action='store_true')
ap.add_argument('--amp-proc1', action='store_true')
ap.add_argument('--amp-proc1-gpio-irq', action='store_true')
ap.add_argument('--amp-observation', action='store_true')
args = ap.parse_args()
source = args.elf
kwargs = dict(amp_control=args.amp_control,amp_proc1=args.amp_proc1,amp_proc1_gpio_irq=args.amp_proc1_gpio_irq,amp_observation=args.amp_observation)
result = v.check(source, **kwargs)
data = source.read_bytes()
header = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
phoff, phsize, phcount = header[5], header[9], header[10]
vector_offset = next(struct.unpack_from('<8I',data,phoff+i*phsize)[1] for i in range(phcount)
    if struct.unpack_from('<8I',data,phoff+i*phsize)[0] == 1)
with tempfile.TemporaryDirectory(prefix='rp1-rtos-elf-negative-') as directory:
    mutated = Path(directory) / 'mutated.elf'
    offsets = [vector_offset + index * 4 for index in [0]+result['checked_vector_indices']]
    if result.get('proc1'):
        def file_offset(va):
            for i in range(phcount):
                kind, off, address, _, size, *_ = struct.unpack_from('<8I',data,phoff+i*phsize)
                if kind == 1 and address <= va < address + size: return off + va - address
            raise AssertionError('address not in file-backed load')
        offsets += [file_offset(result['proc1']['vectors']) + 4*i for i in [0,1,79]]
        offsets += [file_offset(result['proc1']['body_entry'] & ~1)]
    for offset in offsets:
        bad = bytearray(data); struct.pack_into('<I',bad,offset,0)
        mutated.write_bytes(bad)
        try: v.check(mutated, **kwargs)
        except AssertionError: pass
        else: raise AssertionError(f'accepted corrupted ELF at {offset}')
print(f"BUILD host check: positive1, negative{len(offsets)}; no hardware")

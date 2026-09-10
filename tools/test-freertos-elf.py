#!/usr/bin/env python3
"""Mutate vector bytes in temporary ELF copies; never alter the selected image."""
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile

spec = importlib.util.spec_from_file_location('validator', Path(__file__).with_name('check-freertos-elf.py'))
v = importlib.util.module_from_spec(spec); spec.loader.exec_module(v)
source = Path(sys.argv[1]); result = v.check(source)
data = source.read_bytes()
header = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
phoff, phsize, phcount = header[5], header[9], header[10]
vector_offset = next(struct.unpack_from('<8I',data,phoff+i*phsize)[1] for i in range(phcount)
    if struct.unpack_from('<8I',data,phoff+i*phsize)[0] == 1)
with tempfile.TemporaryDirectory(prefix='rp1-rtos-elf-negative-') as directory:
    mutated = Path(directory) / 'mutated.elf'
    for index in [0]+result['checked_vector_indices']:
        bad = bytearray(data); struct.pack_into('<I',bad,vector_offset+index*4,0)
        mutated.write_bytes(bad)
        try: v.check(mutated)
        except AssertionError: pass
        else: raise AssertionError(f'accepted broken vector {index}')
print(f"BUILD host check: positive1, negative{1+len(result['checked_vector_indices'])}; no hardware")

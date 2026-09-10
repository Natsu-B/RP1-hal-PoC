#!/usr/bin/env python3
"""Reject an unsafe R1 memory/vector/link result. No third-party Python modules."""
import json
from pathlib import Path
import re
import struct
import subprocess
import sys

if not __debug__:
    raise SystemExit('ELF admission requires assertions enabled; refuse -O/PYTHONOPTIMIZE')


def check(path):
    data = Path(path).read_bytes()
    assert data[:7] == b'\x7fELF\x01\x01\x01', 'ELF32 little endian required'
    header = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
    assert header[2] == 40, 'ARM ELF required'
    entry, phoff, phsize, phcount = header[4], header[5], header[9], header[10]
    loads = []
    for index in range(phcount):
        kind, off, va, pa, filesz, memsz, flags, align = struct.unpack_from('<8I', data, phoff + index * phsize)
        if kind != 1:
            continue
        assert va == pa, 'loader copy contract requires VMA == p_paddr'
        assert 0x20000000 <= pa <= pa + memsz <= 0x2000e000, 'load overlaps reserved SRAM'
        assert filesz <= memsz and off + filesz <= len(data)
        loads.append((pa, pa + memsz, off, filesz))
    loads.sort()
    assert loads and loads[0][0] == 0x20000000
    assert all(a[1] <= b[0] for a, b in zip(loads, loads[1:])), 'overlapping PT_LOAD'
    assert any(a <= (entry & ~1) < b for a, b, _, _ in loads) and entry & 1
    vector = struct.unpack_from('<80I', data, loads[0][2])
    assert vector[0] == 0x2000f000, 'VTOR[0] must equal reserved MSP top'
    nm = subprocess.check_output(['arm-none-eabi-nm', '-n', str(path)], text=True)
    symbols = {}
    for line in nm.splitlines():
        fields = line.split()
        if len(fields) == 3:
            symbols[fields[2]] = int(fields[0], 16)
    required_vectors = {1:'Reset', **{n:'RP1RtosFault' for n in range(2, 7)},
                        11:'vPortSVCHandler', 14:'xPortPendSVHandler', 15:'xPortSysTickHandler'}
    for index, name in {42:'TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler',
                        35:'SPI0_IRQHandler', 24:'I2C1_IRQHandler'}.items():
        if name in symbols: required_vectors[index] = name
    for index, name in required_vectors.items():
        assert vector[index] == symbols[name] | 1, f'vector {index} must point directly to {name}'
    assert symbols['__ebss'] <= 0x2000e000
    assert not subprocess.check_output(['arm-none-eabi-nm', '-u', str(path)]), 'undefined symbols'
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', '-M', 'reg-names-raw', str(path)], text=True)
    assert not re.search(r'\b(?:ldrex\w*|strex\w*|clrex)\b', dis), 'unproven exclusive instruction linked'
    assert not any(name in symbols for name in ['pvPortMalloc', 'vPortFree', 'malloc', '_Unwind_Resume'])
    # The separate assembly loops must retain literal R4-R11 comparisons and no calls.
    spin = [block for block in re.split(r'\n(?=[0-9a-f]+ <)', dis) if 'freertos_r1' in block.splitlines()[0] and '4spin' in block.splitlines()[0]]
    assert len(spin) == 1 and not re.search(r'\bblx?\s', spin[0]), 'non-yielding spin task missing/contains call'
    for register in range(4, 12):
        assert re.search(rf'cmp(?:\.w)?\s+r{register},', spin[0]), f'r{register} test optimized away'
    return {'classification':'BUILD', 'status':'PASS', 'entry':hex(entry),
            'pt_load_end':hex(max(row[1] for row in loads)), 'msp':[hex(0x2000e000),hex(vector[0])],
            'bss_bytes':symbols['__ebss']-symbols['__sbss'], 'direct_rtos_vectors':True,
            'checked_vector_indices': sorted(required_vectors),
            'exclusive_instructions':0, 'hardware':'OPEN'}


if __name__ == '__main__':
    print(json.dumps(check(sys.argv[1]), indent=2))

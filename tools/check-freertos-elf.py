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
                        35:'SPI0_IRQHandler', 24:'I2C1_IRQHandler', 41:'UART0_IRQHandler'}.items():
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
    proc1 = None
    if 'Proc1RuntimeEntry' in symbols:
        names = ['text', 'vectors', 'data', 'bss', 'lifecycle', 'request', 'response', 'fault']
        regions = [(symbols[f'__proc1_{n}_start'], symbols[f'__proc1_{n}_end']) for n in names]
        stack = (symbols['__proc1_guard_low'], symbols['__proc1_stack_end'])
        assert all(a <= b for a, b in regions)
        assert all(a[1] <= b[0] for a, b in zip(regions, regions[1:] + [stack]))
        assert stack[1] <= symbols['__sbss'] <= symbols['__ebss'] <= 0x2000e000
        assert stack[1] - stack[0] == 2064
        assert symbols['__proc1_stack_top'] - symbols['__proc1_stack_low'] == 2048
        assert symbols['__proc1_stack_low'] % 8 == 0
        va = symbols['__proc1_vectors_start']
        assert va % 512 == 0 and regions[1][1] - va == 320
        contents = [struct.unpack_from('<80I', data, off + va - a)
                    for a, _, off, size in loads if a <= va and va + 320 <= a + size]
        assert len(contents) == 1
        assert contents[0][0] == symbols['__proc1_stack_top']
        assert all(v == symbols['Proc1RuntimeFault'] | 1 for v in contents[0][1:])
        # Proc1 owns no RTOS state. Its worker must not call any external routine
        # (including panic/atomic helpers or the proc0 FreeRTOS bridge).
        worker_dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', '-j', '.proc1_text', str(path)], text=True)
        assert not re.search(r'\bblx?\s', worker_dis), 'proc1 worker contains an external call'
        for line in worker_dis.splitlines():
            branch = re.search(r'\s(?:b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(?:\.[nw])?|cbnz|cbz)\s+(?:r\d+,\s*)?([0-9a-f]+)\s+<', line)
            if branch:
                assert regions[0][0] <= int(branch[1], 16) < regions[0][1], 'proc1 tail branch leaves owned code'
        # This pinned build pushes r7/lr and reserves20 bytes before its MSP
        # snapshot. Refuse a prologue change until explicitly reviewed.
        body = worker_dis.split('<Proc1RuntimeBody>:', 1)[1].strip().splitlines()
        assert re.search(r'push\s+\{r7, lr\}', body[0]), 'proc1 body push changed'
        assert re.search(r'sub\s+sp, #20\b', body[2]), 'proc1 body reservation changed'
        assert re.search(r'mrs\s+r0, MSP', body[4]), 'proc1 MSP snapshot moved'
        proc1 = {'sections': dict(zip(names, [[hex(a), hex(b)] for a, b in regions])),
                 'guarded_stack': [hex(v) for v in stack], 'stack_bytes': 2048,
                 'stack_low': symbols['__proc1_stack_low'], 'stack_top': symbols['__proc1_stack_top'],
                 'body_msp': symbols['__proc1_stack_top'] - 28, 'body_entry': symbols['Proc1RuntimeBody'],
                 'vectors': va, 'encoded_entry': (symbols['Proc1RuntimeEntry'] | 1) ^ 0x4ff83f2d,
                 'proc0_vtor': 0x20000000, 'proc0_stack_floor': 0x2000e000,
                 'global_bss_clear_excludes_proc1': True, 'external_calls': 0}
    return {'classification':'BUILD', 'status':'PASS', 'entry':hex(entry),
            'pt_load_end':hex(max(row[1] for row in loads)), 'msp':[hex(0x2000e000),hex(vector[0])],
            'bss_bytes':symbols['__ebss']-symbols['__sbss'], 'direct_rtos_vectors':True,
            'checked_vector_indices': sorted(required_vectors),
            'exclusive_instructions':0, 'proc1':proc1, 'hardware':'OPEN'}


if __name__ == '__main__':
    print(json.dumps(check(sys.argv[1]), indent=2))

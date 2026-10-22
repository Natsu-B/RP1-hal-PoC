#!/usr/bin/env python3
"""Fail-closed codegen check for the pinned opt-s/fat-LTO UART acceptance ELF.

The manually verified Rust1.96 layout has Context generation/terminal/error at
Driver+236/+160/+268. This is a selected-build regression, NOT a stable Rust ABI
or general alias-safety proof. Layout/compiler changes require renewed review.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

if not __debug__:
    raise SystemExit('assertions must be enabled for codegen admission')


def check(elf):
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', str(elf)], text=True)
    blocks = re.split(r'\n(?=[0-9a-f]+ <)', dis)
    matches = [b for b in blocks if 'uart06Driver8transmit' in b.splitlines()[0]]
    assert len(matches) == 1, 'expected exactly one out-of-line transmit'
    rows = []
    for line in matches[0].splitlines()[1:]:
        m = re.match(r'([0-9a-f]+):\s+(?:[0-9a-f]{4}\s+)+\s*(\S+)\s+(.*)', line)
        if m: rows.append((int(m[1], 16), m[2], m[3]))
    receivers = [re.match(r'(r\d+), r0$', a)[1] for _, op, a in rows
                 if op == 'mov' and re.match(r'(r\d+), r0$', a)]
    assert receivers, 'context base not recognized'
    base = receivers[0]
    tick = next(pc for pc, op, arg in rows if op == 'bl' and '4tick' in arg)
    # The outer loop returns to the IRQ-mask/tick prefix. All these backedges
    # must revisit the three FIRST context reads, never cached entry values.
    edges = [(pc, int(arg.split()[0], 16)) for pc, op, arg in rows
             if re.fullmatch(r'b(?:eq|ne|cs|cc|hi|ls|ge|lt|gt|le|pl|mi)?(?:\.[nw])?', op)
             and re.match(r'[0-9a-f]+ ', arg)
             and int(arg.split()[0], 16) <= tick < pc]
    assert edges, 'outer loop backedge not recognized'
    loads = {}
    for offset in (236, 160, 268):
        pcs = [pc for pc, op, arg in rows if op.startswith('ldr')
               and f'[{base}, #{offset}]' in arg]
        assert pcs, f'context offset {offset} missing'
        loads[offset] = pcs[0]
        assert all(target <= pcs[0] < pc for pc, target in edges), f'cached context offset {offset}'
    return dict(status='PASS', classification='BUILD', elf_sha256=hashlib.sha256(elf.read_bytes()).hexdigest(),
                context_base=base, first_loads={k:hex(v) for k,v in loads.items()},
                outer_backedges=[[hex(pc),hex(target)] for pc,target in edges])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf', type=Path)
    args = parser.parse_args()
    print(json.dumps(check(args.elf), indent=2))

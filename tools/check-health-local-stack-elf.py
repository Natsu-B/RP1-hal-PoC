#!/usr/bin/env python3
"""Exact BE06 BUILD review; not observer admission, timing, or hardware proof."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

assert __debug__, 'assertions required'
SHA = 'cc693d4008c275cdeb634f68a9d95a31c312d594464f863215a99e8f5b8c1f9c'

def identity(data):
    assert hashlib.sha256(data).hexdigest() == SHA, 'new image requires compiled review'

def check(path, self_test=False):
    data = Path(path).read_bytes()
    identity(data)
    spec = importlib.util.spec_from_file_location('foundation', Path(__file__).with_name('check-freertos-elf.py'))
    base = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(base)
    result = base.check(path, True)
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', str(path)], text=True)
    ins = {}
    for line in dis.splitlines():
        m = re.match(r'^([0-9a-f]+):\s+((?:[0-9a-f]{4,8}\s+)+)\s*(\S+)\s*(.*)$', line)
        if m:
            ins[int(m[1],16)] = (sum(len(x)//2 for x in m[2].split()), m[3], m[4])
    # The new cold masked interval is NOT byte-identical to BC12. This fixed
    # build has one out-of-line refusal branch, and PRIMASK uses r10 + spill28.
    pending, seen, edges = [0x20004820], set(), []
    while pending:
        address = pending.pop()
        if address in seen:
            continue
        seen.add(address)
        size, op, arg = ins[address]
        if address == 0x200048d2:
            assert (op,arg) == ('msr','PRIMASK, sl')
            continue
        assert op not in ('bl','blx','bx','pop','tbb','tbh')
        if re.fullmatch(r'b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(?:\.[nw])?', op):
            to = int(arg.split()[0],16)
            successors = [to] if op in ('b','b.n','b.w') else [to,address+size]
        else:
            successors = [address+size]
        for to in successors:
            assert 0x20004820 <= to < 0x20004984
            edges.append((address,to))
            pending.append(to)
    assert len(seen) == 70 and 0x200048d2 in seen
    assert sorted((a,b) for a,b in edges if b <= a) == [(0x20004890,0x20004880),(0x20004982,0x200048ce)]
    for address, op, arg in [
        (0x20004820,'mrs','sl, PRIMASK'),
        (0x20004866,'str.w','sl, [sp, #28]'),
        (0x200048c2,'ldr.w','sl, [sp, #28]'),
        (0x20004886,'add.w','ip, ip, #1'),
        (0x20004b24,'.word','0x000186a0'),  # 100000-iteration timer-failure bound
    ]:
        assert ins[address][1:] == (op,arg)
    negative = 0
    if self_test:
        for at in (0,len(data)//2,len(data)-1):
            changed = bytearray(data)
            changed[at] ^= 1
            try:
                identity(changed)
            except AssertionError:
                negative += 1
            else:
                raise AssertionError('changed image admitted')
    return dict(classification='BUILD', status='PASS', sha256=SHA, foundation=result,
        cold_masked_reachable_instructions=70, calls_while_masked=0,
        old_primask='r10; saved/restored at task SP+28 on the arm path',
        poll_iteration_bound=100000, negative_tests=negative,
        hardware_admitted=False, hardware='OPEN', observer_admission='OPEN',
        cold_timing_equivalence=False, local_stack_switch_and_fault_test='OPEN')

if __name__ == '__main__':
    args = sys.argv[1:]
    test = args[:1] == ['--self-test']
    if test:
        args = args[1:]
    assert len(args) == 1
    print(json.dumps(check(args[0],test),indent=2))

#!/usr/bin/env python3
"""Conservative C0 scheduler stack gate for an explicitly reviewed ELF.

Count every fixed allocation once, including mutually exclusive paths, then
add the largest callee sum (also for tail calls). Reject allocation loops,
recursion, dynamic SP writes, unknown transfers and unreviewed callbacks.
Include cold startup and reviewed MSP/PSP switches. Runtime high-water and
physical IRQ timing still require hardware evidence.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

ROOTS = {'worker': 'freertos_r114openamp_worker', 'service': 'amp_control7service',
         'systick': 'xPortSysTickHandler', 'scmi': 'RP1_SCMI_IRQHandler',
         'pendsv': 'xPortPendSVHandler', 'svc': 'vPortSVCHandler', 'idle': 'prvIdleTask',
         'boot': 'Reset', 'fault': 'RP1RtosFault'}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def parse(dis, nm):
    sizes = {}
    for line in nm.splitlines():
        cols = line.split()
        if len(cols) == 4 and cols[2].lower() in ('t', 'w') and int(cols[1], 16):
            sizes[int(cols[0], 16)] = (cols[3], int(cols[1], 16))
    nodes = {}
    heads = list(re.finditer(r'^([0-9a-f]+) <([^>]+)>:\n', dis, re.M))
    for i, head in enumerate(heads):
        addr = int(head[1], 16)
        if addr not in sizes:
            continue
        name, size = sizes[addr]
        body = dis[head.end():heads[i+1].start() if i+1 < len(heads) else len(dis)]
        ins = {}
        for line in body.splitlines():
            m = re.match(r'^\s*([0-9a-f]+):\s+(?:[0-9a-f]{4,8}\s+)+(.+)', line)
            if m and int(m[1], 16) < addr + size:
                text = m[2].split('@', 1)[0].strip()
                if not text.startswith('.'):
                    ins[int(m[1], 16)] = text
        nodes[addr] = dict(name=name, size=size, ins=ins)
    return nodes


def find(nodes, part):
    hits = [a for a, n in nodes.items() if n['name'] == part]
    if not hits:
        hits = [a for a, n in nodes.items() if part in n['name']]
    require(len(hits) == 1, 'ambiguous/missing symbol ' + part)
    return hits[0]


def loaded_bytes(data, address, size):
    hd = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
    require(data[:7] == b'\x7fELF\x01\x01\x01' and hd[2] == 40, 'ARM ELF32 LE required')
    for i in range(hd[10]):
        kind, off, va, pa, filesz, _, _, _ = struct.unpack_from('<8I', data, hd[5]+i*hd[9])
        if kind == 1 and va == pa and pa <= address < address+size <= pa+filesz:
            return data[off+address-pa:off+address-pa+size]
    raise ValueError('unloaded address ' + hex(address))


def local_analysis(addr, node, nodes, rules, tables, data, stack_writes=None):
    ins = node['ins']; pcs = sorted(ins); cfg = {}; grows = set(); callees = set(); allocation = 0
    for i, pc in enumerate(pcs):
        text = ins[pc]; rawop, _, args = text.partition('\t'); op = rawop.split('.')[0]
        following = [pcs[i+1]] if i+1 < len(pcs) else []
        successors = following
        increase = 0
        if op == 'msr' and re.match(r'(?:MSP|PSP|CONTROL),', args):
            require((stack_writes or {}).get(pc-addr)==text, 'unreviewed stack register switch '+hex(pc))
        if op == 'push' or (op == 'stmdb' and args.startswith('r13!')):
            increase = 4*len(re.findall(r'r\d+', args[args.index('{'):]))
        elif m := re.search(r'\[(?:sp|r13), #-(\d+)\]!', args):
            require(op.startswith('str'), 'unreviewed stack predecrement ' + text)
            increase = int(m[1])
        elif op in ('sub', 'add') and re.match(r'(?:sp|r13),', args):
            m = re.fullmatch(r'(?:sp|r13),\s*(?:(?:sp|r13),\s*)?#(\d+)', args)
            require(m, 'dynamic stack allocation ' + text)
            if op == 'sub': increase = int(m[1])
        elif re.match(r'(?:sp|r13)(?:,|!)', args):
            require(op.startswith(('str', 'stmia', 'ldmia')), 'unreviewed SP write ' + text)
        if re.search(r'\[(?:sp|r13)[^]]*\]!', args) and not increase:
            require(False, 'unreviewed stack writeback ' + text)
        if increase:
            allocation += increase; grows.add(pc)
        if op == 'blx' or (op == 'bx' and args != 'r14'):
            require(pc-addr in rules, 'unresolved indirect transfer ' + hex(pc))
            callees.update(rules[pc-addr])
            if op == 'bx': successors = []
        elif op == 'bx':
            successors = []
        elif op == 'bl' or re.fullmatch(r'b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?|cbnz|cbz', op):
            m = re.search(r'([0-9a-f]+)\s+<', args)
            require(m, 'unknown branch ' + text); target = int(m[1], 16)
            if op == 'bl':
                callees.add(target)
            else:
                successors = [] if op == 'b' else following
                if addr <= target < addr+node['size']:
                    require(target in ins, 'branch into data ' + hex(pc)); successors += [target]
                else:
                    callees.add(target)
        elif op in ('tbb', 'tbh'):
            require(pc in tables, 'unreviewed table branch ' + hex(pc))
            count = tables[pc]; width = 1 if op == 'tbb' else 2
            table = loaded_bytes(data, pc+4, count*width)
            successors = [pc+4+2*int.from_bytes(table[n:n+width], 'little')
                          for n in range(0, len(table), width)]
            require(all(t in ins for t in successors), 'table branches outside function/into data')
        elif op.startswith(('pop', 'ldmia')) and 'r15' in args:
            successors = following if re.match(r'(?:pop|ldmia)(?:eq|ne)', op) else []
        elif op == 'ldr' and re.fullmatch(r'r15, \[(?:sp|r13)\], #4', args):
            successors = []  # Single-register pop PC, used by the startup path.
        elif re.match(r'r15,', args) and op not in ('str', 'stmia'):
            require(False, 'unreviewed PC write ' + text)
        cfg[pc] = successors
    # A growing instruction cannot execute twice in one invocation. Counting
    # all such sites, rather than selecting paths, deliberately overestimates.
    for grow in grows:
        todo = list(cfg[grow]); seen = set()
        while todo:
            target = todo.pop()
            require(target != grow, 'stack allocation loop ' + hex(grow))
            if target in seen: continue
            seen.add(target); todo.extend(cfg[target])
    require(all(a in nodes for a in callees), 'callee lacks sized symbol')
    return allocation, sorted(callees)


def audit(path, proof, openamp_source):
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == proof['elf_sha256'], 'unreviewed ELF hash')
    repo = Path(__file__).resolve().parents[1]
    for name, expected in proof['sources_sha256'].items():
        prefix, relative = name.split('/', 1)
        source = (repo if prefix == 'hal' else openamp_source) / relative
        require(hashlib.sha256(source.read_bytes()).hexdigest() == expected, 'reviewed source changed: '+name)
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', '-M', 'reg-names-raw', str(path)], text=True)
    nm = subprocess.check_output(['arm-none-eabi-nm', '-S', str(path)], text=True)
    nodes = parse(dis, nm)
    rules = {find(nodes, part): {int(off): [find(nodes, t) for t in ts]
             for off, ts in sites.items()} for part, sites in proof['indirect_targets'].items()}
    tables = {int(pc, 16): count for pc, count in proof['table_entries'].items()}
    switches = {find(nodes, part): {int(off): text for off,text in sites.items()}
                for part,sites in proof['stack_register_writes'].items()}
    require(loaded_bytes(data, 0x20000000, 4)==struct.pack('<I',0x2000f000), 'cold initial MSP contract')
    # Reset and the first-task trampoline are naked. Their reviewed literal
    # loads establish F000 before Rust and before SVC; do not infer a dynamic
    # MSP pivot from a register name alone.
    for part,review in proof['stack_pivots'].items():
        a=find(nodes,part)
        require(hashlib.sha256(loaded_bytes(data,a,review['bytes'])).hexdigest()==review['sha256'],
                'stack pivot instructions/literals changed: '+part)
    local = {}; totals = {}
    def bound(a, active):
        require(a not in active, 'recursive call graph')
        if a in totals: return totals[a]
        if a not in local:
            local[a] = local_analysis(a, nodes[a], nodes, rules.get(a, {}), tables, data, switches.get(a, {}))
        frame, children = local[a]
        largest, chain = max((bound(c, active+[a]) for c in children), default=(0, []), key=lambda x: x[0])
        totals[a] = frame+largest, [nodes[a]['name']]+chain
        return totals[a]
    result = {}
    for name, part in ROOTS.items():
        value, chain = bound(find(nodes, part), [])
        result[name] = dict(bound_bytes=value, path=chain)
    # ARMv7-M basic frame32 + alignment4 + PendSV r4-r11=32. Nested handler
    # frames live on MSP; PendSV's PSP save is accounted for separately here.
    for name, size in [('worker', 2048), ('service', 2048), ('idle', 512)]:
        row = result[name]; row['psp_bound_bytes'] = row['bound_bytes']+68+8
        row['remaining_bytes'] = size-row['psp_bound_bytes']
        require(row['remaining_bytes'] >= 128, 'insufficient task stack reserve: '+name)
    # Sum all handler paths and two fault/NMI frames, regardless of priorities;
    # this overestimates the reviewed IRQ57 plus kernel exception nesting.
    msp = sum(result[n]['bound_bytes']+36 for n in ('scmi','systick','pendsv','svc'))+72
    require(msp+128 <= 4096, 'insufficient runtime MSP reserve')
    require(result['fault']['bound_bytes']==0, 'fault handler must remain stackless')
    startup=result['boot']['bound_bytes']
    combined=startup+msp
    require(combined+128<=4096, 'insufficient startup MSP reserve')
    require(loaded_bytes(data, 0x2000b9e0, 24) == struct.pack('<6I', int.from_bytes(b'AG01','little'),1,2,1024,200,1), 'AG01 stack contract')
    return dict(status='PASS_STATIC_PINNED_SCHEDULER_STACK', elf_sha256=proof['elf_sha256'],
                roots=result, runtime_msp_bound_bytes=msp, functions=len(local),
                startup_msp_bound_bytes=startup, conservative_combined_msp_bound_bytes=combined,
                msp_remaining_bytes=4096-combined,
                assumptions=['cold loader enters with vector MSP F000 and owned VTOR',
                    'C0 IRQ admission rejects enabled/active external IRQs before granting IRQ57',
                    'fault/NMI handler remains stackless; no recovery/re-entry admission'],
                limitations=['runtime high-water and physical IRQ timing NOT_TESTED'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf', type=Path)
    parser.add_argument('--review', type=Path, default=Path(__file__).with_name('amp-observation-stack-review.json'))
    parser.add_argument('--openamp-source', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = audit(args.elf, json.loads(args.review.read_text()), args.openamp_source)
    except (ValueError, OSError, KeyError, struct.error) as error:
        print(json.dumps(dict(status='REFUSED', error=str(error))))
        raise SystemExit(1)
    print(json.dumps(result, indent=2))


if __name__ == '__main__': main()

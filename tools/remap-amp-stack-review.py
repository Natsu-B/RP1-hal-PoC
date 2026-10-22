#!/usr/bin/env python3
"""Derive a C0 stack review for a relocated diagnostic build (C1 profiler).

The reviewed proof (tools/amp-observation-stack-review.json) pins the sealed X
ELF: absolute jump-table PCs and stack-pivot bytes. A profiler build moves code
but must not change C0 code. This tool carries the review over only when that
holds: each table entry maps to the same function+offset with identical table
bytes, and each stack pivot differs only in literal words or direct branches
that point at the same-named symbol. Anything else refuses; no review is written.
"""
import argparse
import hashlib
import json
import re
import struct
import subprocess
from pathlib import Path


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


def dehash(name):
    return re.sub(r'17h[0-9a-f]{16}E', '17hE', name)


def symbols(elf):
    out = subprocess.check_output(['arm-none-eabi-nm', '-S', str(elf)], text=True)
    funcs, by_addr = [], {}
    for line in out.splitlines():
        cols = line.split()
        if len(cols) == 4 and cols[2].lower() in ('t', 'w'):
            addr, size = int(cols[0], 16) & ~1, int(cols[1], 16)
            funcs.append((addr, size, cols[3]))
        if len(cols) >= 3:
            by_addr.setdefault(int(cols[0], 16) & ~1, set()).add(dehash(cols[-1]))
    return funcs, by_addr


def loaded(data, address, size):
    hd = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
    for i in range(hd[10]):
        kind, off, va, pa, filesz, _, _, _ = struct.unpack_from('<8I', data, hd[5] + i * hd[9])
        if kind == 1 and va == pa and pa <= address < address + size <= pa + filesz:
            return data[off + address - pa:off + address - pa + size]
    raise ValueError('unloaded address ' + hex(address))


def pivot_rows(elf, address, size):
    text = subprocess.check_output(['arm-none-eabi-objdump', '-d',
        f'--start-address={address}', f'--stop-address={address + size}', str(elf)], text=True)
    rows = {int(m[1], 16) - address: (m[2].split(), m[3], m[4].strip())
            for m in re.finditer(r'^[ \t]*([0-9a-f]+):[ \t]+((?:[0-9a-f]{4,8}[ \t]+)+)[ \t]*(\S+)[ \t]*(.*)$', text, re.M)}
    cursor = 0
    for offset, (code, _, _) in rows.items():
        require(offset == cursor, 'pivot disassembly has a gap or overlap')
        cursor += sum(len(word) // 2 for word in code)
    require(cursor == size, 'pivot disassembly does not cover the reviewed bytes')
    return rows


def check_pivot(old, new, ref_names, cand_names, part):
    require(old.keys() == new.keys(), 'pivot instruction layout changed: ' + part)
    notes = []
    for offset, (code, op, operand) in old.items():
        nc, no, na = new[offset]
        if code == nc:
            continue
        require(op == no and len(code) == len(nc), f'pivot {part} opcode changed at +{offset}')
        if op in ('b.w', 'bl'):
            a, b = int(operand.split()[0], 16), int(na.split()[0], 16)
        elif op == '.word':
            a, b = int(operand, 16), int(na, 16)
            require(a & 1 == b & 1, 'pivot literal Thumb state changed')
        else:
            raise ValueError(f'pivot {part} changed beyond relocation at +{offset}')
        same = ref_names.get(a & ~1, set()) & cand_names.get(b & ~1, set())
        require(same, f'pivot {part} target changed at +{offset}')
        notes.append(f'pivot {part}+{offset}: 0x{a:08x} -> 0x{b:08x} ({sorted(same)[0]})')
    return notes


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--reference-elf', type=Path, required=True)
    ap.add_argument('--review', type=Path, required=True)
    ap.add_argument('--candidate-elf', type=Path, required=True)
    ap.add_argument('--hal', type=Path, required=True)
    ap.add_argument('--openamp-source', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    a = ap.parse_args()
    review = json.loads(a.review.read_text())
    ref_data, cand_data = a.reference_elf.read_bytes(), a.candidate_elf.read_bytes()
    require(hashlib.sha256(ref_data).hexdigest() == review['elf_sha256'], 'review does not belong to reference ELF')
    ref_funcs, ref_names = symbols(a.reference_elf)
    cand_funcs, cand_names = symbols(a.candidate_elf)
    notes = []

    # Jump tables: absolute PCs -> same function, same offset, identical table bytes.
    tables = {}
    for pc_text, count in review['table_entries'].items():
        pc = int(pc_text, 16)
        owner = [(s, z, n) for s, z, n in ref_funcs if s <= pc < s + z]
        require(len(owner) == 1, 'table PC not in one reference function ' + pc_text)
        start, size, name = owner[0]
        off = pc - start
        span = 4 + 2 * count + 2  # tbb/tbh instruction plus its table (upper bound)
        span = min(span, size - off)
        want = loaded(ref_data, pc, span)
        hits = [s + off for s, z, n in cand_funcs if dehash(n) == dehash(name) and z == size
                and loaded(cand_data, s + off, span) == want]
        require(len(set(hits)) == 1, 'table function moved or changed: ' + name)
        tables[f'0x{hits[0]:08x}'] = count
        notes.append(f'table {pc_text} -> 0x{hits[0]:08x} ({dehash(name)}+{off})')

    # Decode instructions so a relocated B.W cannot be mistaken for a literal.
    pivots = {}
    for part, pin in review['stack_pivots'].items():
        ra = [s for s, z, n in ref_funcs if n == part]
        ca = [s for s, z, n in cand_funcs if n == part]
        require(len(ra) == 1 and len(ca) == 1, 'pivot symbol ' + part)
        old, new = loaded(ref_data, ra[0], pin['bytes']), loaded(cand_data, ca[0], pin['bytes'])
        require(hashlib.sha256(old).hexdigest() == pin['sha256'], 'reference pivot mismatch ' + part)
        notes.extend(check_pivot(pivot_rows(a.reference_elf, ra[0], pin['bytes']),
                                 pivot_rows(a.candidate_elf, ca[0], pin['bytes']),
                                 ref_names, cand_names, part))
        pivots[part] = dict(bytes=pin['bytes'], sha256=hashlib.sha256(new).hexdigest())

    sources = {}
    for name, digest in review['sources_sha256'].items():
        prefix, rel = name.split('/', 1)
        path = (a.hal if prefix == 'hal' else a.openamp_source) / rel
        now = hashlib.sha256(path.read_bytes()).hexdigest()
        require(now == digest, 'unexpected reviewed source change ' + name)
        sources[name] = now

    out = dict(review, elf_sha256=hashlib.sha256(cand_data).hexdigest(), sources_sha256=sources,
               table_entries=tables, stack_pivots=pivots,
               derived_from=dict(review_sha256=hashlib.sha256(a.review.read_bytes()).hexdigest(),
                                 reference_elf_sha256=review['elf_sha256'], relocation_notes=notes,
                                 scope='relocation-equivalence only; diagnostic profiler build'))
    a.output.write_text(json.dumps(out, indent=1) + '\n')
    print(json.dumps(dict(status='PASS_RELOCATED_REVIEW', notes=len(notes))))


if __name__ == '__main__':
    main()

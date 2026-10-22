#!/usr/bin/env python3
"""Ensure the diagnostic exception rejects unsafe mutations of the built ELF."""
import argparse
import importlib.util
import os
from pathlib import Path
import re
import struct
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf', type=Path)
    args = parser.parse_args()
    for key in ('RP1_AMP_PROC1_PROFILE', 'RP1_AMP_PROC1_LOCAL_STACK', 'RP1_AMP_PROC1_NO_HOOKS'):
        os.environ[key] = '1'
    if not os.environ.get('RP1_AMP_PROC1_PROFILE_REFERENCE'):
        parser.error('set RP1_AMP_PROC1_PROFILE_REFERENCE to the sealed X reference')
    spec = importlib.util.spec_from_file_location('checker', Path(__file__).with_name('check-freertos-elf.py'))
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    kwargs = dict(require_scmi=True, require_ddr_diagnostics=True, amp_control=True,
                  amp_proc1=True, amp_proc1_gpio_irq=True, amp_observation=True)
    checker.check(args.elf, **kwargs)
    raw = args.elf.read_bytes()
    header = struct.unpack_from('<16sHHIIIIIHHHHHH', raw)
    loads = [struct.unpack_from('<8I', raw, header[5] + i * header[9])
             for i in range(header[10])]

    def offset(address):
        return next(off + address - va for kind, off, va, _, size, *_ in loads
                    if kind == 1 and va <= address < va + size)

    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', '-M', 'reg-names-raw',
                                   str(args.elf)], text=True)

    def block(name):
        return re.search(r'^[0-9a-f]+ <[^>]*' + name + r'[^>]*>:\n.*?(?=^[0-9a-f]+ <|\Z)',
                         dis, re.M | re.S)[0]

    def instruction(text, pattern):
        return int(re.search(r'^\s*([0-9a-f]+):\s+(?:[0-9a-f]{4}\s+)+' + pattern,
                             text, re.M)[1], 16)

    c0, c1 = block('13lib_probes_c0'), block('10lib_probes17h')
    nop32 = bytes.fromhex('00bf00bf')
    ldrex = instruction(c0, r'ldrex\b')
    mutations = [('C0 LDREX removed', ldrex, nop32),
                 ('C0 interrupt-mask restore removed', instruction(c0, r'msr\s+PRIMASK'), nop32),
                 ('C1 DONE guard removed', instruction(c1, r'bpl\.w\b'), nop32),
                 ('exclusive outside diagnostic functions',
                  int(block('AmpProc1Entry').split()[0], 16), raw[offset(ldrex):offset(ldrex)+4])]
    for label, old, new in [('reset CLR changed to SET', 0x40017000, 0x40016000),
                            ('EXAC7 changed to EXAC0', 0x40400380, 0x40400000)]:
        address = int(re.search(r'^\s*([0-9a-f]+):\s+[0-9a-f]{8}\s+\.word\s+0x' +
                                f'{old:08x}', c1, re.M)[1], 16)
        mutations.append((label, address, struct.pack('<I', new)))
    with tempfile.TemporaryDirectory(prefix='exac-elf-negative-') as directory:
        candidate = Path(directory) / 'mutated.elf'
        for label, address, replacement in mutations:
            data = bytearray(raw)
            start = offset(address)
            data[start:start+len(replacement)] = replacement
            candidate.write_bytes(data)
            try:
                checker.check(candidate, **kwargs)
            except (AssertionError, ValueError):
                print('REJECTED:', label)
            else:
                raise AssertionError('unsafe mutation admitted: ' + label)
    os.environ['RP1_AMP_PROC1_NO_HOOKS'] = '0'
    try:
        checker.check(args.elf, **kwargs)
    except (AssertionError, ValueError):
        print('REJECTED: EXAC instructions without the no-hooks feature')
    else:
        raise AssertionError('production mode admitted diagnostic EXAC instructions')
    print('PASS: actual ELF baseline and seven negative admission cases; no hardware')


if __name__ == '__main__':
    main()

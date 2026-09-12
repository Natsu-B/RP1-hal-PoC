#!/usr/bin/env python3
"""AY structural first-link checks; admission needs a separately reviewed ELF pin."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess

if not __debug__:
    raise SystemExit('ELF admission requires assertions enabled')


def identity(data, expected):
    assert re.fullmatch(r'[0-9a-f]{64}', expected), 'expected SHA256 must be 64 lowercase hex digits'
    assert hashlib.sha256(data).hexdigest() == expected, 'image changed; source/compiled review required'


def structure(disassembly, symbols, sections, reviewed_inlined_reset=False):
    addresses = {name: int(address, 16) for address, name in
                 re.findall(r'^([0-9a-f]+) (?:[0-9a-f]+ )?\w (\S+)$', symbols, re.M)}
    def body(name):
        matches = re.findall(r'^[0-9a-f]+ <' + re.escape(name) + r'>:\n(.*?)(?=^[0-9a-f]+ <|\Z)',
                             disassembly, re.M | re.S)
        assert len(matches) == 1, ('missing/duplicate body', name)
        return matches[0]

    assert not any(name in addresses for name in ('Proc1RuntimeEntry', 'SPI0_IRQHandler', 'UART0_IRQHandler'))
    assert 'I2C1_IRQHandler' in addresses, 'direct I2C1/IRQ8 owner missing'
    assert any('warm_i2c6target6worker' in name for name in addresses), 'warm I2C task missing'
    assert addresses['__data_start'] < addresses['__data_end'] <= addresses['__sbss'] < addresses['__ebss']
    assert addresses['__ebss'] <= addresses['__warm_data_shadow_start'] < addresses['__warm_data_shadow_end']
    assert addresses['__warm_data_shadow_end'] <= addresses['__image_end'] <= addresses['__app_limit'] == 0x2000e000
    assert addresses['_stack_start'] == 0x2000f000
    assert addresses['__warm_data_shadow_start'] % 4 == 0
    assert addresses['__warm_data_shadow_end'] - addresses['__warm_data_shadow_start'] == 12 + addresses['__data_end'] - addresses['__data_start']
    for name, kind, start, end in [('.data', 'PROGBITS', '__data_start', '__data_end'),
                                 ('.bss', 'NOBITS', '__sbss', '__ebss'),
                                 ('.warm_data_shadow', 'NOBITS', '__warm_data_shadow_start', '__warm_data_shadow_end')]:
        rows = re.findall(r'^\s*\[\s*\d+\]\s+' + re.escape(name) + r'\s+(\w+)\s+([0-9a-f]+)\s+[0-9a-f]+\s+([0-9a-f]+)\s', sections, re.M)
        assert len(rows) == 1 and rows[0][0] == kind, ('section type changed', name)
        assert int(rows[0][1], 16) == addresses[start] and int(rows[0][2], 16) == addresses[end] - addresses[start]
    stack = re.findall(r'^([0-9a-f]+) ([0-9a-f]+) [bB] task_stacks$', symbols, re.M)
    assert len(stack) == 1 and int(stack[0][1], 16) == 2304 * 4, 'selected task stack pool changed'
    assert int(stack[0][0], 16) % 8 == 0
    assert addresses['__sbss'] <= int(stack[0][0], 16) < int(stack[0][0], 16) + 2304 * 4 <= addresses['__ebss']
    reset = body('Reset')
    for register in ('CONTROL', 'BASEPRI', 'FAULTMASK', 'MSP'):
        assert re.search(r'\bmsr\s+' + register + r',', reset), ('reset register missing', register)
    assert reset.index('cpsid') < reset.index('MSP') < reset.index('<rp1_freertos_reset>')
    start = body('rp1_freertos_reset')
    if '<rp1_freertos_capture_reset_entry>' in start:
        assert start.index('<rp1_freertos_capture_reset_entry>') < start.index('strb') < start.index('<rp1_freertos_warm_start>')
        capture = body('rp1_freertos_capture_reset_entry')
        assert not re.search(r'\bblx?\s', capture), 'capture external call'
        capture_order = 'outlined structural check'
    else:
        # LTO may inline both capture and warm-data handling. Do not invent a
        # disassembly heuristic: the CLI requires a separately reviewed image pin.
        assert reviewed_inlined_reset, 'inlined reset requires exact-image compiled review'
        capture = start
        capture_order = 'reviewed exact image; not mechanically inferred'
    warm = body('rp1_freertos_warm_start')
    assert not re.search(r'\b(?:cpsie|svc)\b', capture + warm), 'interrupt enable before scheduler'
    assert not re.search(r'\b(?:b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(?:\.[nw])?|bl(?:eq|ne)?|blx)\s+[0-9a-f]+ <Reset>', disassembly), 'software Reset branch'
    return dict(direct_i2c_vector_index=24, static_stack_pool_words=2304,
                msp_bytes=4096, warm_shadow_noload=True, capture_before_bss=capture_order,
                software_reset_branch_present=False, semantic_compiled_review_required=True)


def self_test():
    # Small synthetic fixture exercises the checker, never substitutes for an ELF.
    layout = dict(__data_start=0x20001000, __data_end=0x20001010, __sbss=0x20001010,
                  __ebss=0x20005000, __warm_data_shadow_start=0x20005000,
                  __warm_data_shadow_end=0x2000501c, __image_end=0x20006000,
                  __app_limit=0x2000e000, _stack_start=0x2000f000,
                  I2C1_IRQHandler=0x20000150, warm_i2c6target6worker=0x20000200)
    symbols = ''.join(f'{value:08x} T {name}\n' for name, value in layout.items()) + '20002000 00002400 b task_stacks\n'
    sections = '[ 1] .data PROGBITS 20001000 001000 000010 WA\n[ 2] .bss NOBITS 20001010 001010 003ff0 WA\n[ 3] .warm_data_shadow NOBITS 20005000 005000 00001c WA\n'
    dis = ('20000140 <Reset>:\ncpsid i\nmsr CONTROL, r0\nmsr BASEPRI, r0\nmsr FAULTMASK, r0\nmsr MSP, r0\nb 20000300 <rp1_freertos_reset>\n'
           '20000300 <rp1_freertos_reset>:\nbl 20000400 <rp1_freertos_capture_reset_entry>\nstrb r0, [r1]\nbl 20000500 <rp1_freertos_warm_start>\n'
           '20000400 <rp1_freertos_capture_reset_entry>:\nbx lr\n20000500 <rp1_freertos_warm_start>:\nwfe\n')
    structure(dis, symbols, sections)
    inlined = dis.replace('bl 20000400 <rp1_freertos_capture_reset_entry>\n', 'ldr r0, [r1]\n')
    structure(inlined, symbols, sections, reviewed_inlined_reset=True)
    bad = [(dis.replace(old, new), symbols, sections) for old, new in
           [('msr MSP,', 'msr PSP,'), ('msr CONTROL,', 'msr PSP,'), ('cpsid', 'cpsie'),
            ('strb', 'ldrb'), ('bx lr', 'bl 20000900 <helper>'), ('wfe', 'svc #0'),
            ('capture_reset_entry', 'absent_capture')]]
    bad += [(dis, symbols.replace(old, new), sections) for old, new in
            [('I2C1_IRQHandler', 'SPI0_IRQHandler'), ('warm_i2c6target6worker', 'warm_spi6target6worker'),
             ('2000e000', '2000f000'), ('2000f000', '20010000'), ('00002400', '00002800'),
             ('20002000', '20004000'), ('2000501c', '20005018')]]
    bad += [(dis, symbols, sections.replace('NOBITS', 'PROGBITS')),
            (dis + 'b.w 20000140 <Reset>\n', symbols, sections), (inlined, symbols, sections)]
    for sample in bad:
        try:
            structure(*sample)
        except (AssertionError, ValueError, KeyError):
            pass
        else:
            raise AssertionError('structural mutation accepted')
    expected = hashlib.sha256(b'AY ELF identity test').hexdigest()
    identity(b'AY ELF identity test', expected)
    for data, pin in [(b'changed', expected), (b'AY ELF identity test', 'not-a-hash')]:
        try:
            identity(data, pin)
        except AssertionError:
            pass
        else:
            raise AssertionError('identity mutation accepted')
    return len(bad) + 2


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf', nargs='?', type=Path)
    parser.add_argument('--expected-sha256', help='exact image SHA256 selected only after separate compiled review')
    parser.add_argument('--reviewed-inlined-reset', action='store_true',
                        help='accept independently reviewed inlined reset ordering; requires matching --expected-sha256')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.reviewed_inlined_reset and not (args.elf and args.expected_sha256):
        parser.error('--reviewed-inlined-reset requires ELF and --expected-sha256')
    tests = self_test() if args.self_test else 0
    if args.elf is None:
        if not args.self_test or args.expected_sha256:
            parser.error('ELF required except for standalone --self-test')
        print(json.dumps(dict(classification='HOST', status='PASS', mutation_refusals=tests)))
    else:
        root = Path(__file__).parent
        data = args.elf.read_bytes()
        if args.expected_sha256:
            identity(data, args.expected_sha256)
        foundation = runpy.run_path(str(root / 'check-freertos-elf.py'))['check'](args.elf)
        assert foundation['proc1'] is None and 24 in foundation['checked_vector_indices']
        dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', str(args.elf)], text=True)
        symbols = subprocess.check_output(['arm-none-eabi-nm', '-nS', str(args.elf)], text=True)
        sections = subprocess.check_output(['arm-none-eabi-readelf', '-SW', str(args.elf)], text=True)
        proof = structure(dis, symbols, sections, args.reviewed_inlined_reset)
        watchdog = runpy.run_path(str(root / 'check-freertos-watchdog-elf.py'))['check'](dis)
        print(json.dumps(dict(classification='BUILD', status='PASS' if args.expected_sha256 else 'NOT_ADMITTED',
            elf_sha256=hashlib.sha256(data).hexdigest(), expected_image_matched=bool(args.expected_sha256),
            foundation=foundation, structure=proof, watchdog_probe=watchdog, mutation_refusals=tests,
            hardware='OPEN', runtime_restart_proven=False, warm_i2c_owner_proven=False,
            boundary='Structural/link checks only, not a semantic verifier. No expected pin means NOT ADMITTED. '
                     'A matching caller-supplied pin requires a separate recorded source/compiled review. '
                     'No hardware, peer exchange, runtime margin, PCIe recovery or full R3 claim.'), indent=2))

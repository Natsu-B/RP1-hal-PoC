#!/usr/bin/env python3
"""Decode the v6 EXAC diagnostic from an offline uart10.raw capture only."""
import argparse
import json
import re
from pathlib import Path


def decode(text):
    records = {tag: {} for tag in ('PRF1_WORD', 'PRF1_SCRATCH', 'EXAC_WORD')}
    for tag, number, value in re.findall(
            r'\b(PRF1_WORD|PRF1_SCRATCH|EXAC_WORD)\s+(\d+)\s+(0x[0-9a-fA-F]+)\b', text):
        number, value = int(number), int(value, 16)
        if number in records[tag] and records[tag][number] != value:
            raise ValueError(f'conflicting {tag} {number}; decode one snapshot at a time')
        records[tag][number] = value
    for tag, indices in [('PRF1_WORD', [0, 1, 2]), ('PRF1_SCRATCH', range(16, 32)),
                         ('EXAC_WORD', range(13))]:
        missing = sorted(set(indices) - records[tag].keys())
        if missing:
            raise ValueError(f'missing {tag} indices {missing}')
    p, s, e = (records[tag] for tag in records)
    if (p[0], p[1], s[30]) != (int.from_bytes(b'PRF1', 'little'), 6,
                                int.from_bytes(b'EXA6', 'little')) or p[2] != 2:
        raise ValueError('not a completed v6 EXAC/PRF1 snapshot')
    names = ['done_before', 'done_after', 'ctrl_before', 'initial_normal', 'initial_ldrex',
             'c1_strex_status', 'c1_after_normal', 'c1_after_ldrex', 'naked_strex_status',
             'naked_after_normal', 'naked_after_ldrex', 'restore_normal', 'restore_ldrex',
             'final_restore_ldrex', 'magic', 'outcome']
    result = dict(hardware_verified=False, source='offline capture; authenticity not checked',
                  local=dict(zip(names, (s[i] for i in range(16, 32)))),
                  mailbox={str(i): e[i] for i in range(13)}, reverse_trial='not implemented')
    outcomes = {0: 'completed', 1: 'DONE bit6 not set; EXAC not accessed',
                2: 'unexpected owner encoding; EXAC not written', 3: 'C1 ownership not established',
                4: 'C0 acknowledgement timed out'}
    if s[31] not in outcomes:
        raise ValueError(f'unknown outcome {s[31]}')
    result['status'] = outcomes[s[31]]
    if s[31] in (0, 3, 4) and not s[17] & 64:
        raise ValueError('EXAC results with DONE bit6 clear')
    if s[31] == 0:
        if e[1] != 1 or e[5] != 1:
            raise ValueError('completed trial lacks C0 ack or C1 owner=1 control')
        result['cross_core'] = dict(c1_take_ldrex=e[2], c1_take_strex=e[3],
            c1_owned_normal=e[4], c1_owned_ldrex=e[5], c0_ldrex=e[8], c0_strex=e[9],
            c0_after_normal=e[10], c0_after_ldrex=e[11],
            c1_after_challenge_normal=e[6], c1_after_challenge_ldrex=e[7],
            c0_duration_prefix_us=e[12])
        if e[11] == 0:
            result['interpretation'] = 'C0 observed owner=0 while C1 held owner=1; exclusion failed in this direction'
        elif e[8] == e[11] == e[7] == 1:
            result['interpretation'] = 'C0 observed owner=1 throughout; consistent with exclusion in this ordered trial only'
        else:
            result['interpretation'] = 'unexpected values; no exclusion conclusion'
        if s[29] != s[20]:
            result['status'] = 'initial owner value not restored; investigate cleanup'
    return result


def self_test():
    p = {0: int.from_bytes(b'PRF1', 'little'), 1: 6, 2: 2}
    s = dict.fromkeys(range(16, 32), 0)
    s.update({16: 0, 17: 64, 20: 15, 29: 15, 30: int.from_bytes(b'EXA6', 'little')})
    e = dict.fromkeys(range(13), 0)
    e.update({1: 1, 5: 1, 7: 1, 8: 1, 9: 1, 11: 1})

    def capture():
        return '\n'.join(f'{tag} {i} 0x{v:08x}' for tag, words in
            [('PRF1_WORD', p), ('PRF1_SCRATCH', s), ('EXAC_WORD', e)] for i, v in words.items())

    assert 'consistent' in decode(capture())['interpretation']
    s[20] = s[29] = 0
    assert decode(capture())['status'] == 'completed'
    s[20] = s[29] = 15
    e[11] = 0
    assert 'failed' in decode(capture())['interpretation']
    for state in range(1, 5):
        s[31] = state
        assert 'interpretation' not in decode(capture())
    s[31] = 0
    for invalid in [capture().replace('PRF1_WORD 1 0x00000006', 'PRF1_WORD 1 0x00000005'),
                    capture().replace('EXAC_WORD 12 0x00000000', ''),
                    capture() + '\nEXAC_WORD 1 0x00000002']:
        try:
            decode(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError('invalid capture accepted')
    print('PASS: synthetic exclusion, steal, refusals, timeout, wrong version, missing/conflicting words')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path, nargs='?')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.capture is None:
        parser.error('capture is required unless --self-test')
    try:
        result = decode(args.capture.read_text(errors='replace'))
    except ValueError as error:
        parser.exit(2, f'EXAC decode failed: {error}\n')
    print(json.dumps(result, indent=2))
    if result['status'] != 'completed':
        raise SystemExit(1)


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Decode a v7 interleaved EXAC diagnostic from an offline uart10.raw capture."""
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
                         ('EXAC_WORD', range(26))]:
        missing = sorted(set(indices) - records[tag].keys())
        if missing:
            raise ValueError(f'missing {tag} indices {missing}')
    p, s, e = (records[tag] for tag in records)
    if (p[0], p[1], p[2], s[30]) != (int.from_bytes(b'PRF1', 'little'), 7, 2,
                                     int.from_bytes(b'EXA7', 'little')):
        raise ValueError('not a completed v7 EXAC/PRF1 snapshot')
    names = ['done_before', 'done_after', 'ctrl_before', 'initial_word7', 'initial_word6',
             'baseline_ldrex', 'baseline_strex', 'baseline_after', 'completed_trials',
             'final_word7', 'final_word6', 'timeout_trial', 'reserved28', 'reserved29',
             'magic', 'outcome']
    result = dict(hardware_verified=False, source='offline capture; authenticity not checked',
                  local=dict(zip(names, (s[i] for i in range(16, 32)))),
                  mailbox={'command': e[0], 'ack': e[1]})
    outcomes = {0: 'completed', 1: 'DONE bit6 not set; EXAC not accessed',
                2: 'unexpected owner encoding; EXAC not written',
                3: 'single-core baseline failed', 4: 'C0 acknowledgement timed out'}
    if s[31] not in outcomes:
        raise ValueError(f'unknown outcome {s[31]}')
    result['status'] = outcomes[s[31]]
    if s[31] != 1 and not s[17] & 64:
        raise ValueError('EXAC results with DONE bit6 clear')
    if s[24] & ~7:
        raise ValueError('invalid completed-trial mask')
    fields = ['c1_ldrex', 'c0_before', 'c0_strex', 'c0_after', 'c1_strex',
              'c1_after', 'c0_duration_us', 'c1_wait_us']
    result['trials'] = []
    for trial, label in enumerate(['same_word_store', 'same_word_exclusive',
                                   'different_word_store'], 1):
        row = dict(zip(fields, (e[i] for i in range(2 + (trial - 1)*8, 10 + (trial - 1)*8))))
        row.update(trial=label, completed=bool(s[24] & (1 << (trial - 1))),
                   c0_address='0x40400300' if trial == 3 else '0x40400380')
        if row['completed']:
            row['matches_expected'] = (
                [row[k] for k in fields[:6]] ==
                [15, 15, 0 if trial == 2 else 0xffffffff, 0,
                 0 if trial == 3 else 1, 1 if trial == 3 else 0])
        result['trials'].append(row)
    if s[31] == 0:
        if s[24] != 7 or e[0] != 0 or e[1] != 3 or s[27] != 0:
            raise ValueError('completed result lacks all three acknowledgements/results')
        result['matches_expected'] = (
            (s[21], s[22], s[23], s[25], s[26]) == (15, 0, 1, 15, 15)
            and all(row['matches_expected'] for row in result['trials']))
        result['interpretation'] = (
            'capture is consistent with same-word reservation invalidation and a different-word control; '
            'this does not establish a general lock implementation'
            if result['matches_expected'] else
            'unexpected baseline, interleaving, control, or cleanup; no lock-safety conclusion')
    return result


def self_test():
    p = {0: int.from_bytes(b'PRF1', 'little'), 1: 7, 2: 2}
    s = dict.fromkeys(range(16, 32), 0)
    s.update({17: 64, 19: 15, 20: 15, 21: 15, 23: 1, 24: 7, 25: 15, 26: 15,
              30: int.from_bytes(b'EXA7', 'little')})
    e = dict.fromkeys(range(26), 0)
    e[1] = 3
    for trial in range(1, 4):
        row = [15, 15, 0 if trial == 2 else 0xffffffff, 0,
               0 if trial == 3 else 1, 1 if trial == 3 else 0, 7, 12]
        e.update(dict(enumerate(row, 2 + (trial - 1)*8)))

    def capture():
        return '\n'.join(f'{tag} {i} 0x{v:08x}' for tag, words in
            [('PRF1_WORD', p), ('PRF1_SCRATCH', s), ('EXAC_WORD', e)] for i, v in words.items())

    assert decode(capture())['matches_expected']
    assert decode(capture())['hardware_verified'] is False
    for word, bad in [(6, 0), (22, 1), (12, 1)]:
        previous = e[word]
        e[word] = bad
        assert not decode(capture())['matches_expected']
        e[word] = previous
    s[25] = 0
    assert not decode(capture())['matches_expected']
    s[25] = 15
    for state in range(1, 5):
        s[31], s[24] = state, 0
        assert 'interpretation' not in decode(capture())
    s[31], s[24] = 0, 7
    for invalid in [capture().replace('PRF1_WORD 1 0x00000007', 'PRF1_WORD 1 0x00000006'),
                    capture().replace('EXAC_WORD 25 0x0000000c', ''),
                    capture() + '\nEXAC_WORD 1 0x00000002',
                    capture().replace('PRF1_SCRATCH 24 0x00000007',
                                      'PRF1_SCRATCH 24 0x00000003')]:
        try:
            decode(invalid)
        except ValueError:
            pass
        else:
            raise AssertionError('invalid capture accepted')
    print('PASS: synthetic interleavings, failed invalidation/control/pair/cleanup, '
          'refusals, timeout, version, missing/conflicting/incomplete records')


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
    except (ValueError, OSError) as error:
        parser.exit(2, f'EXAC decode failed: {error}\n')
    print(json.dumps(result, indent=2))
    if result['status'] != 'completed' or not result['matches_expected']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()

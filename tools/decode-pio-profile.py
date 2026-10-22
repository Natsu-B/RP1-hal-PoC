#!/usr/bin/env python3
"""Decode the last v6 read-profile capture; no device access or dependencies."""
import argparse
from pathlib import Path
import re

BIT = 1 << 27
FIELDS = (
    'reset_before', 'done_before', 'done_released', 'pio_ctrl_before',
    'fstat_before', 'flevel_before', 'clkdiv_readback', 'fstat_ready',
    'flevel_ready', 'sm0_addr', 'rx_value', 'fstat_after_pop',
    'reset_restored', 'done_restored', 'magic', 'status',
)
STATUS = {
    0: 'constant received; inspect cleanup flags',
    1: 'refused: reset already released (write-only program cannot be saved)',
    2: 'reset DONE timeout; no PIO access attempted',
    3: 'refused: active SM, restart, or nonempty FIFO',
    4: 'configuration readback mismatch; SM never enabled',
    5: 'RX FIFO timeout',
    6: 'unexpected FIFO value, level, or SM address',
}


def decode(text):
    text = re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]', '', text).replace('\r', '')
    begin = text.rfind('PROFILE_READ_BEGIN\n')
    if begin < 0:
        raise ValueError('missing PROFILE_READ_BEGIN')
    text = text[begin:]
    if 'PROFILE_READ_END\n' not in text:
        raise ValueError('last read is incomplete; refusing older or mixed results')
    text = text.split('PROFILE_READ_END\n', 1)[0]
    records = {'WORD': {}, 'SCRATCH': {}}
    for kind, index, value in re.findall(
            r'^PRF1_(WORD|SCRATCH) (\d+) (0[xX][0-9a-fA-F]{1,8})\s*$', text, re.M):
        index = int(index)
        if index in records[kind]:
            raise ValueError(f'duplicate {kind} word {index}')
        records[kind][index] = int(value, 16)
    p, s = records['WORD'], records['SCRATCH']
    if not {0, 1, 2} <= p.keys() or not set(range(16, 32)) <= s.keys():
        raise ValueError('missing required profile/scratch words')
    if (p[0], p[1], p[2], s[30]) != (0x31465250, 6, 2, 0x314f4950):
        raise ValueError('need live PRF1 version 6 and completed PIO1 magic')
    result = dict(zip(FIELDS, (s[i] for i in range(16, 32))))
    passed = (
        s[31] == 0 and s[16] & BIT != 0 and s[18] & BIT != 0
        and (s[28] ^ s[16]) & BIT == 0 and s[19] == 0
        and s[20] == 0x0f000f00 and s[21] == 0 and s[22] == 0x10000
        and s[23] == 0x0f000e00 and s[24] == 0x10 and s[25] == 3
        and s[26] == 21 and s[27] == 0x0f000f00
    )
    return result, passed


def self_test():
    words = [BIT, 0, BIT, 0, 0x0f000f00, 0, 0x10000, 0x0f000e00,
             0x10, 3, 21, 0x0f000f00, BIT, 0, 0x314f4950, 0]
    capture = 'PROFILE_READ_BEGIN\n' + ''.join(
        f'PRF1_WORD {i} 0x{v:08x}\n' for i, v in enumerate([0x31465250, 6, 2]))
    capture += ''.join(f'PRF1_SCRATCH {i} 0x{v:08x}\n'
                       for i, v in enumerate(words, 16)) + 'PROFILE_READ_END\n'
    assert decode(capture)[1]
    for status in (1, 2, 3, 4, 5, 6, 0x100, 0x200):
        assert not decode(capture.replace('PRF1_SCRATCH 31 0x00000000',
                                          f'PRF1_SCRATCH 31 0x{status:08x}'))[1]
    assert not decode(capture.replace('0x00000015', '0x00000014'))[1]
    bad = [capture.replace('PRF1_WORD 1 0x00000006', 'PRF1_WORD 1 0x00000005'),
           capture.replace('PRF1_SCRATCH 26 0x00000015\n', ''),
           capture.replace('0x314f4950', '0x3242494c'),
           capture + 'PROFILE_READ_BEGIN\n',
           capture.replace('PROFILE_READ_END', 'PRF1_WORD 1 0x00000006\nPROFILE_READ_END')]
    for text in bad:
        try:
            decode(text)
        except ValueError:
            continue
        raise AssertionError('invalid capture accepted')
    print('decoder self-test PASS (synthetic captures only)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path, nargs='?')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if args.capture is None:
        parser.error('supply uart10.raw or --self-test')
    try:
        result, passed = decode(args.capture.read_text(errors='replace'))
    except (OSError, ValueError) as error:
        print(f'PIO_RESULT=INVALID: {error}')
        return 2
    for index, name in enumerate(FIELDS, 16):
        print(f'LOCAL[{index}] {name}=0x{result[name]:08x}')
    status = result['status']
    print(STATUS.get(status & 0xff, 'unknown status'))
    if status & 0x100:
        print('FAIL: SM/register/FIFO cleanup mismatch')
    if status & 0x200:
        print('FAIL: reset bit restoration mismatch')
    print('SM0 CLKDIV INT=1 FRAC=0; source=clk_sys; memory timing probes omitted in v6')
    print('PIO_RESULT=' + ('PASS' if passed else 'FAIL'))
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())

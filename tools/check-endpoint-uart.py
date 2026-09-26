#!/usr/bin/env python3
"""Decode bounded read-only endpoint UART records; never infer a reset cause."""
import argparse
import hashlib
import json
from pathlib import Path
import re

NAMES = ('sel0', 'sel1', 'id', 'cmdstat', 'classrev', 'bhlc', 'bar0', 'bar1', 'bar2')
APBS = ('mon2', 'intr', 'inte', 'ints')
HEX32 = rb'0x([0-9a-f]{8})'
FIELDS = b''.join(b' '+n.encode()+b'='+HEX32 for n in NAMES)
EXTRA = b''.join(b' '+n.encode()+b'='+HEX32 for n in APBS)
ROW = re.compile(rb'RP1DBI event=(INIT|CHG |CAP |END ) elapsed_us=0x([0-9a-f]{16}) valid=([01])'+FIELDS+b'(?:'+EXTRA+b')?\r\n')


def decode(raw):
    assert len(raw) < 1_000_000, 'bounded capture'
    rows = []
    for m in ROW.finditer(raw):
        assert len(m[0]) in (206, 270)
        row = {'event': m[1].decode().strip(), 'elapsed_us': int(m[2], 16), 'valid': int(m[3])}
        row.update({k: int(v, 16) for k, v in zip(NAMES+APBS, m.groups()[3:]) if v is not None})
        assert not row['valid'] or row['sel0'] == row['sel1'] == 0
        rows.append(row)
    assert 0 < len(rows) <= 33 and rows[0]['event'] == 'INIT'
    assert all(a['elapsed_us'] < b['elapsed_us'] for a, b in zip(rows, rows[1:]))
    valid = [r for r in rows if r['valid']]
    loss = next((r for r in valid[1:] if r['classrev'] >> 8 == 0), None)
    lost = bool(valid and valid[0]['classrev'] >> 8 != 0 and loss)
    return {
        'classification': 'HW', 'result': 'OBSERVED_CLASS_LOSS' if lost else 'INCONCLUSIVE',
        'records': rows, 'incomplete_or_corrupt_lines': raw.count(b'RP1DBI event=')-len(rows),
        'apbs_records': sum(all(k in r for k in APBS) for r in rows),
        'class_loss_elapsed_us': loss['elapsed_us'] if lost else None,
        'reset_cause': 'OPEN', 'BAR_size': 'NOT_MEASURED', 'SCMI_completion': 'OPEN',
        'sampling': 'sequential plain reads once per monitor pass (~1s); missed transitions and selector ABA possible',
        'apbs_boundary': 'No acknowledgement or mask write; latched events and current levels do not establish event order or official state',
    }


def selftest():
    def row(event, time, cls, extended):
        s = f'RP1DBI event={event} elapsed_us=0x{time:016x} valid=1'
        values = (0, 0, 0x11de4, 6, cls, 0, 0x800000, 0, 0x400000)
        s += ''.join(f' {k}=0x{v:08x}' for k, v in zip(NAMES, values))
        if extended:
            s += ' mon2=0x00130000 intr=0x00000003 inte=0x00000000 ints=0x00000000'
        return (s+'\r\n').encode()
    for extended in (False, True):
        first, later = row('INIT', 100, 0x2000002, extended), row('CHG ', 1000100, 2, extended)
        report = decode(first+later)
        assert report['result'] == 'OBSERVED_CLASS_LOSS'
        assert report['apbs_records'] == (2 if extended else 0)
        assert report['reset_cause'] == 'OPEN'
        assert decode(first)['result'] == 'INCONCLUSIVE'
        assert decode(first+later[:-3])['incomplete_or_corrupt_lines'] == 1
        for bad in (b'', later, first+first, first.replace(b'valid=1 sel0=0x00000000', b'valid=1 sel0=0x00000001')):
            try:
                decode(bad)
            except AssertionError:
                pass
            else:
                raise AssertionError('malformed observation admitted')
    assert decode(first+later.replace(b' ints=0x00000000', b''))['incomplete_or_corrupt_lines'] == 1
    print('HOST endpoint parser PASS: legacy/extended, truncation, partial APBS and eight rejection cases')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('raw', nargs='?', type=Path)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        selftest()
    else:
        if args.raw is None or args.report is None:
            parser.error('raw and --report are required unless --self-test is used')
        data = args.raw.read_bytes()
        result = decode(data)
        result['raw_sha256'] = hashlib.sha256(data).hexdigest()
        args.report.write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps({k: result[k] for k in ('result', 'apbs_records', 'incomplete_or_corrupt_lines')}))

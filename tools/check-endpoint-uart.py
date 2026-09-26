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
GATE_NAMES = ('code', 'before', 'after', 'low', 'high', 'gap', 'reads', 'ro', 'sel',
              'mon0', 'mon1', 'budget', 'maxgap')
GATE = re.compile(b'RP1GATE'+b''.join(b' '+n.encode()+b'='+HEX32 for n in GATE_NAMES)+rb'\r\n')
GATE_RESULTS = ('WINDOW_CANDIDATE_ONLY', 'REJECT_EPOCH_EXPIRED', 'REJECT_LOW_HIGH_GAP',
                'REJECT_DEADLINE', 'REJECT_RESET_RECURRENCE', 'REJECT_SELECTOR',
                'REJECT_LEVEL_DRIFT', 'REJECT_RESET_TUPLE', 'REJECT_RO_ENABLED_OR_UNREAD')


def decode_gate(raw):
    """Independent diagnostic field; never changes legacy DBI classification."""
    records = [{k: int(v, 16) for k, v in zip(GATE_NAMES, m.groups())} for m in GATE.finditer(raw)]
    assert len(records) <= 1, 'one-shot gate must not rearm'
    for r in records:
        assert 1 <= r['code'] <= len(GATE_RESULTS), 'unknown gate code'
        r['result'] = GATE_RESULTS[r['code']-1]
        r['dbi_payload_reads_executed'] = r['reads'] & 0xfe0 == 0xfe0
        r['ro_read_executed'] = bool(r['reads'] & (1 << 13))
        assert (r['budget'], r['maxgap']) == (5000, 2500), 'unknown diagnostic timing contract'
        if r['code'] == 1:
            assert r['reads'] == 0xffff and r['ro'] & 1 == 0 and r['sel'] == 0
            assert r['mon0'] & r['mon1'] & 0x30000 == 0x30000
            assert (r['mon0'] ^ r['mon1']) & 0x1f0000 == 0
            assert r['gap'] == (r['high']-r['low']) & 0xffffffff
            age = (r['after']-r['low']) & 0xffffffff
            assert age < r['budget'] and r['gap'] <= min(r['maxgap'], age)
            assert (r['before']-r['low']) & 0xffffffff <= age
    return {'records': records, 'incomplete_or_corrupt_lines': raw.count(b'RP1GATE')-len(records),
            'write_admission': 'NOT_GRANTED',
            'boundary': '5ms/2.5ms diagnostic targets only; no physical 100ms exclusion, atomicity, MMIO completion or selector ownership proof'}


def decode(raw):
    assert len(raw) < 1_000_000, 'bounded capture'
    rows = []
    for m in ROW.finditer(raw):
        assert len(m[0]) in (206, 270)
        row = {'event': m[1].decode().strip(), 'elapsed_us': int(m[2], 16), 'valid': int(m[3])}
        row.update({k: int(v, 16) for k, v in zip(NAMES+APBS, m.groups()[3:]) if v is not None})
        assert not row['valid'] or row['sel0'] == row['sel1'] == 0
        row['dbi_payload_status'] = 'VALID_OBSERVED' if row['valid'] else 'INVALID_OR_UNEXECUTED'
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
            'sampling': 'sequential plain reads; cadence is image-dependent (legacy ~1s or bounded one-tick loop); missed transitions and selector ABA possible',
        'apbs_boundary': 'No acknowledgement or mask write; latched events and current levels do not establish event order or official state',
        'fresh_boot_gate': decode_gate(raw),
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
    skipped = later.replace(b'valid=1', b'valid=0')
    report = decode(first+skipped)
    assert report['result'] == 'INCONCLUSIVE'
    assert report['records'][1]['dbi_payload_status'] == 'INVALID_OR_UNEXECUTED'
    gate = b'RP1GATE code=0x00000001 before=0x000003e8 after=0x0000044c low=0x00000064 high=0x0000044c gap=0x000003e8 reads=0x0000ffff ro=0x00000000 sel=0x00000000 mon0=0x00130000 mon1=0x00130000 budget=0x00001388 maxgap=0x000009c4\r\n'
    report = decode(first+later+gate)
    assert report['result'] == 'OBSERVED_CLASS_LOSS'
    assert report['fresh_boot_gate']['records'][0]['result'] == 'WINDOW_CANDIDATE_ONLY'
    assert report['fresh_boot_gate']['write_admission'] == 'NOT_GRANTED'
    unread = gate.replace(b'code=0x00000001', b'code=0x00000007').replace(
        b'reads=0x0000ffff', b'reads=0x0000d01f')
    record = decode_gate(unread)['records'][0]
    assert not record['dbi_payload_reads_executed'] and not record['ro_read_executed']
    wrapped = gate.replace(b'low=0x00000064', b'low=0xffffff00').replace(
        b'before=0x000003e8', b'before=0x00000050').replace(
        b'after=0x0000044c', b'after=0x00000100').replace(
        b'high=0x0000044c', b'high=0x00000100').replace(b'gap=0x000003e8', b'gap=0x00000200')
    assert decode_gate(wrapped)['records'][0]['result'] == 'WINDOW_CANDIDATE_ONLY'
    assert decode(first+gate[:-3])['fresh_boot_gate']['incomplete_or_corrupt_lines'] == 1
    for code, result in enumerate(GATE_RESULTS[1:], 2):
        reject = gate.replace(b'code=0x00000001', f'code=0x{code:08x}'.encode())
        assert decode_gate(reject)['records'][0]['result'] == result
    for bad in (gate+gate, gate.replace(b'code=0x00000001', b'code=0x00000000'),
                gate.replace(b'ro=0x00000000', b'ro=0x00000001'),
                gate.replace(b'gap=0x000003e8', b'gap=0x000009c5'),
                gate.replace(b'after=0x0000044c', b'after=0x000013ec'),
                gate.replace(b'before=0x000003e8', b'before=0x0000044d'),
                gate.replace(b'high=0x0000044c', b'high=0x0000044d').replace(
                    b'gap=0x000003e8', b'gap=0x000003e9'),
                gate.replace(b'reads=0x0000ffff', b'reads=0x0000dfff'),
                gate.replace(b'mon1=0x00130000', b'mon1=0x00030000')):
        try:
            decode_gate(bad)
        except AssertionError:
            pass
        else:
            raise AssertionError('malformed candidate admitted')
    print('HOST endpoint parser PASS: legacy/extended classification unchanged, invalid/unexecuted payload, one-shot gate, no write admission')


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

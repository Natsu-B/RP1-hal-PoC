#!/usr/bin/env python3
"""CT01 telemetry / linked wrapper admission. Base R1/GPIO checks are separate."""
import json
from pathlib import Path
import re
import runpy
import subprocess
import sys

if not __debug__:
    raise SystemExit('critical timing admission requires assertions enabled')


def calls(disassembly):
    parts = re.split(r'(?m)^[0-9a-f]+ <([^>]+)>:\n', disassembly)
    functions = dict(zip(parts[1::2], parts[2::2]))
    direct, wrapped = {}, {}
    for name, body in functions.items():
        for kind in ('Enter', 'Exit'):
            for prefix, result in [('', direct), ('__wrap_', wrapped)]:
                count = len(re.findall(r'\b(?:bl|b\.w)\s+[0-9a-f]+ <' + prefix +
                                      'vPort' + kind + r'Critical>', body))
                if count:
                    result.setdefault(name, {})[kind] = count
    assert direct == {
        '__wrap_vPortEnterCritical': {'Enter': 1},
        '__wrap_vPortExitCritical': {'Exit': 1},
        'rp1_freertos_critical_timing_start': {'Enter': 1, 'Exit': 1},
    }, 'unmeasured critical caller or broken real-call binding'
    for name in ('xQueueGenericSend', 'xQueueReceive', 'xQueueSemaphoreTake',
                 'ulTaskGenericNotifyTake', 'xTaskGenericNotify',
                 'rp1_freertos_critical_timing_snapshot'):
        assert all(wrapped.get(name, {}).get(k, 0) > 0 for k in ('Enter', 'Exit')), name
    for kind in ('Enter', 'Exit'):
        body = functions['__wrap_vPort' + kind + 'Critical']
        assert '0x400ac000' in body and re.search(r'ldr\w*\s+\w+, \[\w+, #40\]', body), 'raw timer read absent'
    return {'wrapped_callers': len(wrapped),
            'wrapped_calls': sum(sum(v.values()) for v in wrapped.values())}


def elf(path):
    result = runpy.run_path(str(Path(__file__).with_name('check-freertos-elf.py')))['check'](path)
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', str(path)], text=True)
    result['critical_binding'] = calls(dis)
    result['measurement'] = 'task outer critical body only; no IRQ-mask WCET'
    return result


def validate(text):
    records = {}
    for m in re.finditer(r'\[RTOS\] (\d+) (\d{3}) ((?:[0-9a-f]{8} ?){4})', text):
        number, offset = map(int, m.group(1, 2))
        row = records.setdefault(number, {})
        assert offset not in row, 'duplicate row'
        row[offset] = [int(v, 16) for v in m[3].split()]
    assert text.count('[RTOS] observer-complete read-only=1') == 1, 'observer incomplete/duplicate'
    assert sorted(records) == list(range(31)), 'sample count'
    previous = None
    for n, record in sorted(records.items()):
        assert sorted(record) == list(range(0, 256, 4)), 'incomplete sample'
        w = [v for off in sorted(record) for v in record[off]]
        assert w[:2] == [0x31305452, 1] and w[2] != 0xffffffff and w[3] == 0
        assert not any(w[184:]), 'fault reserved region'
        if n < 3:
            continue
        assert w[96:99] == [int.from_bytes(b'CT01', 'little'), 1, 200], 'CT01 admission'
        assert 200 <= w[99] <= 500 and w[100] == 1, 'calibration count/time'
        assert w[101] > 0 and w[101] % 2 == 0 and w[101] == w[110], 'torn critical snapshot'
        assert w[102] > 1 and w[107] == 0 and 2 <= w[106] <= 32, 'count/depth/saturation'
        assert 0 <= w[103] <= w[105] <= w[104] < 0x80000000 and w[104] >= 200, 'body time'
        assert 0 < (w[112] - w[111]) & 0xffffffff < 0x80000000, 'measurement interval'
        if previous:
            assert w[101] > previous[101] and w[102] > previous[102], 'measurement stalled'
            assert w[104] >= previous[104] and w[106] >= previous[106], 'maximum regressed'
            assert w[103] <= previous[103] and w[111] == previous[111], 'measurement restarted'
        previous = w
    return {'classification': 'HW', 'result': 'CRITICAL_BODY_NUMERIC_PASS', 'samples': 31,
            'count': w[102], 'min_body_us': w[103], 'max_body_us': w[104],
            'max_nesting': w[106], 'calibration_us': w[99],
            'measurement_elapsed_us': (w[112]-w[111]) & 0xffffffff,
            'includes_200us_calibration': True, 'base_R1_and_GPIO_required': True,
            'timestamp_resolution_us': 1, 'full_interrupt_mask_maximum_proven': False}


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--elf':
        result = elf(sys.argv[2])
    else:
        assert len(sys.argv) == 2, 'UART log or --elf ELF'
        result = validate(Path(sys.argv[1]).read_text(errors='replace'))
    print(json.dumps(result, indent=2))

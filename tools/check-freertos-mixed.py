#!/usr/bin/env python3
"""Validate MIX1 numeric evidence; external SPI/UART/GPIO witnesses still required."""
import json
from pathlib import Path
import re
import sys

if not __debug__:
    raise SystemExit('Refuse disabled assertions')

def decode(text):
    records = {}
    for m in re.finditer(r'\[RTOS\] (\d+) (\d{3}) ((?:[0-9a-f]{8} ?){4})', text):
        n, off = map(int, m.group(1, 2))
        record = records.setdefault(n, {})
        assert off not in record, 'duplicate row'
        record[off] = [int(v, 16) for v in m[3].split()]
    assert text.count('[RTOS] observer-complete read-only=1') == 1, 'observer incomplete/duplicate'
    assert sorted(records) == list(range(31)), 'missing samples'
    out = []
    for n in range(31):
        assert sorted(records[n]) == list(range(0, 256, 4)), 'incomplete sample'
        out.append([v for off in sorted(records[n]) for v in records[n][off]])
    return out

def elapsed(start, end):
    value = (end-start) & 0xffffffff
    assert 0 < value < 0x80000000, 'time/progress reversed or stalled'
    return value

def validate(text):
    rows = decode(text)
    for n, w in enumerate(rows):
        assert w[:2] == [0x31305452, 1], 'RT01 schema'
        assert w[2] != 0xffffffff and w[3] == 0 and w[192] != 0x31544652, 'fault/assert'
        if n < 3:
            continue
        assert w[2] == 5 and w[19:21] == [0x13579bdf, 0], 'startup/monitor'
        assert w[28] == 0 and w[29] & 3 == 2 and w[30] & 7 == 0, 'monitor context'
        assert 0x2000e000 <= w[31] <= 0x2000f000
        assert w[21] & 0x700 == 0 and w[22]+1 == w[5]//1000, 'tick/priority'
        assert w[39] == 1 and w[17] == w[50] == 0, 'queue admission/not mutex cohort'
        assert w[40:45] == [0x3158494d, 5, 3, 2560, 256], 'MIX1 memory/tasks'
        limits = [256,128,128,256,256,512,512,512]
        free = w[32:39]+[w[45]]
        assert all(32 < v <= limit for v,limit in zip(free,limits)), 'task stack guard'
        assert 0 < w[18] <= 3968, 'MSP guard'
        assert w[70] == w[86] == 0, 'R4-R11 context'
        assert w[65] == w[81] == 0 and w[66] & 3 == w[82] & 3 == 2
        assert w[67] != w[83] and w[67] & 7 == w[83] & 7 == 0, 'spin PSP'
        assert all(0x20000000 <= w[i] < 0x2000e000 for i in [30,67,83])
        if n > 3:
            for index in [8,9,14,15,49,64,80]:
                elapsed(rows[n-1][index],w[index])
    final = rows[-3:]
    stable = [i for i in range(96,192) if i not in [99,131,163]]
    assert all([w[i] for i in stable] == [final[0][i] for i in stable] for w in final), 'unfinished ledger'
    w = final[-1]
    assert w[52] & 0xfe0 == 0x840 and w[53:55] == [1,1] and w[55] & 16, 'clock contract'
    psps = [w[i] for i in [30,67,83,46,47,51]]
    assert len(set(psps)) == 6 and all(0x20000000 <= p < 0x2000e000 and p & 7 == 0 for p in psps), 'owner PSP'
    for base,magic,count in [(96,b'SPM1',2),(128,b'ICM1',256),(160,b'UAM1',2)]:
        assert w[base] == int.from_bytes(magic,'little') and w[base+1:base+3] == [4,count], 'IO incomplete'
        assert w[base+10] == count and w[base+11] == 0, 'generation/error'
        assert w[base+14:base+16] == [0x5aa5a55a,0xa55a5aa5], 'buffer canary'
        elapsed(final[0][base+3],w[base+3])
        assert w[base+4] >= count and 0 < w[base+6] < 10_000 and 0 < w[base+7] < 10_000, 'IRQ/task wake'
    assert w[108:110] == [0x69963c01,0x69963c02], 'SPI task payload'
    overlaps = []
    for index,base in enumerate([112,120]):
        a = w[base:base+8]
        assert a[2:4] == [index+1,0x69963c01+index] and 1 <= a[4] <= 5, 'SPI receipt'
        assert 0 < a[5] <= elapsed(a[0],a[1]) < 50_000
        assert 0 < a[6] < 10_000 and 0 < a[7] < 10_000
        u = w[178+index*3:181+index*3]
        assert u[2] == index+1 and elapsed(u[0],u[1]) < 5_100_000, 'UART receipt'
        assert elapsed(u[0],a[0]) < elapsed(u[0],a[1]) < elapsed(u[0],u[1]), 'SPI not inside armed UART request'
        overlaps.append(dict(spi_start=a[0],spi_end=a[1],uart_start=u[0],uart_end=u[1]))
    assert w[140:142] == [0x40,0x00800001], 'I2C expected NACK'
    assert 4000 <= w[144] < 20_000 and w[145] >= 4 and w[146] < 10_000, 'I2C checked cleanup'
    assert w[147:149] == [0,0] and w[149] > 0 and w[151] > 0, 'I2C IRQ wake/armed UART overlap'
    assert w[152] == 0xc3c3c3c3 and w[153:157] == [0xc0,0xc0,0xc0,1], 'IRQ priority/route/buffer'
    assert w[172:174] == [41,19] and w[176] > 0 and w[177] == 0, 'UART IRQ/error'
    assert 8000 <= w[184] < 40_000 and w[190] == 0 and w[191] & 0x301 == 0x101, 'UART cleanup'
    data = b''.join(v.to_bytes(4,'big') for v in w[185:190])
    assert data == b'HOST2RP1 IRQ 0002\r\n\xc3', 'UART final task payload'
    ticks = elapsed(rows[3][8],w[8]); us = elapsed(rows[3][11],w[11])
    assert 950 <= us/ticks <= 1050, 'tick mean'
    return dict(classification='HW',result='R2_MIXED_NUMERIC_SELECTED_PASS',
        external_peer_and_gpio_required=True, samples=len(rows),elapsed_us=us,mean_tick_us=us/ticks,
        switches=w[9],queue_completions=w[15],msp_used_bytes=w[18],task_free_words=w[32:39]+[w[45]],
        spi=dict(completions=w[98],irq_entries=w[100],max_us=w[101:104]),
        i2c=dict(expected_nacks=w[130],irq_entries=w[132],max_us=w[133:136],uart_armed_overlaps=w[151]),
        uart=dict(completions=w[162],irq_entries=w[164],max_us=w[165:168]),overlapping_requests=overlaps,
        simultaneous_wire_payloads_proven=False,mutex_inheritance_in_this_image=False,
        R1='PARTIAL',R2='PARTIAL_MIXED_SPI_UART_I2C_NACK',R3='OPEN/BLOCKED')

if __name__ == '__main__':
    print(json.dumps(validate(Path(sys.argv[1]).read_text(errors='replace')),indent=2))

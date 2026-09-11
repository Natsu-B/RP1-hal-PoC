#!/usr/bin/env python3
"""Actual C body test; ELF call-binding refusals; synthetic CT01 log refusals."""
from pathlib import Path
import re
import runpy
import subprocess
import sys
import tempfile

if not __debug__:
    raise SystemExit('tests require assertions')
ROOT = Path(__file__).resolve().parents[1]
CHECK = runpy.run_path(str(ROOT/'tools/check-critical-timing.py'))


def refuses(function, value):
    try:
        function(value)
    except AssertionError:
        return
    raise AssertionError('corrupted input accepted')


def main(elf, normal):
    with tempfile.TemporaryDirectory(prefix='rp1-critical-test-', dir='/dev/shm') as tmp:
        binary = str(Path(tmp)/'test')
        subprocess.run(['cc', '-std=c11', '-Os', '-Wall', '-Wextra', '-Werror',
                        str(ROOT/'crates/rp1-freertos/tests/critical_timing.c'),
                        '-o', binary], check=True)
        subprocess.run([binary], check=True)
    CHECK['elf'](elf)
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', elf], text=True)
    refuses(CHECK['calls'], dis.replace('<__wrap_vPortEnterCritical>', '<vPortEnterCritical>'))
    refuses(CHECK['calls'], dis.replace('<__wrap_vPortExitCritical>', '<vPortExitCritical>'))
    refuses(CHECK['calls'], dis.replace('0x400ac000', '0x400ac004'))
    # Synthetic CT01 fields over real normal-R1 row framing, NOT a hardware run.
    text = Path(normal).read_text(errors='replace')
    values = {}
    for m in re.finditer(r'\[RTOS\] (\d+) (\d{3}) ((?:[0-9a-f]{8} ?){4})', text):
        n, off = map(int, m.group(1, 2))
        for i, v in enumerate(m[3].split()):
            assert (n, off+i) not in values, 'duplicate input row'
            values[n, off+i] = int(v, 16)
    for n in range(31):
        fields = {96: int.from_bytes(b'CT01','little'), 97:1, 98:200, 99:200,
                  100:1, 101:2*(n+1), 102:1000*(n+1), 103:0, 104:210,
                  105:1, 106:2, 107:0, 110:2*(n+1), 111:100, 112:100+1000000*(n+1)}
        for i, v in fields.items(): values[n, i] = v
    def render():
        return ''.join(f'[RTOS] {n} {off:03} '+
                       ' '.join(f'{values[n, off+i]:08x}' for i in range(4))+'\n'
                       for n in range(31) for off in range(0,256,4))+\
                       '[RTOS] observer-complete read-only=1\n'
    CHECK['validate'](render())
    for index, bad in [(96,0), (99,199), (100,2), (101,49), (110,1), (102,1),
                       (103,211), (104,199), (106,1), (107,1), (111,99), (184,1)]:
        old = values[24,index]; values[24,index] = bad
        refuses(CHECK['validate'], render()); values[24,index] = old
    refuses(CHECK['validate'], render()+render().splitlines()[0]+'\n')
    refuses(CHECK['validate'], render().replace('observer-complete', 'incomplete'))
    refuses(CHECK['validate'], render()+'[RTOS] observer-complete read-only=1\n')
    result = subprocess.run([sys.executable, '-O', str(ROOT/'tools/check-critical-timing.py')], capture_output=True)
    assert result.returncode != 0 and b'requires assertions' in result.stderr
    print('PASS STATIC: actual C bodies; ELF + 3 binding refusals; synthetic CT01 + 15 refusals; -O rejected; hardware OPEN')


if __name__ == '__main__':
    assert len(sys.argv) == 3, 'ELF normal-R1-UART-log'
    main(*sys.argv[1:])

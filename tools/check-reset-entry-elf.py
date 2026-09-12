#!/usr/bin/env python3
"""Selected pinned-build startup guard; not a general ARM control-flow proof."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

assert __debug__

def check(disassembly, symbols):
    def body(name):
        m=re.search(r'^[0-9a-f]+ <'+re.escape(name)+r'>:\n(.*?)(?=^\n)',disassembly,re.M|re.S)
        assert m, name
        return m[1]
    def sym(name):
        return int(re.search(r'^([0-9a-f]+) \w '+re.escape(name)+r'$',symbols,re.M)[1],16)
    reset=body('Reset');start=body('rp1_freertos_reset')
    capture=body('rp1_freertos_capture_reset_entry');halt=body('rp1_freertos_reset_entry_halt')
    assert sym('__sbss')<sym('__ebss')<=0x2000c000<0x2000e000<sym('_stack_start')==0x2000f000
    assert '0x2000e000' in reset and '0x2000f000' in reset
    for name in ['CONTROL','BASEPRI','FAULTMASK','MSP']:assert 'msr\t'+name in reset
    assert reset.index('cpsid\ti')<reset.index('msr\tMSP')<reset.index('<rp1_freertos_reset>')
    calls=re.findall(r'\bbl(?:eq)?\s+[0-9a-f]+ <([^>]+)>',start)
    assert calls==['rp1_freertos_capture_reset_entry','rp1_freertos_reset_entry_halt','rp1_entry'],calls
    assert start.index('<rp1_freertos_capture_reset_entry>')<start.index('strb')
    assert re.search(r'cmp\tr0, #1',start) and re.search(r'it\teq',start)
    assert re.search(r'bleq\s+[0-9a-f]+ <rp1_freertos_reset_entry_halt>',start)
    assert not re.search(r'\bbl(?:\w|\.)*\s',capture),'capture external call'
    assert 'push\t{r4, r5, r7, lr}' in capture
    # Reviewed pinned compiler: r1=cookie, r2=known REASON, no BSS/data base.
    assert '#64032' in capture and '#8192' in capture and '#16392' in capture and '#16405' in capture
    assert re.findall(r'\bldr(?:\.w)?\s+\w+, \[(\w+), #\d+\]',capture)==['r1','r1','r1','r1','r2']
    assert set(re.findall(r'\bstr(?:\.w)?\s+\w+, \[(\w+), #\d+\]',capture))=={'r1'}
    halt_calls=re.findall(r'\bbl\s+[0-9a-f]+ <([^>]+)>',halt)
    assert len(halt_calls)==1 and 'Peripherals5steal' in halt_calls[0]
    steal=body(halt_calls[0]);assert not re.search(r'\bbl(?:\w|\.)*\s',steal)
    assert not re.search(r'\b(?:ldrex|strex|cpsie|svc)\b',capture+halt+steal)
    assert 'wfe' in halt and not re.search(r'\bbx\s+lr|\bpop.*pc',halt)
    assert re.search(r'cpsid\ti\n[^\n]*\bb\.w\s+[0-9a-f]+ <Reset>',disassembly)
    return dict(status='PASS',classification='BUILD',capture_stack_bytes=16,
        capture_before_bss=True,positive_halt_before_application=True,
        cookie_range='0x2000fa20..0x2000fa2f',msp_range='0x2000e000..0x2000efff',
        selected_code_sha256={n:hashlib.sha256(b.encode()).hexdigest() for n,b in
            [('reset',reset),('startup',start),('capture',capture),('halt',halt),('steal',steal)]},
        boundary='Pinned compiler/instruction-pattern audit; not hardware reset or arbitrary CFG verification.')

if __name__=='__main__':
    p=Path(sys.argv[-1]);d=subprocess.check_output(['arm-none-eabi-objdump','-d',str(p)],text=True)
    s=subprocess.check_output(['arm-none-eabi-nm','-n',str(p)],text=True)
    result=check(d,s)
    if '--self-test' in sys.argv:
        mutations=[d.replace('msr\tMSP','msr\tPSP'),d.replace('bleq','blne'),
            d.replace('<rp1_freertos_capture_reset_entry>','<absent_capture>'),
            d.replace('#64032','#64036'),d.replace('cpsid\ti','cpsie\ti'),
            d.replace('wfe','svc')]
        for bad in mutations:
            try:check(bad,s)
            except (AssertionError,ValueError):pass
            else:raise AssertionError('invalid startup accepted')
        result['mutation_refusals']=len(mutations)
    result['elf_sha256']=hashlib.sha256(p.read_bytes()).hexdigest()
    print(json.dumps(result,indent=2))

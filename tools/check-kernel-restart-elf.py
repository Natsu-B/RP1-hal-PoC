#!/usr/bin/env python3
"""WDT8 pinned-build startup/monitor admission; not general CFG or HW proof."""
import hashlib
import json
from pathlib import Path
import re
import runpy
import subprocess
import sys

if not __debug__:raise SystemExit('assertions required')
# Reviewed AR rp1-01 compiler bodies, including inlined progress/packet checks.
REVIEWED={
    'Reset':'91aad3f9f840303c294867574591c504b1ab0ee93a1f0022621c16375c28290f',
    'rp1_freertos_reset':'7933a5a99e982a6b2e180fc565c39a24b86aae9ad70f439957a94895950e0e16',
    'rp1_freertos_capture_reset_entry':'6789e2c3830c33a9504e55e1e4485ba0576a01ccc92b884f2873bb49b4253ea1',
    'rp1_freertos_warm_start':'672af6df19a7a6e6a1939ed981eea20562e56715ffc92cee0079077d2ee45638',
    'warm_data7prepare':'d16cda9686af7119361b4b925af36f9bf692613145d6c5efaf9b3be7cf5ca665',
    'warm_data4info':'1014169c39a15bf77f2acbdfa6f9fd1ed50afab4ef0d07ad186654f5178d9ea1',
    'freertos_r17monitor':'530487e5306474a283068e3448ea0ddb54726bd4bc522af3c8ce3e4512610e78',
    'freertos_r13run':'256a711007d7045e355d20d38ae93479a6c7dcba7bbc08faeb106fbb236c3b94',
    'Peripherals5steal':'aa3e3f939f1b5d6915265354a3302a7c75965196a64989e2164e18c2cffdbe8b',
    '11into_output17h2e00c7b161f75994E':'918ec8f4cef207ae9bed8fd0e334f78fda81e3dd802614dc762a3b40ee6aca9a',
}
LAYOUT={'__data_start':0x20008da8,'__data_end':0x20008db8,
    '__sbss':0x20008db8,'__ebss':0x2000c10e,
    '__warm_data_shadow_start':0x2000c110,'__warm_data_shadow_end':0x2000c12c,
    '__image_end':0x2000d000,'__app_limit':0x2000e000,'_stack_start':0x2000f000}


def check(disassembly,symbols,sections,data_dump):
    blocks=re.findall(r'^([0-9a-f]+) <([^>]+)>:\n(.*?)(?=^\n|\Z)',disassembly,re.M|re.S)
    names={};bodies={}
    for selector,digest in REVIEWED.items():
        selected=[(name,body) for _,name,body in blocks
            if name==selector or (selector not in ('Reset','rp1_freertos_reset',
                'rp1_freertos_capture_reset_entry','rp1_freertos_warm_start') and selector in name)]
        assert len(selected)==1,('missing/duplicate reviewed body',selector)
        names[selector],body=selected[0]
        assert hashlib.sha256(body.encode()).hexdigest()==digest,('body changed; review required',selector)
        bodies[selector]=body
    for name,address in LAYOUT.items():
        matches=re.findall(r'^([0-9a-f]+) \w '+re.escape(name)+r'$',symbols,re.M)
        assert len(matches)==1 and int(matches[0],16)==address,('selected layout changed',name)
    for name,kind,start,end in [('.data','PROGBITS','__data_start','__data_end'),
        ('.bss','NOBITS','__sbss','__ebss'),
        ('.warm_data_shadow','NOBITS','__warm_data_shadow_start','__warm_data_shadow_end')]:
        rows=re.findall(r'^\s*\[\s*\d+\]\s+'+re.escape(name)+r'\s+(\w+)\s+([0-9a-f]+)\s+[0-9a-f]+\s+([0-9a-f]+)\s',sections,re.M)
        assert len(rows)==1 and rows[0][0]==kind and int(rows[0][1],16)==LAYOUT[start]
        assert int(rows[0][2],16)==LAYOUT[end]-LAYOUT[start],('section size changed',name)
    data=re.findall(r'^\s*20008da8\s+((?:[0-9a-f]{8}\s+){4})',data_dump,re.M)
    assert len(data)==1 and ''.join(data[0].split())=='df9b5713aaaaaaaa4c445238b3bbadc7','cold .data/token changed'
    reset=bodies['Reset'];start=bodies['rp1_freertos_reset'];capture=bodies['rp1_freertos_capture_reset_entry']
    prepare=bodies['warm_data7prepare'];warm=bodies['rp1_freertos_warm_start']
    def calls(body):return re.findall(r'\bbl(?:eq|ne)?\s+[0-9a-f]+ <([^>]+)>',body)
    assert calls(capture)==calls(prepare)==calls(bodies['warm_data4info'])==[]
    assert calls(start)==[names[n] for n in ('rp1_freertos_capture_reset_entry','warm_data7prepare','rp1_freertos_warm_start')]+['rp1_entry']
    assert start.index('<'+names['warm_data7prepare']+'>')<start.index('strb')<start.index('<rp1_freertos_warm_start>')
    assert 'cmp\tr4, #1' in start and 'bne.n\t200063c6' in start
    assert 'cbnz\tr0, 20006340' in start and 'wfe' in start
    assert reset.index('cpsid\ti')<reset.index('msr\tMSP')<reset.index('<rp1_freertos_reset>')
    assert all('msr\t'+n in reset for n in ('CONTROL','BASEPRI','FAULTMASK','MSP'))
    assert '0x2000e000' in reset and '0x2000f000' in reset
    # Compiler-specific guard checks the token branch precedes shadow/data writes.
    assert prepare.index('ldr.w\tr3, [fp]')<prepare.index('bne.w\t20006240')<prepare.index('str.w\tr1, [r5], #12')
    assert calls(warm)==[names[n] for n in ('warm_data4info','Peripherals5steal',
        '11into_output17h2e00c7b161f75994E','freertos_r13run')]
    assert not re.search(r'\b(?:ldrex\w*|strex\w*|clrex|cpsie|svc)\b',capture+prepare+warm)
    assert not re.search(r'\b(?:b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(?:\.[nw])?|bl(?:eq|ne)?|blx)\s+[0-9a-f]+ <Reset>',disassembly),'software Reset branch'
    monitor=bodies['freertos_r17monitor']
    for literal in ('#5000','#500','#150','#50','#400','#28672'):
        assert literal in monitor,('missing selected monitor/packet constant',literal)
    return dict(classification='BUILD',status='PASS',hardware='OPEN',
        runtime_restart_proven=False,hardware_reset_proven=False,
        selected_code_sha256=REVIEWED,layout={n:hex(a) for n,a in LAYOUT.items()},
        app_budget_bytes=56*1024,capture_stack_bytes=24,prepare_stack_bytes=48,
        cold_data_bytes=16,shadow_bytes=28,shadow_noload=True,
        capture_external_calls=0,capture_before_data_restore_and_bss=True,
        lost_cookie_recapture_guard=True,warm_start_direct_calls=calls(warm),
        warm_bypasses_cold_entry=True,software_reset_branch_present=False,
        progress_basis='Pinned monitor body gates five fresh passes and >=5000 ticks/switches before type-C; no external counter samples.',
        boundary='Exact reviewed compiler bodies and layout, plus generic ELF/probe checks. Not arbitrary control-flow verification, SRAM retention proof or hardware/runtime recovery evidence. R3 OPEN; no PCIe/peripheral/full-R2 recovery.')


if __name__=='__main__':
    args=sys.argv[1:];test=args[:1]==['--self-test']
    if test:args=args[1:]
    assert len(args)==1,'[--self-test] ELF'
    path=Path(args[0])
    d=subprocess.check_output(['arm-none-eabi-objdump','-d',str(path)],text=True)
    s=subprocess.check_output(['arm-none-eabi-nm','-n',str(path)],text=True)
    r=subprocess.check_output(['arm-none-eabi-readelf','-SW',str(path)],text=True)
    data=subprocess.check_output(['arm-none-eabi-objdump','-s','-j','.data',str(path)],text=True)
    result=check(d,s,r,data)
    result['foundation']=runpy.run_path(str(Path(__file__).with_name('check-freertos-elf.py')))['check'](path)
    assert result['foundation']['proc1'] is None
    result['watchdog_probe']=runpy.run_path(str(Path(__file__).with_name('check-freertos-watchdog-elf.py')))['check'](d)
    result['mutation_refusals']=0
    if test:
        mutations=[(d.replace(old,new),s,r,data) for old,new in [
            ('msr\tMSP','msr\tPSP'),('cpsid\ti','cpsie\ti'),
            ('#64032','#64036'),('bne.n\t200063c6','beq.n\t200063c6'),
            ('cbnz\tr0, 20006340','cbz\tr0, 20006340'),
            ('bne.w\t20006240','beq.w\t20006240'),('#5000','#4999'),
            ('#500\t','#499\t'),('#28672','#28671'),('strb.w','ldrb.w'),
            ('<rp1_freertos_capture_reset_entry>','<absent_capture>')]]
        mutations += [(d+'\n2000ffff: f000 b000 b.w 20005e90 <Reset>\n',s,r,data)]
        mutations += [(d,re.sub(r'^'+f'{a:08x}'+r'( \w '+re.escape(n)+r')$',
            f'{a+4:08x}'+r'\1',s,flags=re.M),r,data) for n,a in LAYOUT.items()]
        mutations += [(d,s,r.replace('.warm_data_shadow NOBITS','.warm_data_shadow PROGBITS'),data),
            (d,s,r,data.replace('4c445238','52554e38')),(d,s,r,data.replace('df9b5713','efbeadde'))]
        for index,bad in enumerate(mutations):
            assert bad!=(d,s,r,data),'ineffective mutation'
            try:check(*bad)
            except (AssertionError,ValueError):result['mutation_refusals']+=1
            else:raise AssertionError(f'invalid startup accepted: mutation {index}')
    result['elf_sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
    print(json.dumps(result,indent=2))

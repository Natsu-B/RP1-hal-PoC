#!/usr/bin/env python3
"""Mutate the actual phase1 ELF; never admit it as phase0 or legacy R1."""
import importlib.util
import os
import argparse
from pathlib import Path
import struct
import re
import subprocess
import sys
import tempfile

spec=importlib.util.spec_from_file_location('checker',Path(__file__).with_name('check-freertos-elf.py'))
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('elf',type=Path)
ap.add_argument('--amp-proc1-gpio-irq',action='store_true')
ap.add_argument('--amp-observation',action='store_true');args=ap.parse_args()
p=args.elf;kwargs=dict(require_scmi=True,require_ddr_diagnostics=True,amp_control=True,amp_proc1=True,
                      amp_proc1_gpio_irq=args.amp_proc1_gpio_irq,amp_observation=args.amp_observation)
def codegen(dis): return m.proc1_codegen(dis,args.amp_proc1_gpio_irq,args.amp_observation)
result=m.check(p,**kwargs)
for wrong in [{},dict(amp_control=True)]+([dict(amp_control=True,amp_proc1=True)] if args.amp_proc1_gpio_irq else []):
    try:m.check(p,**wrong)
    except AssertionError:pass
    else:raise AssertionError('phase1 admitted under wrong profile')
raw=p.read_bytes();h=struct.unpack_from('<16sHHIIIIIHHHHHH',raw)
sections=[struct.unpack_from('<10I',raw,h[6]+i*h[11]) for i in range(h[12])]
strings=sections[h[13]];names=raw[strings[4]:strings[4]+strings[5]]
slots={names[s[0]:].split(b'\0',1)[0]:(i,s) for i,s in enumerate(sections)}
mutations=[]
for name in [b'.amp_proc1_storage',b'.amp_proc1_vectors',b'.amp_proc1_stack']+([b'.amp_proc1_irq_storage',b'.amp_proc1_irq_text'] if args.amp_proc1_gpio_irq else [])+([b'.amp_observation_state',b'.amp_observation_text',b'.amp_observation_codec'] if args.amp_observation else [])+([b'.amp_proc1_profile_text'] if os.environ.get('RP1_AMP_PROC1_PROFILE') == '1' else []):
    i,s=slots[name]
    mutations += [(h[6]+i*h[11]+field*4,value) for field,value in [(1,0),(2,0),(3,s[3]+8),(5,0),(5,0x8000)]]
i,s=slots[b'.amp_control_contract'];mutations.append((s[4]+20,0))
# NOBITS still needs owned load memory; a zero file size does not waive coverage.
for i in range(h[10]):
    kind,off,address,pa,filesz,*_=struct.unpack_from('<8I',raw,h[5]+i*h[9])
    if kind == 1 and pa == 0x2000ba00:
        assert filesz == 0
        mutations.append((h[5]+i*h[9]+20,0))
        break
else: raise AssertionError('proc1 storage PT_LOAD missing')
if os.environ.get('RP1_AMP_PROC1_PROFILE') == '1':
    i,s=slots[b'.amp_proc1_profile_text']; mutations.append((s[4],0))
i,s=slots[b'.amp_proc1_vectors'];mutations += [(s[4]+j*4,0) for j in [0,1,2,79]+([16,17] if args.amp_proc1_gpio_irq else [])]
table=result['amp_proc1_probe']['codegen'].get('status_table')
if table:
    for i in range(h[10]):
        kind,off,address,_,size,*_=struct.unpack_from('<8I',raw,h[5]+i*h[9])
        if kind==1 and address<=table['address']<table['address']+48<=address+size:
            pos=off+table['address']-address
            mutations += [(pos+j*4,struct.unpack_from('<I',raw,pos+j*4)[0]^1) for j in range(12)]
            break
    else:raise AssertionError('status table not loaded')
if args.amp_proc1_gpio_irq:
    i,s=slots[b'.amp_proc1_irq_text'];mutations += [(s[4]+s[5]-24+j*4,0) for j in [0,1,2,4,5]]
cold=(result.get('amp_observation') or {}).get('cold_irq_admission')
if cold:
    i,s=slots[b'.amp_observation_codec'];pos=s[4]+cold['address']-s[3]
    # Actual image mutations remove IPSR/bank reads, change the final result,
    # or turn the NVIC read into a write before it can be admitted.
    mutations += [(pos+j,0) for j in [0,4,8,12,16,20,24,28,32,36,40]]
for name in ['entry','body_entry','fault']:
    va=result['amp_proc1_probe'][name]&~1
    for i in range(h[10]):
        kind,off,address,_,size,*_=struct.unpack_from('<8I',raw,h[5]+i*h[9])
        if kind==1 and address<=va<address+size:mutations.append((off+va-address,0));break
    else:raise AssertionError('proc1 code not loaded')
with tempfile.TemporaryDirectory(prefix='amp-proc1-elf-negative-') as d:
    bad=Path(d)/'bad.elf'
    for offset,value in mutations:
        b=bytearray(raw);struct.pack_into('<I',b,offset,value);bad.write_bytes(b)
        try:m.check(bad,**kwargs)
        except (AssertionError,ValueError):pass
        else:raise AssertionError(f'proc1 mutation admitted at {offset:#x}')
dis=subprocess.check_output(['arm-none-eabi-objdump','-d','-M','reg-names-raw',str(p)],text=True)
changes=[('AmpProc1Entry',r'cpsid\s+i','nop'),('AmpProc1Fault',r'cpsid\s+i','nop'),
         ('AmpProc1Body',r'sub(?:\.w)?\s+(?:sp, #|r13, r13, #)'+str(result['amp_proc1_probe']['codegen']['body_frame_bytes']-8),'sub sp, #400'),
         ('freertos_r18timesync3now',r'movs\s+r2, #5','movs r2, #6')]
for name,pattern,replacement in changes:
    block=re.search(r'(^[0-9a-f]+ <[^>]*'+name+r'[^>]*>:\n)(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)
    assert block and re.search(pattern,block[2]),'expected reviewed proc1 instruction missing'
    changed=block[1]+re.sub(pattern,replacement,block[2],count=1)
    try:codegen(dis.replace(block[0],changed,1))
    except AssertionError:pass
    else:raise AssertionError('unsafe proc1 codegen accepted: '+name)
for extra in ['2000aaaa: 4780 blx r0','2000aaaa: 81000000 .word 0x81000000','2000aaaa: 40018000 .word 0x40018000']:
    block=re.search(r'(^[0-9a-f]+ <AmpProc1Body>:\n)(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)
    try:codegen(dis.replace(block[0],block[0]+extra+'\n',1))
    except AssertionError:pass
    else:raise AssertionError('unadmitted proc1 call/address accepted')
tag=result['amp_proc1_probe']['codegen']['telemetry_version']
if tag=='P1A9':
    found=re.search(r'(^[0-9a-f]+ <AmpProc1Body>:\n)(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)
    due_registers = 'r0, r4' if os.environ.get('RP1_AMP_PROC1_PROFILE') == '1' else 'r2, r0'
    for pattern,replacement in [(r'subs\.w\s+'+due_registers+r', #200\b','subs.w '+due_registers+', #201'),
            (r'(subs\s+r0, )#1(\s*\n[^\n]*it\s+eq\s*\n[^\n]*moveq\.w\s+r0, #1000\b)',r'\g<1>#2\g<2>')]:
        assert found and len(re.findall(pattern,found[2]))==1,'period/countdown instruction missing'
        changed=found[1]+re.sub(pattern,replacement,found[2],count=1)
        try:codegen(dis.replace(found[0],changed,1))
        except AssertionError:pass
        else:raise AssertionError('period/countdown mutation admitted')
    print('HOST_PASS: fast due boundary and guard countdown mutations rejected')
if tag in ('P1A5','P1A6','P1A7','P1A8','P1A9'):
    for name,pattern,replacement in [('8set_high',r'#4194304\b','#2097152'),
            ('7set_low',r'0x400e3000\b','0x400e0000'),
            ('AmpProc1Fault',r'lsls\s+r1, r1, #22','lsls r1, r1, #21'),
            ('hardware12clear_output',r'0x400e3000\b','0x400e2000')]:
        exact=next((n for n in result['amp_proc1_probe']['codegen']['reachable_functions'] if name in n),name)
        found=re.search(r'(^[0-9a-f]+ <'+re.escape(exact)+r'>:\n)(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)
        assert found and re.search(pattern,found[2]), 'GPIO22 reviewed instruction missing'
        changed=found[1]+re.sub(pattern,replacement,found[2],count=1)
        try:codegen(dis.replace(found[0],changed,1))
        except AssertionError:pass
        else:raise AssertionError('GPIO22 mutation admitted: '+name)
    print('HOST_PASS: four GPIO22 mask/alias/fault/stop mutations rejected')
if args.amp_proc1_gpio_irq:
    changes=[('AmpProc1GpioIrq',r'cpsid\s+i','nop'),
        ('AmpProc1GpioIrq',r'cmp\s+r1, #16','cmp r1, #17'),
        ('AmpProc1GpioIrq',r'mov\.w\s+r12, #4','mov.w r12, #5'),
        ('AmpProc1GpioIrq',r'#512\b','#1024'),
        ('AmpProc1GpioIrq',r'0x400d3114\b','0x400d3108'),
        ('AmpProc1GpioIrq',r'0x400d204c\b','0x400d2044'),
        ('gpio_irq5force',r'0x400d2114\b','0x400d2108'),
        ('gpio_irq5start',r'0x400d0110\b','0x400d0104'),
        ('gpio_irq4stop',r'cpsid\s+i','nop'),
        ('AmpProc1Fault',r'0xe000e180\b','0xe000e184')]
    for name,pattern,replacement in changes:
        found=re.search(r'(^[0-9a-f]+ <[^>]*'+name+r'[^>]*>:\n)(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)
        assert found and re.search(pattern,found[2]), 'GPIO IRQ reviewed instruction missing'
        changed=found[1]+re.sub(pattern,replacement,found[2],count=1)
        try:codegen(dis.replace(found[0],changed,1))
        except AssertionError:pass
        else:raise AssertionError('GPIO IRQ mutation admitted: '+name)
    print('HOST_PASS: ten GPIO IRQ route/mask/clock/alias/stop mutations rejected')
# Later ABI/transport mutations must always operate on the actual body.
block=re.search(r'(^[0-9a-f]+ <AmpProc1Body>:\n)(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)
magic=format(int.from_bytes(tag.encode(),'little'),'08x')
for replacement in ['00000000','32413150' if tag=='P1A1' else '31413150']:
    changed=block[0].replace('.word\t0x'+magic,'.word\t0x'+replacement)
    assert changed!=block[0], 'reviewed telemetry literal missing'
    try:codegen(dis.replace(block[0],changed,1))
    except AssertionError:pass
    else:raise AssertionError('telemetry/frame mismatch admitted')
other=re.search(r'^([0-9a-f]+) <rp1_bulk_poll>:',dis,re.M);assert other
for branch in ['beq.w','cbnz r0,']:
    extra=f'2000aaaa: d000 {branch} {other[1]} <rp1_bulk_poll>\n'
    try:codegen(dis.replace(block[0],block[0]+extra,1))
    except AssertionError:pass
    else:raise AssertionError('conditional branch to transport accepted')
print(f'HOST_PASS: phase1 explicit,phase0/legacy refused,{len(mutations)} ELF +11 codegen mutations rejected; no hardware')

#!/usr/bin/env python3
"""Selected WDT9 reviewed image identity + existing ELF/probe checks, NOT HW proof."""
import hashlib,json,runpy,subprocess,sys
from pathlib import Path
assert __debug__
PIN='10bba8e3e82f4a167ff5eac10c3418375251f28c4f375b8b61ef8b8a38c77e09'
def identity(data):
    assert hashlib.sha256(data).hexdigest()==PIN,'Image changed: new source/compiled review required'
if __name__=='__main__':
    args=sys.argv[1:];test=args[:1]==['--self-test']
    if test:args=args[1:]
    assert len(args)==1,'[--self-test] ELF'
    p=Path(args[0]);data=p.read_bytes();identity(data)
    root=Path(__file__).parent
    base=runpy.run_path(str(root/'check-freertos-elf.py'))['check'](p)
    assert base['proc1'] is None
    d=subprocess.check_output(['arm-none-eabi-objdump','-d',str(p)],text=True)
    probe=runpy.run_path(str(root/'check-freertos-watchdog-elf.py'))['check'](d)
    refusals=0
    if test:
        for i in [0,1,4,8,32,len(data)//4,len(data)//2,len(data)-4,len(data)-1]:
            bad=bytearray(data);bad[i]^=1
            try:identity(bad)
            except AssertionError:refusals+=1
            else:raise AssertionError('Image mutation accepted')
    print(json.dumps(dict(classification='BUILD',status='PASS',elf_sha256=PIN,
        foundation=base,watchdog_probe=probe,image_mutation_refusals=refusals,
        hardware='OPEN',runtime_restart_proven=False,
        boundary='Exact WDT9 compiler image manually reviewed; no generalized semantic verifier. Type E only identifies a rejected guard. No relaxed IRQ gate, PCIe/peripheral recovery, autonomous feeding or full R3 claim.'),indent=2))

#!/usr/bin/env python3
"""Selected AX warm SPI reviewed image identity + existing ELF/probe checks, NOT HW proof."""
import hashlib,json,runpy,subprocess,sys
from pathlib import Path
assert __debug__
PIN='5f421f30e88cc3550819c21a3507266f138c9663bf7d21bca8802b522d1dea9d'
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
        boundary='Exact warm SPI compiler image manually reviewed; no generalized semantic verifier. Cold watchdog then fresh warm SPI owner; 2 real 4-byte IRQ19 exchanges and >=32-word warm stack margin gate F. Hardware OPEN; no PCIe recovery, autonomous feeding or full R3 claim.'),indent=2))

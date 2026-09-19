#!/usr/bin/env python3
"""Independent local-stack R1 control/UDF BUILD checks, never deployment admission."""
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile

assert __debug__
# Source-audited naked capture code from BE normal07 cc693d40. Its literal pool
# is checked separately: function ST_SIZE excludes those six address words.
CAPTURE_CODE_SHA='5cfe5400b4135e8071414ca2d39c9cca9b7f0271594cf9e0ea53a425bb97a494'
MODES=('freertos-r1-local-stack','freertos-r1-local-stack-fault')

def check(path, mode):
    assert mode in MODES
    spec=importlib.util.spec_from_file_location('foundation',Path(__file__).with_name('check-freertos-elf.py'))
    base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
    result=base.check(path,True)
    data=Path(path).read_bytes();h=struct.unpack_from('<16sHHIIIIIHHHHHH',data)
    headers=[struct.unpack_from('<8I',data,h[5]+i*h[9]) for i in range(h[10])]
    def offset(addr,size):
        matches=[off+addr-va for kind,off,va,pa,fs,ms,flags,align in headers
                 if kind==1 and va<=addr and addr+size<=va+fs]
        assert len(matches)==1,'code must be file-backed once'
        return matches[0]
    symbols={}
    for line in subprocess.check_output(['arm-none-eabi-nm','-S','--defined-only',str(path)],text=True).splitlines():
        parts=line.split()
        if len(parts) in (3,4):
            symbols[parts[-1]]=(int(parts[0],16),int(parts[1],16) if len(parts)==4 else None)
    assert not any(n in symbols for n in ['rp1_freertos_capture_reset_entry','rp1_freertos_warm_start']), 'WDT/warm path linked'
    address,size=symbols['RP1RtosFault'];assert size==124
    start=offset(address,size+24)
    assert hashlib.sha256(data[start:start+size]).hexdigest()==CAPTURE_CODE_SHA,'fault path changed; review required'
    assert struct.unpack_from('<6I',data,start+size)==(0x2000fb00,0xe000ed28,0x10003800,0x10003fe0,0x2000efe0,0x31544652), 'fault literals'
    probe='rp1_rtos_udf_instruction'
    assert (probe in symbols)==(mode==MODES[1]), 'probe/mode mismatch'
    mutation_offsets=[start,start+size+8,start+size+12]
    if probe in symbols:
        pc,_=symbols[probe]
        assert data[offset(pc,2):offset(pc,2)+2]==b'\x51\xde','expected architected UDF #0x51'
        mutation_offsets.append(offset(pc,2))
    result.update(feature=mode,elf_sha256=hashlib.sha256(data).hexdigest(),
        fault_code_bytes=size,fault_pool_words=6,halt_only_fault=True,
        explicit_udf=probe in symbols,probe_pc=symbols.get(probe,(None,None))[0],
        WDT_warm_entry_linked=False,hardware_admitted=False,
        boundary='BUILD only. No local-frame HW proof, no BE warm-health proof, no recovery/IRQ/peripheral timing claim.')
    return result,mutation_offsets

if __name__=='__main__':
    args=sys.argv[1:];test=args[:1]==['--self-test']
    if test:args=args[1:]
    assert len(args)==2,'[--self-test] ELF feature'
    path,mode=args;result,offsets=check(path,mode)
    if test:
        data=Path(path).read_bytes()
        with tempfile.TemporaryDirectory(prefix='rp1-local-r1-refusal-',dir='/dev/shm') as tmp:
            for at in offsets:
                changed=bytearray(data);changed[at]^=1
                candidate=Path(tmp)/'bad.elf';candidate.write_bytes(changed)
                try:check(candidate,mode)
                except AssertionError:pass
                else:raise AssertionError('changed code/literal/probe admitted')
        try:check(path,MODES[1] if mode==MODES[0] else MODES[0])
        except AssertionError:pass
        else:raise AssertionError('wrong mode admitted')
        result['refusal_tests']=len(offsets)+1
    print(json.dumps(result,indent=2))

#!/usr/bin/env python3
"""Check the monomorphic Cortex-M3 local queue probe, not hardware timing."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess


def check_disassembly(disassembly):
    blocks = {}
    for block in re.split(r'\n[0-9a-f]+ <',disassembly)[1:]:
        name,body=block.split('>:',1)
        blocks[name]=body
    if not {'local_queue_codegen_probe','local_control_codegen_probe'} <= blocks.keys():
        raise ValueError('missing monomorphic probes')
    body=blocks['local_queue_codegen_probe']
    if re.search(r'\b(?:ldrex|strex|ldaxr|stlxr)\w*\b',body):
        raise ValueError('queue uses exclusive/read-modify-write instructions')
    barriers=len(re.findall(r'\bdmb\b',body))
    if barriers < 4:
        raise ValueError('queue acquire/release publication barriers missing')
    if re.search(r'\bblx\b',body):
        raise ValueError('queue uses an indirect call')
    calls=re.findall(r'R_ARM_THM_(?:CALL|JUMP24)\s+(\S+)',body)
    if any(name not in {'memcpy','__aeabi_memcpy','__aeabi_memcpy4','__aeabi_memcpy8'} for name in calls):
        raise ValueError('queue calls an unadmitted helper')
    return {'status':'HOST_CODEGEN_PASS','scope':'Cortex-M3 queue push/pop publication; no hardware timing or SRAM contention proof',
            'queue_dmb_instructions':barriers,'queue_exclusive_instructions':0,'queue_helpers':calls}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--object',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();data=a.object.read_bytes()
    if data[:7]!=b'\x7fELF\x01\x01\x01' or struct.unpack_from('<H',data,18)[0]!=40:
        raise ValueError('ELF32 little-endian ARM object required')
    dis=subprocess.check_output(['arm-none-eabi-objdump','-dr',str(a.object)],text=True)
    result=check_disassembly(dis)
    result['object']={'path':str(a.object),'sha256':hashlib.sha256(data).hexdigest()}
    with a.output.open('x') as f:json.dump(result,f,indent=2);f.write('\n')
    print(a.output,result['status'])

if __name__=='__main__':main()

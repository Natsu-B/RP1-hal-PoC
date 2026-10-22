#!/usr/bin/env python3
"""Positive bound and unsafe-transfer/stack mutations for the actual gate."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path
from unittest.mock import patch

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('elf', type=Path)
p.add_argument('--openamp-source', type=Path, required=True)
p.add_argument('--review', type=Path, default=Path(__file__).with_name('amp-observation-stack-review.json'))
a = p.parse_args()
spec = importlib.util.spec_from_file_location('stack_gate', Path(__file__).with_name('check-amp-worker-stack.py'))
gate = importlib.util.module_from_spec(spec); spec.loader.exec_module(gate)
proof = json.loads(a.review.read_text())
positive = gate.audit(a.elf, proof, a.openamp_source)
assert positive['roots']['worker']['remaining_bytes'] == 260
assert positive['roots']['service']['remaining_bytes'] == 524
assert positive['runtime_msp_bound_bytes'] == 904
assert positive['startup_msp_bound_bytes'] == 528
assert positive['conservative_combined_msp_bound_bytes'] == 1432
count = 0
def refused(run, reason):
    global count
    try: run()
    except ValueError as error:
        assert reason in str(error), (reason, str(error)); count += 1
    else: raise AssertionError('unsafe mutation accepted: '+reason)

bad = copy.deepcopy(proof); bad['elf_sha256'] = '00'*32
refused(lambda: gate.audit(a.elf, bad, a.openamp_source), 'unreviewed ELF hash')
bad = copy.deepcopy(proof); bad['sources_sha256']['hal/openamp/rp1.c'] = '00'*32
refused(lambda: gate.audit(a.elf, bad, a.openamp_source), 'reviewed source changed')
bad = copy.deepcopy(proof); del bad['indirect_targets']['rpmsg_virtio_rx_callback']['174']
refused(lambda: gate.audit(a.elf, bad, a.openamp_source), 'unresolved indirect transfer')
table=next(pc for pc,n in proof['table_entries'].items() if n==12)
bad = copy.deepcopy(proof); bad['table_entries'][table] = 256
refused(lambda: gate.audit(a.elf, bad, a.openamp_source), 'table branches outside')
bad = copy.deepcopy(proof); del bad['table_entries'][table]
refused(lambda: gate.audit(a.elf, bad, a.openamp_source), 'unreviewed table branch')

def local(ins, rules=None):
    node = dict(name='test', size=64, ins=ins)
    return gate.local_analysis(0x20000000, node, {0x20000000:node}, rules or {}, {}, b'')
refused(lambda: local({0x20000000:'sub\tr13, r0'}), 'dynamic stack allocation')
refused(lambda: local({0x20000000:'mov\tr13, r4'}), 'unreviewed SP write')
refused(lambda: local({0x20000000:'mov\tr15, r4'}), 'unreviewed PC write')
refused(lambda: local({0x20000000:'bx\tr3'}), 'unresolved indirect transfer')
refused(lambda: local({0x20000000:'msr\tMSP, r0'}), 'unreviewed stack register switch')
refused(lambda: local({0x20000000:'ldr.w\tr15, [r13], #8'}), 'unreviewed PC write')
assert local({0x20000000:'ldr.w\tr15, [r13], #4'}) == (0, [])
refused(lambda: local({0x20000000:'push\t{r4, r14}',
                      0x20000002:'b.n\t20000000 <test>'}), 'stack allocation loop')
assert local({0x20000000:'str.w\tr11, [r13, #-4]!',
              0x20000004:'bx\tr14'}) == (4, [])

original = gate.subprocess.check_output
def enlarged(cmd, **kw):
    output = original(cmd, **kw)
    if cmd[0] == 'arm-none-eabi-objdump':
        assert 'sub.w\tr13, r13, #796' in output
        output = output.replace('sub.w\tr13, r13, #796', 'sub.w\tr13, r13, #1200', 1)
    return output
with patch.object(gate.subprocess, 'check_output', side_effect=enlarged):
    refused(lambda: gate.audit(a.elf, proof, a.openamp_source), 'insufficient task stack reserve')
bad=copy.deepcopy(proof); del bad['stack_register_writes']['Reset']['54']
refused(lambda: gate.audit(a.elf,bad,a.openamp_source), 'unreviewed stack register switch')
bad=copy.deepcopy(proof); bad['stack_pivots']['prvPortStartFirstTask']['sha256']='00'*32
refused(lambda: gate.audit(a.elf,bad,a.openamp_source), 'stack pivot instructions/literals changed')
assert gate.parse('20000000 <weak>:\n20000000: 4770 bx r14\n','20000000 00000002 W weak\n')[0x20000000]['size']==2
def startup_enlarged(cmd, **kw):
    output=original(cmd,**kw)
    if cmd[0]=='arm-none-eabi-objdump':
        import re
        block=re.search(r'^([0-9a-f]+ <[^>]*__rp1_main[^>]*>:\n)(.*?)(?=^[0-9a-f]+ <)',output,re.M|re.S)
        assert block and 'sub\tsp, #176' in block[2]
        output=output.replace(block[0],block[1]+block[2].replace('sub\tsp, #176','sub\tsp, #4000',1),1)
    return output
with patch.object(gate.subprocess,'check_output',side_effect=startup_enlarged):
    refused(lambda: gate.audit(a.elf,proof,a.openamp_source), 'insufficient startup MSP reserve')
print(f'HOST_PASS exact cold/scheduler bound; {count} unsafe mutations refused; weak symbol and single-register pop covered')

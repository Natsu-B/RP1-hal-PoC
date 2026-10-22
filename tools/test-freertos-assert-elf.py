#!/usr/bin/env python3
"""Check the actual linked official-kernel assertion path; no hardware."""
import re
import subprocess
import sys

names=['rp1_freertos_config_assert_probe','vTaskPrioritySet','rp1_freertos_assert']
def bodies(text):
    result={}
    for name in names:
        m=re.search(r'^[0-9a-f]+ <'+name+r'>:\n(.*?)(?=^[0-9a-f]+ <|\Z)',text,re.M|re.S)
        assert m,'missing linked '+name
        result[name]=m[1]
    return result
def check(b):
    p,k,a=(b[n] for n in names)
    assert re.search(r'movs\s+r1, #8\b',p) and re.search(r'movs\s+r0, #0\b',p)
    assert p.index('<vTaskPrioritySet>')<p.index('#42321')<p.index('<rp1_freertos_fault_hook>')
    assert re.search(r'cmp\s+r1, #7\b',k)
    assert re.search(r'bls\.n\s+[0-9a-f]+ <vTaskPrioritySet\+0x14>',k)
    assert k.index('#2857')<k.index('<rp1_freertos_assert>')<k.index('<vPortEnterCritical>')
    assert re.search(r'mov\s+r1, r0\b',a) and re.search(r'movs\s+r0, #1\b',a)
    assert '<rp1_freertos_fault_hook>' in a

text=subprocess.check_output(['arm-none-eabi-objdump','-d',sys.argv[1]],text=True)
original=bodies(text);check(original)
for name,before,after in [(names[0],'#8','#7'),(names[0],'<vTaskPrioritySet>','<wrong>'),
    (names[0],'#42321','#42320'),(names[1],'#2857','#2856'),
    (names[1],'bls.n','bhi.n'),(names[2],'#1','#4')]:
    assert before in original[name]
    bad=dict(original);bad[name]=bad[name].replace(before,after,1)
    try:check(bad)
    except (AssertionError,ValueError):pass
    else:raise AssertionError('accepted wrong assertion path')
print('BUILD linked C probe->vTaskPrioritySet assert2857->reason1 hook, positive1/negative6 PASS; HW OPEN')

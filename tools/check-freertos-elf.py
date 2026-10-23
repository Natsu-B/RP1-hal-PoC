#!/usr/bin/env python3
"""Reject an unsafe R1 memory/vector/link result. No third-party Python modules."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import os
import struct
import subprocess
import sys

if not __debug__:
    raise SystemExit('ELF admission requires assertions enabled; refuse -O/PYTHONOPTIMIZE')


def profile_reference():
    path=os.environ.get('RP1_AMP_PROC1_PROFILE_REFERENCE')
    assert path, 'profile build needs RP1_AMP_PROC1_PROFILE_REFERENCE (sealed X RP1.elf)'
    text=subprocess.check_output(['arm-none-eabi-objdump','-d','-M','reg-names-raw',path],text=True)
    starts=list(re.finditer(r'^([0-9a-f]+) <([^>]+)>:\n',text,re.M))
    return {m[2]:text[m.end():starts[i+1].start() if i+1<len(starts) else len(text)] for i,m in enumerate(starts)}


def proc1_codegen(disassembly, gpio_irq=False, observation=False):
    profile=os.environ.get('RP1_AMP_PROC1_PROFILE')=='1'
    doorbell=profile and os.environ.get('RP1_AMP_PROC1_LOCAL_STACK')=='1' and os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1'
    starts=list(re.finditer(r'^([0-9a-f]+) <([^>]+)>:\n',disassembly,re.M))
    blocks={int(m[1],16):(m[2],disassembly[m.end():starts[i+1].start() if i+1<len(starts) else len(disassembly)]) for i,m in enumerate(starts)}
    named={name:(addr,body) for addr,(name,body) in blocks.items()}
    for name in ['AmpProc1Entry','AmpProc1Fault']:
        body=named[name][1]
        assert re.search(r'\bcpsid\s+i\b',body.splitlines()[0]), 'proc1 entry/fault must mask IRQ without stack use'
        assert not re.search(r'\b(?:push|bl|blx)\b',body), 'proc1 entry/fault uses stack or call before private context'
    text=named['AmpProc1Body'][1];body=text.splitlines()
    versions=[(tag,frame) for tag,frame in [('P1A1',352),('P1A2',368),('P1A3',352),('P1A4',600),('P1A5',600),('P1A6',600),('P1A7',600),('P1A8',600),('P1A9',600)]
              if re.search(r'\.word\s+0x'+format(int.from_bytes(tag.encode(),'little'),'08x')+r'\b',text)]
    assert len(versions)==1, 'proc1 telemetry codegen version changed'
    version,frame=versions[0]
    assert not observation or (gpio_irq and version=='P1A9'), 'collector C1 codegen profile'
    body_code='\n'.join(line.split(':',1)[1].split('@',1)[0].strip() for line in text.splitlines()
        if re.match(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]{4}\s+)+[a-z]',line)).encode()
    if profile:
        # Reviewed profile bodies: v2/v3 (shared stack) and the proc-local DSRAM stack diagnostic.
        # Keyed by (DSRAM stack, hooks compiled out); each value was reviewed against its disassembly.
        body_pins={('0','0'):'afe9292ec9f8ca0a5bd0b438362a74ef7674aa4fa4d4bf1dae2b3e406e8008ec',
                   ('1','0'):'79b4cad2ba4eebfb2653e2b1c1d7ed3f268ad088fdb0b1ef4714dbccd3b99405',
                   ('1','1'):'501a9c131bed3096fe93f0e23c5ddb268fbdeb7cb91475dc5b9af36cb30fbaa0'}
        mode=(os.environ.get('RP1_AMP_PROC1_LOCAL_STACK','0'),os.environ.get('RP1_AMP_PROC1_NO_HOOKS','0'))
        assert mode in body_pins and hashlib.sha256(body_code).hexdigest()==body_pins[mode], 'profile loop instructions changed; review again'
    peer_quiesce=not observation and version=='P1A9' and hashlib.sha256(body_code).hexdigest()=='c4324f7e22b9c180a8de3c004d22117f29b904f7c17d8e663eb1ec6f116f1508'
    cold_irq=observation and hashlib.sha256(body_code).hexdigest()=='c437a91dfc53c19b1d62c47e26e0e4c5d469f608d0b9e852e9a1de268ad149eb'
    # Profiler builds keep the same cold-IRQ admission source; the guard symbol check in check() still applies.
    cold_irq=cold_irq or (observation and profile)
    assert gpio_irq == (version in ('P1A6','P1A7','P1A8','P1A9')), 'GPIO IRQ requires separate explicit admission'
    fault=named['AmpProc1Fault'][1]
    assert ('0x400e3000' in fault)==(version in ('P1A5','P1A6','P1A7','P1A8','P1A9')), 'fault output contract/version mismatch'
    if version in ('P1A5','P1A6','P1A7','P1A8','P1A9'):
        assert re.search(r'lsls\s+r1, r1, #22\b',fault) and re.search(r'str\s+r1, \[r0, #0\]',fault), 'fault must clear only GPIO22'
    assert ('amp_proc18hardware6finish' in text)==(version in ('P1A3','P1A4','P1A5','P1A6','P1A7','P1A8','P1A9')), 'proc1 stopped-scan path/version mismatch'
    # Profiler hooks add 8 bytes of C1 locals (608 of the 2048-byte guarded C1 stack).
    reservation=(r'sub\.w\s+r13, r13, #608\b' if profile else r'sub\.w\s+r13, r13, #600\b') if version in ('P1A4','P1A5','P1A6','P1A7','P1A8','P1A9') else r'sub\s+sp, #'+str(frame)+r'\b'
    assert re.search(r'push\s+\{r7, r14\}',body[0]) and re.search(reservation,body[2]), 'proc1 body frame changed; review stack budget'
    pure={'__aeabi_memclr','__aeabi_memset','__aeabi_memcpy','memset','memcpy','__aeabi_uldivmod','__udivmoddi4'}
    if version in ('P1A4','P1A5','P1A6','P1A7','P1A8','P1A9'): pure.add('memcmp')
    # P1A4 helpers copy only local controller/stack data; pin reviewed opcodes.
    command_outlines={
        'OUTLINED_FUNCTION_0':'f6a57784e8c5cfc490c4a017fd27e445a2b6e794f19ea1279658e6aeb4e7dde6',
        'OUTLINED_FUNCTION_10':'340ebda477ab3c4e50440f14691c2fdc06e53e15e80a56e0ab0a67a2266f779c',
        'OUTLINED_FUNCTION_12':'41957e0257b7c327de9a07e8be5fdba844255afef4a7cdee05737154dcb0494c',
        'OUTLINED_FUNCTION_13':'872896e7bde2e09e064a164abb3e6b9911e8dfec66e5c050568ca67f714f3816',
    }
    if version=='P1A5':
        command_outlines={
            'OUTLINED_FUNCTION_9':'340ebda477ab3c4e50440f14691c2fdc06e53e15e80a56e0ab0a67a2266f779c',
            'OUTLINED_FUNCTION_11':'d7d0ec79e2ece34b691b317a7124f6a173e217335918ae22ac4bb7e410dfb003',
        }
    if gpio_irq:
        # Finite Q candidate only: reviewed instructions AND literal pools.
        irq_review={
            'AmpProc1GpioIrq':('2adb71acdc95d495a760b18ed22b3453663e1e08d52c0cd52695aa4afe2a1be9',
                'e00ff01c 2000c750 400d0118 400d0114 400d0048 400ac024 400d3114 400d204c'),
            'AmpProc1Fault':('fc6848c8dc66079c77fcc0067391b5ae1372ba84c3c1ce7fee014839291b97a9',
                'e00ff01c 2000be00 e000ed08 2000c748 400d3110 400d3114 e000e180 2000ba00 400e3000'),
            'AmpProc1Body':('2259e49fce31456da57023951e61cdece970657442669963b6f94cd6b6b953f4',
                'e000ed08 e00ff01c 36413150 400e0000 2000bda0 2000a304 2000ba00 2000be00 2000bf48 2000c748 2000bd50 31525443 2000ba80 0001869e 01312cff'),
            'amp_proc18gpio_irq4stop':('c6dc10bfb1a6249117d85ebb6ccaea85a2cd5b2697dcc653cf400aeb8bd75e70',
                '2000c7d0 2000c750 e000e100 400d304c 400d204c 400d004c'),
            'amp_proc18gpio_irq5force':('9169fc3a31e5cf6d5368eada665baa6e90ef8578af0382c8e5351b500d605714',
                '2000c750 0001869f 400d2114'),
            'amp_proc18gpio_irq5start':('160f05a813d55093b4bc5d14babad577c185f910855d3c54b409bcb50c9d3c95',
                'e00ff01c e000e100 400d0110 2000c7d0 400e0004 400d304c cff00000 400d204c 32463147 2000c750'),
            'PolymorphicIter':('3f2618ce582d53390f6dfbc683ed47a6cda98ea998db32c73c109bcb785a2fcc',''),
        }
        command_outlines={
            'OUTLINED_FUNCTION_6':'667dd264f3f4ca8401336975ca4016d1108c06091ebeaf46dab1a7450beec31f',
            'OUTLINED_FUNCTION_8':'e57608f17ccb696c759fa3636ccd05e6f119ea283d306f73d9b390844aeb4558',
            'OUTLINED_FUNCTION_10':'340ebda477ab3c4e50440f14691c2fdc06e53e15e80a56e0ab0a67a2266f779c',
            'OUTLINED_FUNCTION_11':'fb8339ba69dd454f8f431a47e6ddb8e47c3feb576f95b4a7908dfc885b1ce824',
        }
        if version in ('P1A7','P1A8','P1A9'):
            # R: lazy local trace/edge_us=0; identical GPIO ISR/mask/literal scope.
            irq_review.update({
                'AmpProc1Body':('8b9c305fb81b8b85ff3b33fd399764beca32f87a7eaf9d95e7ba647f821fb67e',
                    'e000ed08 e00ff01c 37413150 400e0000 2000bda0 2000a304 2000ba00 2000be00 2000bf48 2000c748 2000bd50 31525443 2000ba80 0001869e 01312cff'),
                'amp_proc18gpio_irq5force':('42d9176a60dbc0e29e7a3b999be94f2d36f08f6adee2eb06a62ca10fd293e795',
                    '2000c750 0001869f 400d2114'),
                'amp_proc18gpio_irq5start':('a30228553a3114cdb065a72bc80bcc10e0bbec2e75cb80d3d25631f6525ae550',
                    'e00ff01c e000e100 400d0110 2000c7d0 400e0004 400d304c cff00000 400d204c 32463147 2000c750'),
                'PolymorphicIter':('eb123f5f4cb7863f0d44bd35a9f326ce9dd7c18a5f41a7791715a1e247a4a4b1',''),
            })
            command_outlines={
                'OUTLINED_FUNCTION_5':'667dd264f3f4ca8401336975ca4016d1108c06091ebeaf46dab1a7450beec31f',
                'OUTLINED_FUNCTION_9':'340ebda477ab3c4e50440f14691c2fdc06e53e15e80a56e0ab0a67a2266f779c',
            }
            if version in ('P1A8','P1A9'):
                # S: C0 post-copy query clock; all27 normalized C1 bodies unchanged.
                irq_review.update({
                    'AmpProc1Body':('7740a5240f0d44ef1678f1985573cafa21a1d4053ca93a75c03280ed1a214c84',
                        'e000ed08 e00ff01c 38413150 400e0000 2000bda0 2000a304 2000ba00 2000be00 2000bf48 2000c748 2000bd50 31525443 2000ba80 0001869e 01312cff'),
                    'PolymorphicIter':('493d2ca89b426e39e90fcee2723f4166726c33e16084663738ee72ec4e5760da',''),
                })
            if version=='P1A9':
                # T: normal due path skips division; executed-cycle guard countdown.
                irq_review.update({
                    'AmpProc1Body':('4591c91689e260a7a33158f0d8be34a678c7cd95af2c23e4e11438a5c3ab7b09',
                        'e000ed08 e00ff01c 39413150 400e0000 2000bda0 2000a304 2000ba00 2000be00 2000bf48 2000c748 2000bd50 31525443 2000ba80 0001869e 01312cff'),
                    'amp_proc18gpio_irq5force':('df65bcbbcafe33f9dced3a2035b0ed5e239d7a95a5b25677ee9b1dc3471da61e',
                        '2000c750 0001869f 400d2114'),
                    'amp_proc18gpio_irq5start':('9973d3f646e645fe64c906cdc5c03aa3cbe21763830b45c436c443d2019f906e',
                        'e00ff01c e000e100 400d0110 2000c7d0 400e0004 400d304c cff00000 400d204c 32463147 2000c750'),
                    'PolymorphicIter':('e502c6ac5f7be7e4b903482b74488c726a5056d0dd95a34e50ac8d999a27d536',''),
                })
            if observation:
                # V: all27 C1 bodies match T after relocation/outline renumbering;
                # status()'s relocated48-byte readonly table is byte-identical.
                irq_review.update({
                    'AmpProc1Body':('7224be25315c18aff18887bac5b53bf8ee64ab0f77d77c0cd5f8297447f4afc6',
                        irq_review['AmpProc1Body'][1]),
                    'amp_proc18gpio_irq4stop':('739af35f4422ad9c5b708413a45a98b0a03f2cc62de3557003af1c898411e7b1',
                        irq_review['amp_proc18gpio_irq4stop'][1]),
                    'amp_proc18gpio_irq5force':('ed1682dca0c0ecf9d73cb96eda880f8154c7d59f8117fa82fe977124afc2f213',
                        irq_review['amp_proc18gpio_irq5force'][1]),
                    'amp_proc18gpio_irq5start':('cb682269d99201868e73dfeb3575f7c83b85cbc8bad25f7df871356100dd1cd4',
                        irq_review['amp_proc18gpio_irq5start'][1]),
                    'PolymorphicIter':('8f8fdd1f81263dca4a160818728ed2afc1e504855f3c123017fbc0eb0d64725f',''),
                })
                command_outlines={
                    'OUTLINED_FUNCTION_4':'667dd264f3f4ca8401336975ca4016d1108c06091ebeaf46dab1a7450beec31f',
                    'OUTLINED_FUNCTION_8':'340ebda477ab3c4e50440f14691c2fdc06e53e15e80a56e0ab0a67a2266f779c',
                }
                if cold_irq:
                    # X: same27 C1 bodies; readonly status table bytes unchanged.
                    for key, digest in {
                        'AmpProc1Body':'c437a91dfc53c19b1d62c47e26e0e4c5d469f608d0b9e852e9a1de268ad149eb',
                        'amp_proc18gpio_irq5force':'da56fcadf4bfd6140ad263883825407ac4b7ff81ee409644f6d71a5c56973050',
                        'amp_proc18gpio_irq5start':'4878eabc0e6aaa9245a4918ff2b29e231d209888e0319b46a6c022ae5e448158',
                        'PolymorphicIter':'9857c2d554e03a4a2ea92e4979d7035bb2cf39b7b336390e93bb30324d97d168',
                    }.items():
                        irq_review[key]=(digest,irq_review[key][1])
            elif peer_quiesce:
                # V default: C0 prefix drain only; all27 C1 bodies match T after
                # relocation, including the separately checked status table.
                irq_review.update({
                    'AmpProc1Body':('c4324f7e22b9c180a8de3c004d22117f29b904f7c17d8e663eb1ec6f116f1508',irq_review['AmpProc1Body'][1]),
                    'PolymorphicIter':('0b82656cedb51149bcc6fcae5905e1d8e8106efbbe3e8310ddf50d1b619e19c5',''),
                })
                command_outlines={
                    'OUTLINED_FUNCTION_4':'667dd264f3f4ca8401336975ca4016d1108c06091ebeaf46dab1a7450beec31f',
                    'OUTLINED_FUNCTION_9':'340ebda477ab3c4e50440f14691c2fdc06e53e15e80a56e0ab0a67a2266f779c',
                }
        assert 'AmpProc1GpioIrq' in named, 'GPIO IRQ handler missing'
        handler=named['AmpProc1GpioIrq'][1]
        assert re.search(r'\bcpsid\s+i\b',handler.splitlines()[0]), 'GPIO IRQ must mask first'
        assert not re.search(r'\b(?:push|pop|stmdb|ldmia|bl|blx)\b|\br(?:[4-9]|10|11|13)\b',handler), 'GPIO IRQ must use only hardware-stacked registers'
    todo=[named['AmpProc1Body'][0]];seen=set()
    if gpio_irq: todo.append(named['AmpProc1GpioIrq'][0])
    while todo:
        address=todo.pop()
        if address in seen:continue
        seen.add(address);name,body=blocks[address]
        allowed=name=='AmpProc1Body' or name in pure or 'amp_proc18hardware' in name or 'compiler_builtins' in name
        timer='freertos_r18timesync3now' in name
        outline=name.startswith('OUTLINED_FUNCTION_')
        gate=version in ('P1A4','P1A5','P1A6','P1A7','P1A8','P1A9') and '7rp1_abi3amp11CommandGate6accept' in name
        gpio=version in ('P1A5','P1A6','P1A7','P1A8','P1A9') and '7rp1_hal4gpio46ConfiguredPin' in name and ('8set_high' in name or '7set_low' in name)
        irq= gpio_irq and any(key in name for key in irq_review)
        # Opt-in profiler: its hooks change the reviewed body; pin is replaced by the recorded hash.
        profiled=profile and (name=='AmpProc1Body' or 'amp_proc17profile' in name)
        doorbell_code=doorbell and name in {'AmpProc1DoorbellStart','AmpProc1DoorbellIrq'}
        assert allowed or timer or outline or gate or gpio or irq or profiled or doorbell_code, 'proc1 calls unadmitted service/driver '+name
        if doorbell_code:
            # Written review: docs/doorbell-disassembly-review.md. No general indirect-call exception.
            code='\n'.join(line.split(':',1)[1].split('@',1)[0].strip() for line in body.splitlines()
                if re.match(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]{4}\s+)+[a-z]',line)).encode()
            assert hashlib.sha256(code).hexdigest()=={
                'AmpProc1DoorbellStart':'26e47b5b2a68fe6f5f3513ff0cb768b76d814d8a5752b66e6860ba48e75d0c63',
                'AmpProc1DoorbellIrq':'f50dc120eb94adc7193c3ea819ec5f41e09ab125c34cb41eec672ad78f4d7c59',
            }[name], 'doorbell reviewed opcodes changed'
        # Profile-only exception: probe() times one fixed NOP snippet in proc-local ISRAM and
        # the same snippet in free shared SRAM; no other indirect call is admitted.
        snippet_call=(profile and 'amp_proc17profile5probe' in name
            and {'10000001','2000f680'} <= set(re.findall(r'\.word\s+0x([0-9a-f]+)',body))
            and all(op.strip() in ('r0','r1','r2','r3','r4','r5','r6','r8','r9','r10','r11') for op in re.findall(r'\bblx\s+([^\n@]+)',body)))
        assert snippet_call or not re.search(r'\bblx\b',body), 'proc1 indirect call'
        if irq and not profiled and profile:
            # Profile build: Rust symbol hashes differ with the feature set, so compare the
            # reviewed IRQ helpers with the sealed X image instead of the pinned hashes.
            # Position-independent compare: instruction addresses, branch encodings/targets and
            # outline numbering move with the profiler; literal .word values are kept verbatim.
            def norm(t):
                t=re.sub(r'17h[0-9a-f]{16}E','17hE',t)
                t=re.sub(r'OUTLINED_FUNCTION_\d+','OUTLINED_FUNCTION_N',t)
                out=[]
                for line in t.splitlines():
                    m=re.match(r'^\s*[0-9a-f]+:\s+((?:[0-9a-f]{4}\s+)+)(.*)$',line)
                    if not m:
                        if line.strip():out.append(line.strip())
                        continue
                    ins=m[2].split('@',1)[0].strip()
                    if re.match(r'(?:b|bl|cbz|cbnz|b[a-z]{2})(?:\.[nw])?\s',ins):
                        out.append(re.sub(r'\b[0-9a-f]{8}\b','ADDR',ins))
                    elif ins.startswith('.word'):
                        out.append(ins)
                    else:
                        out.append(m[1].strip()+' '+ins)
                return '\n'.join(out)
            reference=profile_reference()
            ref=[b for n,b in reference.items() if norm(n)==norm(name)]
            # Generic monomorphizations share a de-hashed name; one must match exactly.
            if name=='AmpProc1Fault' and os.environ.get('RP1_AMP_PROC1_LOCAL_STACK')=='1':
                # AmpProc1Entry shares this literal pool: the DSRAM-stack diagnostic swaps the
                # shared stack-top literal for the DSRAM bounds, shifting pc-relative offsets.
                # Compare each instruction with pc-relative loads resolved to the loaded word,
                # and require the pool to differ only by that swap.
                def resolved(t):
                    words={int(a,16):w for a,w in re.findall(r'^\s*([0-9a-f]+):\s+[0-9a-f]{8}\s+\.word\s+0x([0-9a-f]+)',t,re.M)}
                    ops=[]
                    for line in t.splitlines():
                        m=re.match(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]{4}\s+)+([a-z].*)$',line)
                        if not m or m[1].startswith('.word') or m[1].strip()=='movs\tr0, r0':continue
                        ins=m[1]
                        lit=re.search(r'\[(?:r15|pc), #\d+\]\s*@ \(([0-9a-f]+)',ins)
                        if lit:ins=re.sub(r'\[(?:r15|pc), #\d+\].*','=0x'+words[int(lit[1],16)],ins)
                        ins=re.sub(r'\b[0-9a-f]{8} <[^>]*>','ADDR',ins.split('@',1)[0] if not lit else ins)
                        ops.append(ins.strip())
                    return ops,sorted(words.values())
                cand_ops,cand_words=resolved(body)
                if os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1':
                    # Relaunch diagnostic: Fault first saves CFSR/HFSR to 0x2000f6f8 (exact 7-instruction prefix), and
                    # AmpProc1Entry's added record code puts its own words into the shared pool. Nothing else may change.
                    prefix=['ldr\tr0, =0x2000f6f8','ldr\tr1, =0xe000ed28','ldr\tr2, [r1, #0]','str\tr2, [r0, #0]',
                            'ldr\tr2, [r1, #4]','str\tr2, [r0, #4]','dsb\tsy']
                    assert cand_ops[1:8]==prefix, 'relaunch fault prefix changed'
                    cand_ops=cand_ops[:1]+cand_ops[8:]
                    extra={'2000f640','2000f6f8','324e5552','400ac028','e000ed04','e000ed28'}
                    cand_words=sorted(w for w in cand_words if w not in extra)
                assert any(resolved(r)[0]==cand_ops and sorted([w for w in resolved(r)[1] if w!='2000c748']+['10002000','10002800'])==cand_words for r in ref), \
                    'profile build changed reviewed GPIO IRQ code: '+name
            else:
                assert ref and any(norm(r)==norm(body) for r in ref), 'profile build changed reviewed GPIO IRQ code: '+name
        elif irq and not profiled:
            code='\n'.join(line.split(':',1)[1].split('@',1)[0].strip() for line in body.splitlines()
                if re.match(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]{4}\s+)+[a-z]',line)).encode()
            pinned=[value for key,value in irq_review.items() if key in name]
            assert len(pinned)==1 and hashlib.sha256(code).hexdigest()==pinned[0][0], 'GPIO IRQ reviewed opcodes changed: '+name
            assert re.findall(r'\.word\s+0x([0-9a-f]+)',body)==pinned[0][1].split(), 'GPIO IRQ reviewed literal pool changed'
        if timer:
            assert '0x400ac024' in body and re.search(r'movs\s+r2, #5\b',body), 'proc1 readonly bounded timer changed'
        if gpio:
            code=b''.join(bytes.fromhex(x) for x in re.findall(r'^\s*[0-9a-f]+:\s+((?:[0-9a-f]{4}\s+)+)',body,re.M))
            assert hashlib.sha256(code).hexdigest()=='8d6745a2ce6d69764ceeeff5561019ead942485536705da5a41aa1c5ea845fc1', 'GPIO22 write opcodes changed'
            assert re.findall(r'\.word\s+0x([0-9a-f]+)',body)==['400e2000' if '8set_high' in name else '400e3000'], 'GPIO22 atomic alias changed'
            assert re.search(r'mov\.w\s+r1, #4194304\b',body), 'GPIO22 only mask changed'
        if version in ('P1A5','P1A6','P1A7','P1A8','P1A9') and 'amp_proc18hardware12clear_output' in name:
            code=b''.join(bytes.fromhex(x) for x in re.findall(r'^\s*[0-9a-f]+:\s+((?:[0-9a-f]{4}\s+)+)',body,re.M))
            assert hashlib.sha256(code).hexdigest()=='e707d746f287ea53c29a0053af324d628bbd878b140b0db060b509424473a0b9', 'GPIO22 stop/readback opcodes changed'
        if outline and version in ('P1A4','P1A5','P1A6','P1A7','P1A8','P1A9'):
            code=b''.join(bytes.fromhex(x) for x in re.findall(r'^\s*[0-9a-f]+:\s+((?:[0-9a-f]{4}\s+)+)',body,re.M))
            if profile:
                # The profiler reshuffles machine outlining; admit only plain ALU/load/store/branch
                # helpers (callees and literals are still walked and MMIO-checked below).
                safe={'movs','mov','adds','add','adcs','subs','sub','sbcs','ldr','ldrd','ldrb','ldrh','str','strd',
                      'strb','strh','orr','orrs','bic','bics','and','ands','lsl','lsls','lsr','lsrs','uxtb','uxth',
                      'cmp','cmn','ldmia','stmia','pop','push','bx','b','beq','bne','bmi','bpl','bhi','bls','bcs','bcc','bge','blt','bgt','ble','it'}
                for mnemonic in re.findall(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]{4}\s+)+([a-z][a-z0-9.]*)',body,re.M):
                    assert mnemonic.split('.')[0] in safe, 'profile build outlined helper uses '+mnemonic
            else:
                assert hashlib.sha256(code).hexdigest()==command_outlines.get(name), 'unreviewed command outlined helper'
        elif outline:
            instructions=re.findall(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]+\s+)+([a-z][a-z0-9.]*)\s+([^\n]+)',body,re.M)
            for mnemonic,operands in instructions:
                assert mnemonic.split('.')[0] in {'movs','adds','adcs','bx','ldr','pop'}, 'unreviewed proc1 outlined helper'
                if mnemonic.startswith('ldr'):assert re.fullmatch(r'r11, \[r13\], #4',operands.strip()), 'outlined load must use only stack'
                if mnemonic=='bx':assert operands.strip()=='r14', 'outlined indirect branch must return'
        for word in re.findall(r'\.word\s+0x([0-9a-f]+)',body):
            value=int(word,16)
            assert not 0x81000000<=value<0x81010000, 'proc1 references shared DDR window'
            if 0x40000000<=value<0x60000000:
                permitted={0x400ac024}
                if version in ('P1A5','P1A6','P1A7','P1A8','P1A9'):
                    if name=='AmpProc1Body':permitted.add(0x400e0000)
                    if name=='AmpProc1Fault' and gpio_irq:permitted.add(0x400e3000)
                    # Relaunch diagnostic: AmpProc1Entry's raw-timer timestamp literal lands in the shared Entry/Fault pool.
                    if name=='AmpProc1Fault' and os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1':permitted.add(0x400ac028)
                    if 'amp_proc18hardware12clear_output' in name:permitted.update({0x400e3000,0x400e0000})
                    if gpio:permitted.add(0x400e2000 if '8set_high' in name else 0x400e3000)
                if profiled:permitted.update({0x400ac028,0x400e0000})
                if profiled and 'profile10lib_probes17h' in name and os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1':permitted.update({0x40014018,0x40017000,0x40400380})
                if irq:
                    permitted.update({0x400d0110,0x400d0114,0x400d0118,0x400d0048,
                        0x400d004c,0x400d204c,0x400d304c,0x400d2114,0x400d3110,0x400d3114,0x400e0004})
                if doorbell_code:permitted.update({0x400ac028,0x400d004c,0x400d0118,0x400d2110,0x400d3110,0x400d3114})
                assert value in permitted, 'proc1 references peripheral MMIO'
            if 0xe0000000<=value<0xe0100000:
                assert value in ({0xe000ed08,0xe00ff01c}|({0xe000e100,0xe000e180,0xe000e200,0xe000e280} if irq else set())
                    |({0xe000e010,0xe000e014,0xe000e018,0xe000e01c,0xe000e100} if profiled else set())
                    |({0xe000e104,0xe000e200,0xe000e204,0xe000e280,0xe000e284} if profiled and 'lib_probes' in name and os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1' else set())
                    |({0xe000ed04,0xe000ed28} if name=='AmpProc1Fault' and os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1' else set())
                    |({0xe000e100,0xe000e180,0xe000e280} if doorbell_code else set())), 'proc1 references unadmitted local MMIO'
        for branch in re.finditer(r'\b(?:bl|b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(?:\.[nw])?|cbnz|cbz)\s+(?:r\d+,\s*)?([0-9a-f]+)\s+<',body):
            target=int(branch[1],16)
            if target in blocks and target!=address:todo.append(target)
    status_table=None
    if observation or peer_quiesce:
        status=[b for n,(_,b) in named.items() if 'amp_proc18hardware6status' in n]
        assert len(status)==1
        address=0x2000a0d8 if cold_irq else 0x2000a0e4 if observation else 0x2000986c
        if profile:
            # Moves with .text in the profiler build; contents are still hash-checked by the caller.
            words=re.findall(r'\.word\s+0x([0-9a-f]+)',status[0])
            assert len(words)==1, 'C1 status table literal'
            address=int(words[0],16)
        assert re.findall(r'\.word\s+0x([0-9a-f]+)',status[0])==[f'{address:08x}'], 'C1 status table address changed'
        status_table=dict(address=address,bytes=48,sha256='903e707369259ced4c6358b9931f74340df8f39d1601c58f75ae12f0a5e6f63f')
    return {'reachable_functions':sorted(blocks[a][0] for a in seen),'body_frame_bytes':frame+(16 if profile else 8),'telemetry_version':version,
            'status_table':status_table,'cold_irq_admission':cold_irq,
            'irq_exception_stack_bytes_max':40 if gpio_irq else 0,
            'scope':'static codegen only; not a whole-stack bound or measured timing',
            **({'profile_body_sha256':hashlib.sha256(body_code).hexdigest()} if profile else {})}


def check(path, local_monitor=False, require_scmi=False, require_ddr_diagnostics=False, amp_control=False, amp_proc1=False, amp_proc1_gpio_irq=False, amp_observation=False):
    assert not amp_proc1 or amp_control, 'proc1 phase1 requires explicit AMP control admission'
    assert not amp_proc1_gpio_irq or amp_proc1, 'GPIO IRQ requires explicit proc1 admission'
    assert not amp_observation or amp_proc1_gpio_irq, 'collector requires explicit GPIO IRQ proc1 admission'
    data = Path(path).read_bytes()
    assert data[:7] == b'\x7fELF\x01\x01\x01', 'ELF32 little endian required'
    header = struct.unpack_from('<16sHHIIIIIHHHHHH', data)
    assert header[2] == 40, 'ARM ELF required'
    entry, phoff, phsize, phcount = header[4], header[5], header[9], header[10]
    loads = []
    nulls = []
    # Diagnostic-only C1 profiler build (freertos-amp-proc1-profile); never a production image.
    profile = os.environ.get('RP1_AMP_PROC1_PROFILE') == '1'
    doorbell = profile and os.environ.get('RP1_AMP_PROC1_LOCAL_STACK')=='1' and os.environ.get('RP1_AMP_PROC1_NO_HOOKS')=='1'
    assert not profile or amp_observation, 'profiler requires the OBS1 C1 build'
    profile_loads = []
    for index in range(phcount):
        kind, off, va, pa, filesz, memsz, flags, align = struct.unpack_from('<8I', data, phoff + index * phsize)
        if kind != 1:
            if kind == 0:
                nulls.append((off, va, pa, filesz, memsz, flags, align))
            continue
        assert va == pa, 'loader copy contract requires VMA == p_paddr'
        if profile and pa >= 0x2000e000:
            # Opt-in C1 profiler: one R+X file-backed segment above the fixed MSP top only.
            assert pa == 0x2000f000 and 0 < memsz == filesz <= 0x600 and flags == 5, 'profile segment shape'
            assert not profile_loads, 'more than one profile segment'
            profile_loads.append((pa, pa + memsz))
        else:
            assert 0x20000000 <= pa <= pa + memsz <= 0x2000e000, 'load overlaps reserved SRAM'
        assert filesz <= memsz and off + filesz <= len(data)
        loads.append((pa, pa + memsz, off, filesz))
    loads.sort()
    assert loads and loads[0][0] == 0x20000000
    assert all(a[1] <= b[0] for a, b in zip(loads, loads[1:])), 'overlapping PT_LOAD'
    assert any(a <= (entry & ~1) < b for a, b, _, _ in loads) and entry & 1
    vector = struct.unpack_from('<80I', data, loads[0][2])
    assert vector[0] == 0x2000f000, 'VTOR[0] must equal reserved MSP top'
    nm = subprocess.check_output(['arm-none-eabi-nm', '-n', str(path)], text=True)
    symbols = {}
    for line in nm.splitlines():
        fields = line.split()
        if len(fields) == 3:
            symbols[fields[2]] = int(fields[0], 16)
    required_vectors = {1:'Reset', **{n:'RP1RtosFault' for n in range(2, 7)},
                        11:'vPortSVCHandler', 14:'xPortPendSVHandler', 15:'xPortSysTickHandler'}
    for index, name in {42:'TIMER0_ALARM0_IRQ26_CANDIDATE_IRQHandler',
                        35:'SPI0_IRQHandler', 24:'I2C1_IRQHandler', 41:'UART0_IRQHandler',
                        73:'RP1_SCMI_IRQHandler'}.items():
        if name in symbols: required_vectors[index] = name
    for index, name in required_vectors.items():
        assert vector[index] == symbols[name] | 1, f'vector {index} must point directly to {name}'
    assert symbols['__ebss'] <= 0x2000e000
    shoff, shsize, shcount, shstr = header[6], header[11], header[12], header[13]
    sections = [struct.unpack_from('<10I', data, shoff+i*shsize) for i in range(shcount)]
    strings = sections[shstr]
    names = data[strings[4]:strings[4]+strings[5]]
    diagnostic = [s for s in sections if names[s[0]:].split(b'\0',1)[0] == b'.ddr_diagnostics_text']
    assert not require_ddr_diagnostics or diagnostic, 'DDR diagnostic code missing'
    if diagnostic:
        assert len(diagnostic) == 1
        _, kind, flags, addr, off, size, _, _, _, _ = diagnostic[0]
        assert kind == 1 and flags == 6 and off+size <= len(data), 'diagnostic code must be loaded executable bytes'
        assert addr == 0x2000d290 and 0 < size <= 0x2000d7c0-addr, 'diagnostic code overlaps SCMI'
        profiles = [s for s in sections if names[s[0]:].split(b'\0',1)[0] == b'.rp1_clock_profile']
        assert len(profiles) == 1 and symbols['__ebss'] <= profiles[0][3]
        assert profiles[0][3]+profiles[0][5] <= addr, 'diagnostic code overlaps clock profile'
        assert any(a <= addr < addr+size <= a+n for a,_,_,n in loads), 'diagnostic code is not loaded'
        sizes = subprocess.check_output(['arm-none-eabi-nm','-S',str(path)],text=True)
        rows = [l.split() for l in sizes.splitlines() if l.endswith(' task_stacks')]
        assert len(rows) == 1 and int(rows[0][1],16) == (1024 if amp_control else 2304)*4, 'diagnostic task pool allocation'
    assert local_monitor == ('__local_monitor_stack_start' in symbols), 'local stack requires explicit admission'
    if local_monitor:
        assert symbols['__local_monitor_stack_start'] == 0x10003800
        assert symbols['__local_monitor_stack_end'] == 0x10004000
        assert len(nulls) == 1
        off, va, pa, filesz, memsz, flags, align = nulls[0]
        assert (va, pa, filesz, memsz, flags, align) == (0x10003800, 0x10003800, 0, 2048, 6, 8)
        assert off % 8 == 0 and off <= len(data), 'PT_NULL still must pass the pinned parser'
        shoff, shsize, shcount, shstr = header[6], header[11], header[12], header[13]
        sections = [struct.unpack_from('<10I', data, shoff+i*shsize) for i in range(shcount)]
        strings = sections[shstr]
        names = data[strings[4]:strings[4]+strings[5]]
        local = []
        for section in sections:
            name, kind, flags, addr, _, size, _, _, align, _ = section
            if not flags & 2 or size == 0:
                continue
            label = names[name:].split(b'\0', 1)[0]
            if label == b'.local_monitor_stack':
                assert (kind, flags, addr, size, align) == (8, 3, 0x10003800, 2048, 8)
                local.append(section)
            else:
                assert any(a <= addr <= addr+size <= b for a,b,_,_ in loads), 'unowned allocated section'
        assert len(local) == 1
        sizes = subprocess.check_output(['arm-none-eabi-nm', '-S', str(path)], text=True)
        for name, address, size in [('monitor_stack', 0x10003800, 2048), ('task_stacks', None, 7168)]:
            rows = [line.split() for line in sizes.splitlines() if line.endswith(' '+name)]
            assert len(rows) == 1 and int(rows[0][1], 16) == size, 'unchanged total task stack budget'
            assert address is None or int(rows[0][0], 16) == address
    assert not subprocess.check_output(['arm-none-eabi-nm', '-u', str(path)]), 'undefined symbols'
    dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', '-M', 'reg-names-raw', str(path)], text=True)
    exclusive_scope = dis
    assert not re.search(r'\b(?:ldrex\w*|strex\w*|clrex)\b', exclusive_scope), 'unproven exclusive instruction linked'
    assert not any(name in symbols for name in ['pvPortMalloc', 'vPortFree', 'malloc', '_Unwind_Resume'])
    scmi = None
    assert not require_scmi or 'RP1_SCMI_IRQHandler' in symbols, 'SCMI handler missing'
    if 'RP1_SCMI_IRQHandler' in symbols:
        from scmi_elf_layout import read_layout
        layout = read_layout(path)
        sizes = subprocess.check_output(['arm-none-eabi-nm', '-S', str(path)], text=True)
        rows = [line.split() for line in sizes.splitlines() if line.endswith(' RP1_SCMI_TELEMETRY')]
        assert len(rows) == 1 and int(rows[0][1], 16) == 27*4, 'SCMI telemetry ABI size'
        address = int(rows[0][0], 16)
        assert symbols['__sbss'] <= address < address+108 <= symbols['__ebss'], 'SCMI telemetry must be private .bss'
        assert address+108 <= layout['local_start'], 'telemetry overlaps SCMI shared channel'
        scmi = dict(candidate_irq=57, vector=73, telemetry=address, telemetry_bytes=108,
            shmem=layout['local_start'], profile_sha256=layout['profile_sha256'],
            scope='cold readonly IRQ commissioning; HW route and Linux handoff OPEN')
    control_contract = [s for s in sections if names[s[0]:].split(b'\0',1)[0] == b'.amp_control_contract']
    assert amp_control == bool(control_contract), 'AMP control requires separate explicit admission'
    control = None
    if amp_control:
        assert len(control_contract) == 1
        _,kind,flags,addr,off,size,_,_,_,_ = control_contract[0]
        # Rust #[used] sets SHF_GNU_RETAIN; both forms must remain read-only data.
        assert kind == 1 and flags in (2, 0x200002) and size == 24 and off+size <= len(data), 'AMP control contract bytes'
        assert struct.unpack_from('<6I',data,off) == (int.from_bytes(b'AG01','little'),1,2,1024,200,int(amp_proc1)), 'AMP control contract contents'
        assert addr == symbols['RP1_AMP_CONTROL_CONTRACT'], 'AMP control contract symbol mismatch'
        assert symbols['__ebss'] <= addr and addr+size <= 0x2000d7c0, 'AMP control contract overlap'
        assert any(a <= addr < addr+size <= a+n and off == fileoff+addr-a
                   for a,_,fileoff,n in loads), 'AMP control contract not loaded at its section address'
        if diagnostic:
            assert profiles[0][3]+profiles[0][5] <= addr and addr+size <= diagnostic[0][3], 'AMP control contract overlaps profile/diagnostic code'
        sizes = subprocess.check_output(['arm-none-eabi-nm','-S',str(path)],text=True)
        rows = [line.split() for line in sizes.splitlines() if line.endswith(' task_stacks')]
        assert len(rows) == 1 and int(rows[0][1],16) == 1024*4, 'two-task AMP stack allocation'
        assert symbols['__sbss'] <= int(rows[0][0],16) < int(rows[0][0],16)+4096 <= symbols['__ebss'], 'AMP task pool outside initialized BSS'
        assert not any('freertos_r1' in name and any(s in name for s in ['4spin','8producer','8consumer','9mutex_low','10mutex_high']) for name in symbols), 'synthetic R1 workload linked in AMP service'
        assert 'Proc1RuntimeEntry' not in symbols, 'old peripheral-free worker is not AMP control'
        control = dict(tasks=2,task_stack_words=1024,contract_address=addr,
            target_period_us=200,phase=int(amp_proc1),scope='finite readonly proc1 probe; no control IRQ/actuator' if amp_proc1 else 'proc0 services only; proc1 control/IRQ is not connected')
    amp_sections = {names[s[0]:].split(b'\0',1)[0]:s for s in sections if names[s[0]:].split(b'\0',1)[0].startswith(b'.amp_proc1_')}
    assert amp_proc1 == bool(amp_sections), 'proc1 local regions require separate phase1 admission'
    amp_probe = None
    if amp_proc1:
        expected = {b'.amp_proc1_storage':(8,3,0x2000ba00,None),
                    b'.amp_proc1_vectors':(1,2,0x2000be00,320),
                    b'.amp_proc1_stack':(1,6,0x2000bf40,2064) if doorbell else (8,3,0x2000bf40,2064)}
        if doorbell:
            s=amp_sections[b'.amp_proc1_stack']
            assert hashlib.sha256(data[s[4]:s[4]+s[5]]).hexdigest()=='0cc39bd1ec9db6259203df70f0dc8dcba5af040c2f737bd3867daafb72227a68', 'doorbell source/literals changed; review again'
            assert symbols['AmpProc1DoorbellStart']&~1==0x2000bf48
            assert symbols['AmpProc1DoorbellIrq']&~1==0x2000c0f8 and symbols['AmpProc1DoorbellIrqEnd']==0x2000c160, 'doorbell copy bounds changed'
        if amp_proc1_gpio_irq:
            expected.update({b'.amp_proc1_irq_storage':(8,3,0x2000c750,130),
                             b'.amp_proc1_irq_text':(1,6,0x2000c7e0,None)})
        if profile:
            expected[b'.amp_proc1_profile_text']=(1,6,0x2000f000,None)
        assert amp_sections.keys() == expected.keys(), 'proc1 local section set'
        for name,(kind,flags,va,size) in expected.items():
            s=amp_sections[name]
            assert s[1]==kind and s[3]==va and s[2] in (flags,flags|0x200000), 'proc1 local section ownership/type'
            limit=0xab0 if name==b'.amp_proc1_irq_text' else 0x600 if name==b'.amp_proc1_profile_text' else 0x400
            assert 0<s[5]<=limit if size is None else s[5]==size, 'proc1 local section size'
            assert any(a<=va<va+s[5]<=b for a,b,_,_ in loads), 'proc1 region outside owned PT_LOAD'
        if profile:
            # Profiler scratch 0x2000f600 and results 0x2000f700 follow the code, below the RTOS record.
            assert profile_loads==[(0x2000f000,0x2000f000+amp_sections[b'.amp_proc1_profile_text'][5])], 'profile segment/section mismatch'
            prof = amp_sections[b'.amp_proc1_profile_text']
            assert hashlib.sha256(data[prof[4]:prof[4]+prof[5]]).hexdigest() == {('0','0'):'d5e38660b89ccd30b79425dbee6f96544a2d058b69c2910a59e22207eb9ce328',('1','0'):'d4bab8044ee4e92d29ebaf6192e0cbce8ec0187abeb38233eef12ce697fe34d8',('1','1'):'e7a150437ee21275bf8b3cf7e015233e44bf8018f353b849086a9e77a1ed0d3f'}.get((os.environ.get('RP1_AMP_PROC1_LOCAL_STACK','0'),os.environ.get('RP1_AMP_PROC1_NO_HOOKS','0'))), 'profile SysTick instructions changed; review again'
        assert control_contract[0][3]+24<=0x2000ba00, 'proc1 overlaps clock/proc0 contract'
        assert 0x2000c750<=diagnostic[0][3], 'proc1 stack overlaps DDR code'
        for name,value in [('__amp_proc1_vectors',0x2000be00),('__amp_proc1_guard_low',0x2000bf40),
                           ('__amp_proc1_stack_low',0x2000bf48),('__amp_proc1_stack_top',0x2000c748),
                           ('__amp_proc1_guard_high',0x2000c748)]:
            assert symbols[name]==value, 'proc1 region symbol mismatch'
        s=amp_sections[b'.amp_proc1_storage']
        assert s[3]<=symbols['RP1_AMP_PROC1_STATUS']<symbols['RP1_AMP_PROC1_STATUS']+128<=s[3]+s[5], 'proc1 status outside local storage'
        s=amp_sections[b'.amp_proc1_vectors']
        assert any(a<=s[3]<s[3]+320<=a+n and s[4]==fileoff+s[3]-a for a,_,fileoff,n in loads), 'proc1 vectors not file-backed at VMA'
        v=struct.unpack_from('<80I',data,s[4])
        assert v[:2]==(symbols['__amp_proc1_stack_top'],symbols['AmpProc1Entry']|1), 'proc1 initial MSP/entry'
        assert all(x==(symbols['AmpProc1GpioIrq']|1 if amp_proc1_gpio_irq and i==16 else symbols['AmpProc1Fault']|1)
                   for i,x in enumerate(v[2:],2)), 'proc1 owned IRQ/stackless fault vectors'
        for name in ['AmpProc1Body','AmpProc1Entry','AmpProc1Fault','rp1_control_session','rp1_control_retire']:
            assert name in symbols, 'proc1 hook/body missing'
        amp_probe=dict(phase=1,stack_bytes=2048,storage_bytes=amp_sections[b'.amp_proc1_storage'][5],
                       status=symbols['RP1_AMP_PROC1_STATUS'],vectors=0x2000be00,
                       entry=symbols['AmpProc1Entry'],body_entry=symbols['AmpProc1Body'],fault=symbols['AmpProc1Fault'],
                       scope='finite polling/no collector; hardware admission OPEN, no IRQ/sensor/actuator')
        amp_probe['codegen']=proc1_codegen(dis,amp_proc1_gpio_irq,amp_observation)
        table=amp_probe['codegen']['status_table']
        if table:
            addr=table['address'];size=table['bytes']
            tables=[data[off+addr-a:off+addr-a+size] for a,_,off,n in loads if a<=addr<addr+size<=a+n]
            assert len(tables)==1 and hashlib.sha256(tables[0]).hexdigest()==table['sha256'], 'C1 status table contents changed'
        if amp_probe['codegen']['telemetry_version'] in ('P1A5','P1A6','P1A7','P1A8','P1A9'):
            amp_probe['scope']='finite local command/GPIO22 output; no sensor IRQ, peripheral transfer, collector or realtime qualification'
        if amp_proc1_gpio_irq:
            s=amp_sections[b'.amp_proc1_irq_storage'];assert symbols['RP1_AMP_GPIO_IRQ_STATUS']==s[3], 'GPIO IRQ status ABI moved'
            s=amp_sections[b'.amp_proc1_irq_text'];assert s[3]+s[5]<=diagnostic[0][3], 'GPIO IRQ overlaps DDR code'
            assert symbols['AmpProc1GpioIrq']&~1==s[3], 'GPIO IRQ handler outside private section'
            assert struct.unpack_from('<6I',data,s[4]+s[5]-24)==(int.from_bytes(b'G1F2','little'),2,9,0,100000,200), 'GPIO IRQ finite contract changed'
            amp_probe['gpio_irq']=dict(status=0x2000c750,pin=9,nvic_irq=0,force_limit=100000,
                scope='software-force candidate; physical acquisition and end-to-end timing NOT_TESTED')
        if amp_probe['codegen']['telemetry_version'] in ('P1A4','P1A5','P1A6','P1A7','P1A8','P1A9'):
            s=amp_sections[b'.amp_proc1_storage']
            assert s[3]<=symbols['RP1_AMP_COMMAND_RECEIPT']<symbols['RP1_AMP_COMMAND_RECEIPT']+64<=s[3]+s[5], 'command receipt outside local storage'
            assert 'rp1_control_message' in symbols and s[5]==0x400, 'command ingress storage/hook missing'
    observation_sections={names[s[0]:].split(b'\0',1)[0]:s for s in sections
        if names[s[0]:].split(b'\0',1)[0].startswith(b'.amp_observation_')}
    assert amp_observation == bool(observation_sections), 'collector requires explicit observation admission'
    observation=None
    if amp_observation:
        assert observation_sections.keys()=={b'.amp_observation_state',b'.amp_observation_text',b'.amp_observation_codec'}, 'collector section set'
        state=observation_sections[b'.amp_observation_state']
        text=observation_sections[b'.amp_observation_text']
        codec=observation_sections[b'.amp_observation_codec']
        openamp=[s for s in sections if names[s[0]:].split(b'\0',1)[0]==b'.openamp_state']
        assert len(openamp)==1 and openamp[0][3]==0x2000db80, 'collector OpenAMP state location'
        assert state[3]==0x2000ce00 and state[3]+state[5]<=0x2000d290, 'collector state budget'
        assert amp_sections[b'.amp_proc1_irq_text'][3]+amp_sections[b'.amp_proc1_irq_text'][5]<=state[3], 'collector overlaps C1 IRQ code'
        assert text[3]==(state[3]+state[5]+3)&~3 and text[3]+text[5]<=0x2000d290, 'collector code budget'
        assert codec[3]==(openamp[0][3]+openamp[0][5]+3)&~3 and codec[3]+codec[5]<=0x2000e000, 'collector codec budget'
        for name,s in observation_sections.items():
            writable=name==b'.amp_observation_state'
            assert s[1]==(8 if writable else 1) and s[2] in ((3,0x200003) if writable else (6,0x200006)) and s[5]>0, 'collector section type/size'
            assert any(a<=s[3]<s[3]+s[5]<=b for a,b,_,_ in loads), 'collector section outside PT_LOAD'
            if not writable:
                assert s[4]+s[5]<=len(data) and any(a<=s[3]<s[3]+s[5]<=a+n and s[4]==off+s[3]-a
                    for a,_,off,n in loads), 'collector code file/VMA mapping'
        assert 'rp1_control_take_trace' in symbols, 'collector one-shot consumer handoff missing'
        guards=[(int(m[1],16),m[2]) for m in re.finditer(
            r'^([0-9a-f]+) <[^>]*cold_irq_admission5check[^>]*>:\n(.*?)(?=^[0-9a-f]+ <|\Z)',dis,re.M|re.S)]
        cold=amp_probe['codegen']['cold_irq_admission']
        assert len(guards)==int(cold), 'cold IRQ admission/codegen mismatch'
        if cold:
            address,body=guards[0]
            code='\n'.join(line.split(':',1)[1].split('@',1)[0].strip() for line in body.splitlines()
                if re.match(r'^\s*[0-9a-f]+:\s+(?:[0-9a-f]{4}\s+)+[a-z]',line)).encode()
            assert codec[3]<=address<address+44<=codec[3]+codec[5], 'cold IRQ guard outside owned codec'
            assert hashlib.sha256(code).hexdigest()=='40ea16c1e8a850c65024303dd0deb61b08c2fb9d24a927a84a68e16b018f73e5', 'cold IRQ guard opcodes changed'
            assert re.findall(r'\.word\s+0x([0-9a-f]+)',body)==['e000e100'], 'cold IRQ guard register base changed'
        observation=dict(state_bytes=state[5],copy_words_per_pass=4,frame_bytes=96,
            cold_irq_admission=(dict(address=guards[0][0],bytes=44,external_irq_banks=2,
                reads='IPSR, ISER0/1, IABR0/1; no NVIC writes; pending retained') if cold else None),
            regions={n.decode():[hex(s[3]),hex(s[3]+s[5])] for n,s in observation_sections.items()},
            scope='C0 local-SRAM collector; hardware stream and real-time integrity NOT_TESTED')
    # The separate assembly loops must retain literal R4-R11 comparisons and no calls.
    spin = [block for block in re.split(r'\n(?=[0-9a-f]+ <)', dis) if 'freertos_r1' in block.splitlines()[0] and '4spin' in block.splitlines()[0]]
    if not amp_control:
        assert len(spin) == 1 and not re.search(r'\bblx?\s', spin[0]), 'non-yielding spin task missing/contains call'
        for register in range(4, 12):
            assert re.search(rf'cmp(?:\.w)?\s+r{register},', spin[0]), f'r{register} test optimized away'
    proc1 = None
    if 'Proc1RuntimeEntry' in symbols:
        names = ['text', 'vectors', 'data', 'bss', 'lifecycle', 'request', 'response', 'fault']
        regions = [(symbols[f'__proc1_{n}_start'], symbols[f'__proc1_{n}_end']) for n in names]
        stack = (symbols['__proc1_guard_low'], symbols['__proc1_stack_end'])
        assert all(a <= b for a, b in regions)
        assert all(a[1] <= b[0] for a, b in zip(regions, regions[1:] + [stack]))
        assert stack[1] <= symbols['__sbss'] <= symbols['__ebss'] <= 0x2000e000
        assert stack[1] - stack[0] == 2064
        assert symbols['__proc1_stack_top'] - symbols['__proc1_stack_low'] == 2048
        assert symbols['__proc1_stack_low'] % 8 == 0
        va = symbols['__proc1_vectors_start']
        assert va % 512 == 0 and regions[1][1] - va == 320
        contents = [struct.unpack_from('<80I', data, off + va - a)
                    for a, _, off, size in loads if a <= va and va + 320 <= a + size]
        assert len(contents) == 1
        assert contents[0][0] == symbols['__proc1_stack_top']
        assert all(v == symbols['Proc1RuntimeFault'] | 1 for v in contents[0][1:])
        # Proc1 owns no RTOS state. Its worker must not call any external routine
        # (including panic/atomic helpers or the proc0 FreeRTOS bridge).
        worker_dis = subprocess.check_output(['arm-none-eabi-objdump', '-d', '-j', '.proc1_text', str(path)], text=True)
        assert not re.search(r'\bblx?\s', worker_dis), 'proc1 worker contains an external call'
        for line in worker_dis.splitlines():
            branch = re.search(r'\s(?:b(?:eq|ne|cs|cc|hs|lo|mi|pl|vs|vc|hi|ls|ge|lt|gt|le)?(?:\.[nw])?|cbnz|cbz)\s+(?:r\d+,\s*)?([0-9a-f]+)\s+<', line)
            if branch:
                assert regions[0][0] <= int(branch[1], 16) < regions[0][1], 'proc1 tail branch leaves owned code'
        # This pinned build pushes r7/lr and reserves20 bytes before its MSP
        # snapshot. Refuse a prologue change until explicitly reviewed.
        body = worker_dis.split('<Proc1RuntimeBody>:', 1)[1].strip().splitlines()
        assert re.search(r'push\s+\{r7, lr\}', body[0]), 'proc1 body push changed'
        assert re.search(r'sub\s+sp, #20\b', body[2]), 'proc1 body reservation changed'
        assert re.search(r'mrs\s+r0, MSP', body[4]), 'proc1 MSP snapshot moved'
        proc1 = {'sections': dict(zip(names, [[hex(a), hex(b)] for a, b in regions])),
                 'guarded_stack': [hex(v) for v in stack], 'stack_bytes': 2048,
                 'stack_low': symbols['__proc1_stack_low'], 'stack_top': symbols['__proc1_stack_top'],
                 'body_msp': symbols['__proc1_stack_top'] - 28, 'body_entry': symbols['Proc1RuntimeBody'],
                 'vectors': va, 'encoded_entry': (symbols['Proc1RuntimeEntry'] | 1) ^ 0x4ff83f2d,
                 'proc0_vtor': 0x20000000, 'proc0_stack_floor': 0x2000e000,
                 'global_bss_clear_excludes_proc1': True, 'external_calls': 0}
    return {'classification':'BUILD', 'status':'PASS', 'entry':hex(entry),
            'pt_load_end':hex(max(row[1] for row in loads)), 'msp':[hex(0x2000e000),hex(vector[0])],
            'bss_bytes':symbols['__ebss']-symbols['__sbss'], 'direct_rtos_vectors':True,
            'checked_vector_indices': sorted(required_vectors),
            'exclusive_instructions':0, 'proc1':proc1, 'scmi':scmi, 'amp_control':control,'amp_proc1_probe':amp_probe,
            'amp_observation':observation,
            'local_monitor_stack':local_monitor, 'hardware':'OPEN'}


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('elf')
    ap.add_argument('--local-monitor-stack', action='store_true')
    ap.add_argument('--require-scmi', action='store_true')
    ap.add_argument('--require-ddr-diagnostics', action='store_true')
    ap.add_argument('--amp-control', action='store_true')
    ap.add_argument('--amp-proc1', action='store_true')
    ap.add_argument('--amp-proc1-gpio-irq', action='store_true')
    ap.add_argument('--amp-observation', action='store_true')
    args = ap.parse_args()
    print(json.dumps(check(args.elf, args.local_monitor_stack, args.require_scmi, args.require_ddr_diagnostics, args.amp_control,args.amp_proc1,args.amp_proc1_gpio_irq,args.amp_observation), indent=2))

#!/usr/bin/env python3
"""Compile the supplied base plus generated policy as one DTS, then validate.

No deployment. Output includes private base-DT data; do not publish it blindly.
Structural PASS is not physical clock hold, kernel build or SCMI IRQ proof.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from clock_profile import DEFAULT, load, outputs
from scmi_elf_layout import read_layout, transport_dtsi
from validate_linux_dtb import Tree, decode, strings, validate


def resolve_labels(fragment, tree, symbols):
    local = set(re.findall(r'\b([A-Za-z_]\w*):\s*[A-Za-z0-9_,@.-]+\s*\{', fragment))
    def replace(match):
        label = match[1]
        if label in local:
            if label in symbols: raise ValueError('new label collides with base: ' + label)
            return match[0]
        path = strings(symbols.get(label))
        if len(path) != 1 or path[0] not in tree.nodes:
            raise ValueError('missing/invalid base symbol: ' + label)
        if not re.fullmatch(r'/[A-Za-z0-9_,@./+-]+',path[0]):
            raise ValueError('unsafe base symbol path')
        return '&{' + path[0] + '}'
    return re.sub(r'&([A-Za-z_]\w*)', replace, fragment)


def build(base, elf, profile_path, out):
    profile = load(profile_path)
    layout = read_layout(elf)
    root = decode(base); tree = Tree(root)
    if tree.errors: raise ValueError(tree.errors)
    transport = transport_dtsi(layout,tree,profile)
    generated = {p.name:content for p,content in outputs(profile).items()}
    # Never use flattened overlays for /delete-property/: the base property
    # would survive. Resolve base labels and merge the source before flattening.
    fragment = transport + generated['clocks.dtsi'] + generated['ownership.dtsi']
    fragment = resolve_labels(fragment,tree,root.get('__symbols__', {}))
    out.mkdir(parents=True,exist_ok=False)
    source = subprocess.run(['dtc','-I','dtb','-O','dts',str(base)],capture_output=True,check=True)
    (out/'base-decode.stderr').write_bytes(source.stderr)
    (out/'candidate.dts').write_bytes(source.stdout + b'\n' + fragment.encode())
    compiled = subprocess.run(['dtc','-@','-I','dts','-O','dtb','-o',str(out/'candidate.dtb'),
                               str(out/'candidate.dts')],capture_output=True)
    (out/'compile.stderr').write_bytes(compiled.stderr)
    compiled.check_returncode()
    candidate_root=decode(out/'candidate.dtb'); candidate_tree=Tree(candidate_root)
    result = validate(candidate_root,profile,firmware_layout=layout)
    symbols=root.get('__symbols__', {})
    disabled_paths=[strings(symbols.get(label))[0] for label in profile['linux']['disable_labels']]
    uart=strings(symbols.get(profile['linux']['uart1_label']))[0]
    mailbox=[p for p,n in tree.nodes.items() if 'raspberrypi,rp1-mbox' in strings(n.get('compatible'))]
    changes=[]
    for path,on in tree.enabled.items():
        after=candidate_tree.enabled.get(path)
        if after==on: continue
        changes.append({'node':path,'before':on,'after':after})
        allowed=(after is False and any(path==p or path.startswith(p+'/') for p in disabled_paths))
        allowed |= after is True and (path==uart or path in mailbox)
        if not allowed: result['failures'].append('unexpected base-node availability change: '+path)
    result['result']='FAIL' if result['failures'] else 'PASS'
    result['base_availability_changes']=changes
    result.update(base_dtb_sha256=hashlib.sha256(base.read_bytes()).hexdigest(),
        dtb_sha256=hashlib.sha256((out/'candidate.dtb').read_bytes()).hexdigest(),
        construction='one DTS translation unit; no deployment')
    (out/'validation.json').write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
    (out/'layout.json').write_text(json.dumps(layout,indent=2,sort_keys=True)+'\n')
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base',required=True,type=Path)
    parser.add_argument('--firmware-elf',required=True,type=Path)
    parser.add_argument('--profile',type=Path,default=DEFAULT)
    parser.add_argument('--out',required=True,type=Path,help='new directory; may contain private DT data')
    args=parser.parse_args()
    result=build(args.base,args.firmware_elf,args.profile,args.out)
    print(result['result']+': '+str(len(result['failures']))+' failures; '+str(args.out/'validation.json'))
    return 0 if result['result']=='PASS' else 1


if __name__=='__main__':
    raise SystemExit(main())

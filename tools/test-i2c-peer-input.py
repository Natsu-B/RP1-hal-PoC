#!/usr/bin/env python3
"""Compile the actual pure admission function; no target MMIO or serial."""
from pathlib import Path
import re
import subprocess
import tempfile
root=Path(__file__).resolve().parents[1]
peer=(root/'examples/minimal/src/freertos_i2c_peer.rs').read_text()
function=re.search(r'fn input_admitted\(pre:\[u32;7\]\)->bool \{[^{}]+\}',peer).group()
main=(root/'examples/minimal/src/main.rs').read_text()
assert 'freertos_r1::i2c::set_peer_pin(_miso);' in main
assert 'into_input' not in peer and 'assert!(input_admitted(pre));' in peer
checks=r'''
#[test] fn configured_r1_input_only() {
    let good=[0x85,0xda,0x0aae0000,0x00400980,0x38c,0xfd73fffe,0x038c0001];
    assert!(input_admitted(good));
    let mut independent=good; independent[4]=0xffffffff;
    assert!(input_admitted(independent)); // HIGH dwell is checked separately.
    assert!(!input_admitted([0x9f,0x96,0x04400000,0x00400000,0,good[5],good[6]]));
    for (index,value) in [(0,0x80),(1,0x56),(2,good[2]|1<<13),
        (3,good[3]|1<<9),(5,good[5]|1<<19),(6,good[6]&!(1<<19))] {
        let mut bad=good;bad[index]=value;assert!(!input_admitted(bad));
    }
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-i2c-input-test-') as tmp:
    p=Path(tmp);(p/'check.rs').write_text(function+checks)
    subprocess.run(['rustc','--edition=2021','--test',str(p/'check.rs'),'-o',str(p/'check')],check=True)
    subprocess.run([str(p/'check')],check=True)
print('STATIC actual input admission and startup handle path PASS; HIGH dwell/HW separate')

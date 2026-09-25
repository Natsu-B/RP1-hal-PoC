#!/usr/bin/env python3
"""Check resource floors and child cleanup without a kernel build/hardware."""
import json
import os
from pathlib import Path
import sys
import tempfile
import build_linux_image as b
from build_linux_image import GIB, capacity

def require(condition):
    if not condition:
        raise AssertionError('build guard check failed')

require(capacity(2*GIB, GIB, 1_500_000_000))
require(not capacity(2*GIB-1, GIB, 1_500_000_000))
require(not capacity(2*GIB, GIB-1, 1_500_000_000))
require(not capacity(2*GIB, GIB, 1_499_999_999))
with tempfile.TemporaryDirectory(prefix='rp1-image-check-') as tmp:
    path = Path(tmp)
    original = b.resources
    try:
        b.resources = lambda *_: [2*GIB, GIB, 1_500_000_000]
        b.run(['/bin/true'], path, path, os.environ.copy(), 'ok', 10)
        for name, command, seconds, expected in (
            ('exit', [sys.executable, '-c', 'raise SystemExit(7)'], 10, None),
            ('timeout', ['/bin/sleep', '60'], -1, 'time limit'),
        ):
            try:
                b.run(command, path, path, os.environ.copy(), name, seconds)
                raise AssertionError('failed command accepted')
            except RuntimeError:
                result = json.loads((path/(name+'.json')).read_text())
                require(result['guard'] == expected and result['exit'] != 0)
        b.resources = lambda *_: [0, GIB, 1_500_000_000]
        try:
            b.run(['/bin/true'], path, path, os.environ.copy(), 'refused', 10)
            raise AssertionError('resource floor ignored')
        except RuntimeError:
            require(not (path/'refused.log').exists())
    finally:
        b.resources = original
print('PASS resource floors, success/failure, timeout group cleanup and admission refusal')

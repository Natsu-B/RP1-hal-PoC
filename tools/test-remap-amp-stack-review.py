#!/usr/bin/env python3
"""Only same-symbol direct branch/literal relocations may change a pivot."""
import importlib.util
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('remap', Path(__file__).with_name('remap-amp-stack-review.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
with patch.object(m.subprocess, 'check_output', return_value='20000000: bf00 nop\n20000002: 0000 .short 0x0000\n'):
    assert len(m.pivot_rows(Path('unused'), 0x20000000, 4)) == 2
old = {0: (['f006', 'bb9f'], 'b.w', '200069f0 <reset>'),
       4: (['2000f000'], '.word', '0x2000f000')}
new = {0: (['f006', 'bab5'], 'b.w', '2000681c <reset>'),
       4: (['2000f000'], '.word', '0x2000f000')}
assert m.check_pivot(old, new, {0x200069f0: {'reset'}}, {0x2000681c: {'reset'}}, 'Reset')
for bad, names in [
    ({**new, 0: (['f006', 'bab5'], 'bl', '2000681c <reset>')}, {'reset'}),
    (new, {'other'}),
    ({**new, 4: (['2000e000'], '.word', '0x2000e000')}, {'reset'}),
    ({**new, 4: (['2000f001'], '.word', '0x2000f001')}, {'reset'}),
    ({**new, 4: (['b672'], 'cpsid', 'i')}, {'reset'}),
]:
    try:
        m.check_pivot(old, bad, {0x200069f0: {'reset'}}, {0x2000681c: names}, 'Reset')
    except ValueError:
        pass
    else:
        raise AssertionError('unsafe pivot accepted')
print('HOST_PASS same-symbol branch relocation; 5 unsafe mutations refused')

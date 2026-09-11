#!/usr/bin/env python3
"""MD01 numeric extension; original mixed and external GPIO acceptance stays required."""
import json
from pathlib import Path
import runpy
import sys

if not __debug__:
    raise SystemExit('evidence assertions required')
BASE=runpy.run_path(str(Path(__file__).with_name('check-freertos-mixed-repeat.py')))

def validate(text):
    result=BASE['validate'](text); rows=BASE['decode'](text)
    for n,w in enumerate(rows[3:],3):
        assert w[63]==int.from_bytes(b'MD01','little'), 'monitor deadline schema'
        assert w[60]==0, 'monitor no-block/missed cycle'
        assert 0<w[61]<10000, 'monitor bookkeeping bound'
        assert w[62]<10, 'monitor wake lateness bound'
        if n>3:
            assert rows[n-1][61]<=w[61] and rows[n-1][62]<=w[62], 'monitor maximum regression'
    result['monitor_deadline']=dict(period_ticks=1000,no_block_count=rows[-1][60],
        max_bookkeeping_us=rows[-1][61],max_wake_lateness_ticks=rows[-1][62],
        absolute_wall_accuracy_proven=False,full_mask_maximum_proven=False)
    result['result']='R2_MIXED_ABSOLUTE_MONITOR_NUMERIC_PASS'
    return result

if __name__=='__main__':
    assert len(sys.argv)==2, 'observer log required'
    print(json.dumps(validate(Path(sys.argv[1]).read_text(errors='replace')),indent=2))
